use anyhow::{Result, bail};
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::files::file_task_media::Identity;
use plain_rs::services::files::watch_backend::Kind;
use plain_rs::services::files::watch_backend::Watcher;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::time::Instant;

const IDENTITY_PREFIX: &str = "external_file_identity:";
const MOVE_PAIR_TIMEOUT: Duration = Duration::from_secs(1);
const BACKGROUND_BATCH_SIZE: usize = 32;
const VOLUME_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone, Serialize, Deserialize)]
struct Known {
    identity: Identity,
    file_key: Option<String>,
}

struct Catalog {
    roots: Vec<PathBuf>,
    known: BTreeMap<PathBuf, Known>,
    watcher: Watcher,
    moves: HashMap<u32, (PathBuf, Instant)>,
    work: Work,
}

#[derive(Default)]
struct Work {
    directories: VecDeque<PathBuf>,
    files: VecDeque<PathBuf>,
    walking: Option<tokio::fs::ReadDir>,
    missing: Option<VecDeque<PathBuf>>,
    reconciling: bool,
}

impl Work {
    fn new(roots: &[PathBuf]) -> Self {
        Self {
            directories: roots.iter().cloned().collect(),
            reconciling: true,
            ..Self::default()
        }
    }

    fn pending(&self) -> bool {
        self.reconciling
            || self.walking.is_some()
            || !self.directories.is_empty()
            || !self.files.is_empty()
    }

    fn files(&mut self) -> Vec<PathBuf> {
        let count = self.files.len().min(BACKGROUND_BATCH_SIZE);
        self.files.drain(..count).collect()
    }
}

async fn change_or_work<Read: std::future::Future>(
    read: Read,
    background: bool,
) -> Option<Read::Output> {
    tokio::select! {
        biased;
        change = read => Some(change),
        _ = std::future::ready(()), if background => None,
    }
}

pub fn start(state: CoreRuntime) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut stop = state.stop.clone();
        let mut connection = state.host.connection_changes();
        loop {
            if *stop.borrow() {
                return;
            }
            if state.host.connected() && state.host.uses_system_media_catalog() {
                match run(&state).await {
                    Ok(false) => return,
                    Ok(true) => {}
                    Err(error) => log::warn!("External media indexing: {error}"),
                }
            }
            tokio::select! {
                _ = stop.changed() => return,
                _ = connection.changed() => {},
                _ = tokio::time::sleep(Duration::from_secs(5)), if state.host.connected() && state.host.uses_system_media_catalog() => {},
            }
        }
    })
}

async fn roots(state: &CoreRuntime) -> Result<Vec<PathBuf>> {
    let facts = state.file_platform.mounts(state.prefs.clone()).await?;
    let rows = facts
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("invalid storage volume facts"))?;
    let mut roots = Vec::new();
    for row in rows {
        if !matches!(
            row["driveType"].as_str(),
            Some("INTERNAL_STORAGE" | "SDCARD" | "USB_STORAGE")
        ) {
            continue;
        }
        let Some(path) = row["path"].as_str().filter(|path| !path.is_empty()) else {
            continue;
        };
        let path = match tokio::fs::canonicalize(path).await {
            Ok(path) => path,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        if path.parent().is_none() || !state.file_platform.watchable(&path) {
            continue;
        }
        state
            .file_platform
            .authorize(&path.to_string_lossy(), false)
            .await?;
        if tokio::fs::metadata(&path).await?.is_dir() {
            roots.push(path);
        }
    }
    Ok(plain_rs::files::filesystem::roots::prune_nested(
        roots,
        |_| false,
    ))
}

async fn run(state: &CoreRuntime) -> Result<bool> {
    let Some(watcher) = Watcher::new()? else {
        return Ok(false);
    };
    let generation = state.host.generation();
    let mut stop = state.stop.clone();
    let mut connection = state.host.connection_changes();
    let mut events = state.events.subscribe();
    let mut catalog = Catalog {
        roots: roots(state).await?,
        known: BTreeMap::new(),
        watcher,
        moves: HashMap::new(),
        work: Work::default(),
    };
    for entry in state.store.get()?.scan_prefix(IDENTITY_PREFIX) {
        let (key, value) = entry?;
        let row: Known = serde_json::from_str(&value)?;
        let path = PathBuf::from(&row.identity.path);
        if key != identity_key(&path)
            || !path.is_absolute()
            || row.identity.file_id.is_empty()
            || !matches!(row.identity.media_type, 1 | 2 | 3 | 24)
        {
            bail!("invalid saved external media identity");
        }
        catalog.known.insert(path, row);
    }
    catalog.work = Work::new(&catalog.roots);
    let mut refresh = tokio::time::interval(VOLUME_REFRESH_INTERVAL);
    refresh.tick().await;
    loop {
        if *stop.borrow() || state.host.generation() != generation {
            return Ok(true);
        }
        let deadline = catalog
            .moves
            .values()
            .map(|(_, deadline)| *deadline)
            .min()
            .unwrap_or_else(|| Instant::now() + VOLUME_REFRESH_INTERVAL);
        tokio::select! {
            biased;
            _ = stop.changed() => return Ok(true),
            _ = connection.changed() => return Ok(true),
            _ = refresh.tick() => {
                let current = roots(state).await?;
                if current != catalog.roots {
                    let Some(watcher) = Watcher::new()? else { return Ok(false) };
                    catalog.watcher = watcher;
                    catalog.roots = current;
                    catalog.moves.clear();
                    catalog.work = Work::new(&catalog.roots);
                }
            },
            event = events.recv() => match event {
                Ok(event) if matches!(event.event_type, "PERMISSIONS_UPDATED" | "PREFS_UPDATED") => return Ok(true),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => return Ok(true),
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(false),
                _ => {},
            },
            _ = tokio::time::sleep_until(deadline), if !catalog.moves.is_empty() => {
                let expired = catalog.moves.iter().filter_map(|(cookie, (path, deadline))|
                    (*deadline <= Instant::now()).then_some((*cookie, path.clone()))).collect::<Vec<_>>();
                for (cookie, path) in expired {
                    catalog.moves.remove(&cookie);
                    catalog.remove(state, &path).await?;
                    catalog.watcher.remove(&path);
                }
            },
            change = change_or_work(catalog.watcher.read(), catalog.work.pending()) => {
                let Some(raw) = change else { catalog.next_work(state).await?; continue };
                let mut completed = Vec::new();
                let mut rescan = false;
                for raw in raw? {
                    let Some(change) = catalog.watcher.change(raw) else { continue };
                    if change.kind == Kind::Refresh { rescan = true; continue; }
                    if !catalog.contains(&change.path) { continue; }
                    if change.path.file_name().is_some_and(|name| name == ".nomedia") {
                        if let Some(parent) = change.path.parent() {
                            state.file_platform.authorize(&parent.to_string_lossy(), false).await?;
                            state.file_platform.scan(&[parent.to_string_lossy().into_owned()]).await?;
                        }
                        rescan = true;
                        continue;
                    }
                    match change.kind {
                        Kind::From(cookie) => { catalog.moves.insert(cookie, (change.path, Instant::now() + MOVE_PAIR_TIMEOUT)); },
                        Kind::To(cookie) => {
                            if let Some((source, _)) = catalog.moves.remove(&cookie) {
                                catalog.watcher.moved(&source, &change.path);
                                catalog.moved(state, &source, &change.path).await?;
                            }
                            completed.extend(catalog.enqueue(state, &change.path).await?);
                        },
                        Kind::Removed => { catalog.remove(state, &change.path).await?; catalog.watcher.remove(&change.path); },
                        Kind::Directory => completed.extend(catalog.enqueue(state, &change.path).await?),
                        Kind::Completed => completed.push(change.path),
                        Kind::Refresh => {},
                    }
                }
                if rescan {
                    let current = roots(state).await?;
                    let Some(watcher) = Watcher::new()? else { return Ok(false) };
                    catalog.watcher = watcher;
                    catalog.roots = current;
                    catalog.moves.clear();
                    catalog.work = Work::new(&catalog.roots);
                } else {
                    catalog.scan(state, completed).await?;
                }
            },
        }
    }
}

impl Catalog {
    fn contains(&self, path: &Path) -> bool {
        self.roots.iter().any(|root| path.starts_with(root))
    }

    async fn enqueue(&mut self, state: &CoreRuntime, path: &Path) -> Result<Vec<PathBuf>> {
        if !self.contains(path) || !state.file_platform.watchable(path) {
            return Ok(Vec::new());
        }
        let metadata = match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) => metadata,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                ) =>
            {
                return Ok(Vec::new());
            }
            Err(error) => return Err(error.into()),
        };
        if metadata.is_dir() {
            self.watcher.add(path)?;
            self.work.directories.push_front(path.to_owned());
        } else if metadata.is_file() && state.file_platform.indexable(path) {
            return Ok(vec![path.to_owned()]);
        }
        Ok(Vec::new())
    }

    async fn next_work(&mut self, state: &CoreRuntime) -> Result<()> {
        if let Some(entries) = self.work.walking.as_mut() {
            for _ in 0..BACKGROUND_BATCH_SIZE {
                let Some(entry) = entries.next_entry().await? else {
                    self.work.walking = None;
                    break;
                };
                let path = entry.path();
                if !state.file_platform.watchable(&path) {
                    continue;
                }
                let kind = match entry.file_type().await {
                    Ok(kind) => kind,
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                        ) =>
                    {
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                if kind.is_dir() {
                    self.work.directories.push_back(path);
                } else if kind.is_file() && state.file_platform.indexable(&path) {
                    self.work.files.push_back(path);
                }
            }
        } else if let Some(path) = self.work.directories.pop_front() {
            if !self.contains(&path) || !state.file_platform.watchable(&path) {
                return Ok(());
            }
            let opened = async {
                if !tokio::fs::symlink_metadata(&path).await?.is_dir() {
                    return Ok(None);
                }
                self.watcher.add(&path)?;
                tokio::fs::read_dir(&path).await.map(Some)
            }
            .await;
            match opened {
                Ok(entries) => self.work.walking = entries,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        } else if !self.work.files.is_empty() {
            let files = self.work.files();
            self.scan(state, files).await?;
        } else if self.work.reconciling {
            if self.work.missing.is_none() {
                if roots(state).await? != self.roots {
                    bail!("storage volumes changed during media reconciliation");
                }
                self.work.missing = Some(
                    self.known
                        .keys()
                        .filter(|path| self.contains(path))
                        .cloned()
                        .collect(),
                );
            }
            let path = self.work.missing.as_mut().and_then(VecDeque::pop_front);
            if let Some(path) = path {
                if missing_path(&path).await? {
                    self.remove(state, &path).await?;
                }
            } else {
                self.work.reconciling = false;
            }
        }
        Ok(())
    }

    async fn scan(&mut self, state: &CoreRuntime, mut paths: Vec<PathBuf>) -> Result<()> {
        paths.sort();
        paths.dedup();
        paths.retain(|path| self.contains(path) && state.file_platform.indexable(path));
        if paths.is_empty() {
            return Ok(());
        }
        let mut changed_types = Vec::new();
        for path in &paths {
            let key = file_key(path).await?;
            if self.known.contains_key(path) {
                continue;
            }
            let current = state
                .file_platform
                .identities(&[path.to_string_lossy().into_owned()])
                .await?;
            let candidates = self
                .known
                .iter()
                .filter_map(|(source, row)| {
                    (self.contains(source)
                        && source != path
                        && matches_identity(row, key.as_deref(), &current))
                    .then_some(source.clone())
                })
                .collect::<Vec<_>>();
            let mut missing = Vec::new();
            for source in candidates {
                if missing_path(&source).await? {
                    missing.push(source);
                }
            }
            if missing.len() == 1 {
                self.moved(state, &missing[0], path).await?;
            }
        }
        for paths in paths.chunks(128) {
            let mut existing = Vec::new();
            for path in paths {
                if !missing_path(path).await? {
                    existing.push(path.to_string_lossy().into_owned());
                }
            }
            if existing.is_empty() {
                continue;
            }
            state
                .file_platform
                .authorize_paths(&existing, false)
                .await?;
            state.file_platform.scan(&existing).await?;
            let mut updates = Vec::new();
            for identity in state.file_platform.identities(&existing).await? {
                let path = PathBuf::from(&identity.path);
                if !existing.contains(&identity.path) {
                    bail!("external media receipt outside completed paths");
                }
                if let Some(previous) = self.known.get(&path) {
                    state
                        .files
                        .external_replace(previous.identity.clone(), &identity)
                        .await?;
                    changed_types.push(previous.identity.media_type);
                }
                changed_types.push(identity.media_type);
                let row = Known {
                    file_key: file_key(&path).await?,
                    identity,
                };
                updates.push((path, row));
            }
            self.save(state, updates, &[])?;
        }
        plain_rs::services::files::catalog_events::publish(&state.events, changed_types);
        Ok(())
    }

    async fn moved(
        &mut self,
        state: &CoreRuntime,
        source: &Path,
        destination: &Path,
    ) -> Result<()> {
        if !missing_path(source).await? || missing_path(destination).await? {
            return Ok(());
        }
        let selected = self
            .known
            .iter()
            .filter_map(|(path, row)| {
                path.starts_with(source)
                    .then_some((path.clone(), row.identity.clone()))
            })
            .collect::<Vec<_>>();
        if !state.file_platform.watchable(destination) {
            return self.remove(state, source).await;
        }
        state
            .file_platform
            .authorize_paths(
                &[
                    source.to_string_lossy().into_owned(),
                    destination.to_string_lossy().into_owned(),
                ],
                false,
            )
            .await?;
        let mut moving = Vec::new();
        let mut destination_paths = Vec::new();
        for (path, row) in &selected {
            let target = destination.join(path.strip_prefix(source)?);
            let target_kind = plain_rs::media::catalog_paths::infer_type(&target.to_string_lossy());
            let expected_kind = match row.media_type {
                1 => "audio",
                2 => "video",
                3 => "image",
                24 => "doc",
                _ => "",
            };
            if state.file_platform.indexable(&target) && target_kind == expected_kind {
                moving.push(row.clone());
                destination_paths.push(target.to_string_lossy().into_owned());
            } else {
                state.files.external_remove(path, vec![row.clone()]).await?;
            }
        }
        if !moving.is_empty() {
            state
                .files
                .external_move(source, destination, moving)
                .await?;
        }
        let mut updates = Vec::new();
        for identity in state.file_platform.identities(&destination_paths).await? {
            if !destination_paths.contains(&identity.path) {
                bail!("external move receipt outside destination paths");
            }
            let path = PathBuf::from(&identity.path);
            let row = Known {
                file_key: file_key(&path).await?,
                identity,
            };
            updates.push((path, row));
        }
        if updates.len() != destination_paths.len() {
            bail!("external move identity receipt incomplete");
        }
        self.save(
            state,
            updates,
            &selected
                .into_iter()
                .map(|(path, _)| path)
                .collect::<Vec<_>>(),
        )
    }

    async fn remove(&mut self, state: &CoreRuntime, root: &Path) -> Result<()> {
        if !self.contains(root) || !missing_path(root).await? {
            return Ok(());
        }
        let selected = self
            .known
            .iter()
            .filter_map(|(path, row)| {
                path.starts_with(root)
                    .then_some((path.clone(), row.identity.clone()))
            })
            .collect::<Vec<_>>();
        state
            .file_platform
            .authorize(&root.to_string_lossy(), false)
            .await?;
        state
            .files
            .external_remove(root, selected.iter().map(|(_, row)| row.clone()).collect())
            .await?;
        self.save(
            state,
            Vec::new(),
            &selected
                .into_iter()
                .map(|(path, _)| path)
                .collect::<Vec<_>>(),
        )
    }

    fn save(
        &mut self,
        state: &CoreRuntime,
        updates: Vec<(PathBuf, Known)>,
        removed: &[PathBuf],
    ) -> Result<()> {
        let store = state.store.get()?;
        save_identities(&store, &updates, removed)?;
        for path in removed {
            self.known.remove(path);
        }
        self.known.extend(updates);
        Ok(())
    }
}

fn save_identities(
    store: &plain_rs::kv::Db,
    updates: &[(PathBuf, Known)],
    removed: &[PathBuf],
) -> Result<()> {
    let mut batch = store.batch_with_capacity(updates.len() + removed.len());
    for path in removed {
        batch.remove(identity_key(path));
    }
    for (path, row) in updates {
        batch.insert(identity_key(path), serde_json::to_string(row)?);
    }
    store.apply_batch(batch)
}

fn matches_identity(row: &Known, key: Option<&str>, current: &[Identity]) -> bool {
    match (row.file_key.as_deref(), key) {
        (Some(previous), Some(current)) => previous == current,
        _ => current.iter().any(|current| {
            current.media_type == row.identity.media_type && current.file_id == row.identity.file_id
        }),
    }
}

fn identity_key(path: &Path) -> String {
    format!("{IDENTITY_PREFIX}{}", path.to_string_lossy())
}

async fn missing_path(path: &Path) -> Result<bool> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error.into()),
    }
}

async fn file_key(path: &Path) -> Result<Option<String>> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || match plain_rs::utils::file_identity::key(&path) {
        Ok(key) => Ok(key),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::Unsupported
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error.into()),
    })
    .await?
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/external_index.rs"]
mod tests;
