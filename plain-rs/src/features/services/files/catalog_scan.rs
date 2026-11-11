#[cfg(feature = "runtime-media")]
mod directory_walk;

#[cfg(feature = "runtime-media")]
use crate::features::services::files::media_scans::Purpose;

#[cfg(feature = "runtime-media")]
const SCAN_BATCH_SIZE: usize = 256;

use crate::features::enums::DataType;
use crate::infrastructure::db::MediaFileMetadata;
use crate::infrastructure::db::MediaFileRow;
use crate::runtime::Runtime as CoreRuntime;

pub async fn host_connected(state: CoreRuntime, generation: u64) -> Result<(), String> {
    configure_sources(state, generation, false).await
}

async fn configure_sources(
    state: CoreRuntime,
    generation: u64,
    scan_new_sources: bool,
) -> Result<(), String> {
    if state.host.uses_system_media_catalog() {
        return Ok(());
    }
    let capabilities = crate::features::services::capabilities::snapshot(&state.host).await;
    if !state.host.is_current(generation) {
        return Ok(());
    }
    if !capabilities
        .map_err(|error| error.to_string())?
        .iter()
        .any(|capability| capability == "MEDIA_SCAN")
    {
        return Ok(());
    }
    #[cfg(feature = "runtime-media")]
    {
        let volumes = crate::features::services::files::indexing::discover(&state).await?;
        let policy = state.media_scans.policy()?;
        if policy
            .scope
            .as_ref()
            .is_some_and(|scope| scope.volumes == volumes)
        {
            return Ok(());
        }
        let catalog = NativeMediaCatalog::new(state);
        catalog.configure_policy((*policy).clone())?;
        catalog.watch_source_directories()?;
        if scan_new_sources
            && let Some(previous) = &policy.scope
        {
            let references = catalog
                .source_directories()?
                .into_iter()
                .filter(|path| !previous.directories.contains(path))
                .collect::<Vec<_>>();
            if !references.is_empty() {
                catalog.start_scan(references, catalog.state.media_scans.policy()?, true)?;
            }
        }
        Ok(())
    }
    #[cfg(not(feature = "runtime-media"))]
    {
        let _ = scan_new_sources;
        Err("MEDIA_SCAN advertised without a scanner implementation".into())
    }
}

pub async fn access_changed(state: CoreRuntime, generation: u64) -> Result<(), String> {
    if state.file_platform.uses_sdk() && !state.host.uses_system_media_catalog() {
        configure_sources(state, generation, true).await?;
    }
    Ok(())
}

#[cfg(feature = "runtime-media")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletedFilesRequest {
    paths: Vec<String>,
}

#[derive(Clone)]
pub struct NativeMediaCatalog {
    state: CoreRuntime,
    #[cfg(test)]
    read_gate: Option<std::sync::Arc<(tokio::sync::Notify, tokio::sync::Semaphore)>>,
}

impl NativeMediaCatalog {
    pub fn new(state: CoreRuntime) -> Self {
        Self {
            state,
            #[cfg(test)]
            read_gate: None,
        }
    }

    pub fn source_directories(&self) -> Result<Vec<String>, String> {
        self.state.native_client_id()?;
        let volumes = self.state.db.volumes().map_err(|error| error.to_string())?;
        Ok(crate::features::files::index_scope::source_directories(
            &volumes,
        ))
    }

    #[cfg(feature = "runtime-media")]
    pub async fn scan_file(
        &self,
        data_type: DataType,
        reference: &str,
    ) -> Result<MediaFileRow, String> {
        let metadata = self.read_file(data_type, reference).await?;
        let rows = self.record_file_reads(&[metadata])?;

        rows.into_iter()
            .next()
            .ok_or_else(|| "Missing recorded media file".into())
    }

    #[cfg(feature = "runtime-media")]
    async fn read_file(
        &self,
        data_type: DataType,
        reference: &str,
    ) -> Result<crate::infrastructure::db::MediaFileMetadataRead, String> {
        #[cfg(test)]
        if let Some(gate) = &self.read_gate {
            gate.0.notify_one();
            gate.1
                .acquire()
                .await
                .map_err(|error| error.to_string())?
                .forget();
        }
        let path =
            crate::features::services::files::file_access::resolve_native(&self.state, reference)
                .await
                .map_err(|error| error.to_string())?;
        if crate::features::media::catalog_paths::scan_excluded_root(&path).is_some() {
            return Err("File excluded from scanning".into());
        }
        let metadata = tokio::task::spawn_blocking(move || {
            crate::features::media::file_facts::read_file_metadata_with_availability(
                &path, data_type,
            )
            .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())??;
        Ok(metadata)
    }

    #[cfg(feature = "runtime-media")]
    pub async fn scan_files(
        &self,
        data_type: DataType,
        references: Vec<String>,
    ) -> Result<crate::features::services::files::media_scans::Snapshot, String> {
        self.state.native_client_id()?;
        data_type
            .media_type_str()
            .ok_or("Unsupported media catalog type")?;
        let policy = self.state.media_scans.policy()?;
        let references = references
            .into_iter()
            .filter(|path| {
                crate::features::media::catalog_paths::scan_excluded_root(std::path::Path::new(
                    path,
                ))
                .is_none()
            })
            .collect::<Vec<_>>();
        let session = self.state.media_scans.begin(references.len())?;
        let mut cancel = session.cancel.clone();
        let mut stop = self.state.stop.clone();
        let result = async {
            for reference in references {
                session.wait_running().await?;
                let metadata = tokio::select! {
                    biased;
                    _ = crate::features::services::files::media_scans::wait_stop(&mut cancel) => return Err("Media scan cancelled".into()),
                    _ = crate::features::services::files::media_scans::wait_stop(&mut stop) => {
                        self.state.media_scans.cancel()?;
                        return Err("Rust core stopped".into());
                    },
                    metadata = self.read_file(data_type, &reference) => metadata?,
                };
                session.record_when_running(1,|| self.persist_file_reads(&[metadata], policy.scope.as_ref())).await?;

            }
            Ok(())
        }.await;
        session.finish(result)
    }

    #[cfg(feature = "runtime-media")]
    pub async fn scan_completed_files(&self, params: plain_libs::serde_json::Value) -> Result<bool, String> {
        self.state.native_client_id()?;
        let request: CompletedFilesRequest =
            plain_libs::serde_json::from_value(params).map_err(|error| error.to_string())?;
        crate::features::services::files::native_scan::completed(
            &self.state.db,
            &self.state.media_scans,
            &self.state.directory,
            request.paths,
        )
        .await?;

        Ok(true)
    }

    #[cfg(feature = "runtime-media")]
    pub fn configure_scan_directories(
        &self,
        config_file: Option<&std::path::Path>,
    ) -> Result<(), String> {
        self.state.native_client_id()?;
        let policy = crate::features::services::files::media_scan_policy::Policy::load(
            self.state.cache.root(),
            config_file,
        )?;
        self.configure_policy(policy)
    }

    #[cfg(feature = "runtime-media")]
    pub fn configure_policy(
        &self,
        mut policy: crate::features::services::files::media_scan_policy::Policy,
    ) -> Result<(), String> {
        let scope = crate::features::files::index_scope::Scope::load(
            &self.state.db,
            &self.state.directory,
            self.state.cache.root(),
            &policy.excluded_roots,
            self.state.media_scans.defaults()?,
        )?;
        policy.scope = Some(scope.clone());
        self.state.media_scans.configure(policy)?;
        self.state
            .db
            .update_index_visibility(&scope, None)
            .map_err(|error| error.to_string())
    }

    #[cfg(feature = "runtime-media")]
    pub async fn refresh_scope(&self, volume_id: &str) -> Result<(), String> {
        self.state
            .media_watch
            .lock()
            .map_err(|_| "Media watcher unavailable")?
            .take();
        self.state.media_scans.cancel()?;
        self.state.media_scans.wait_background().await?;
        self.state.media_scans.wait_idle().await?;
        self.configure_policy((*self.state.media_scans.policy()?).clone())?;
        self.watch_source_directories()?;
        self.start_volume_scan(volume_id)?;
        Ok(())
    }

    #[cfg(feature = "runtime-media")]
    pub async fn scan_directory(
        &self,
        reference: &str,
    ) -> Result<crate::features::services::files::media_scans::Snapshot, String> {
        self.scan_directories(vec![reference.to_owned()], Purpose::Rebuild)
            .await
    }

    #[cfg(feature = "runtime-media")]
    pub fn watch_source_directories(&self) -> Result<bool, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .refresh_volume_availability()
            .map_err(|error| error.to_string())?;
        let roots = self
            .source_references("/")?
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect::<Vec<_>>();
        if roots.is_empty() {
            self.state
                .media_watch
                .lock()
                .map_err(|_| "Media watcher unavailable")?
                .take();
            return Ok(false);
        }
        let catalog_state = self.state.clone();
        let handle = crate::features::services::files::media_watch::start(
            &roots,
            self.state.media_scans.policy()?.scope.clone(),
            move |event| {
                let catalog = Self::new(catalog_state.clone());
                async move { catalog.apply_watch_event(event).await }
            },
        )?;
        let mut slot = self
            .state
            .media_watch
            .lock()
            .map_err(|_| "media watch unavailable")?;
        *slot = Some(handle);
        Ok(true)
    }

    #[cfg(feature = "runtime-media")]
    async fn apply_watch_event(&self, event: plain_libs::file_events::Event) -> Result<(), String> {
        self.state
            .db
            .refresh_volume_availability()
            .map_err(|error| error.to_string())?;
        use crate::features::services::files::media_watch::Effect;
        use crate::features::services::files::media_watch::effect_for;
        use plain_libs::file_events::event::{EventKind, ModifyKind, RenameMode};
        let volumes = if matches!(event.kind, EventKind::Other)
            || event.paths.iter().any(|path| path.is_dir())
        {
            crate::features::services::files::indexing::discover(&self.state).await?
        } else {
            self.state.db.volumes().map_err(|error| error.to_string())?
        };
        let policy = self.state.media_scans.policy()?;
        if policy
            .scope
            .as_ref()
            .is_some_and(|scope| scope.volumes != volumes)
        {
            self.configure_policy((*policy).clone())?;
        }
        if matches!(event.kind, EventKind::Other) {
            if !self.state.media_scans.wait_idle().await? {
                return Ok(());
            }
            self.scan_directories(self.source_directories()?, Purpose::Reconcile)
                .await?;
            return Ok(());
        }
        if matches!(
            event.kind,
            EventKind::Modify(ModifyKind::Name(RenameMode::Both))
        ) && event.paths.len() == 2
        {
            let source = &event.paths[0];
            let destination = &event.paths[1];
            if !source.try_exists().map_err(|error| error.to_string())?
                && destination
                    .try_exists()
                    .map_err(|error| error.to_string())?
                && crate::features::media::catalog_paths::scan_excluded_root(source).is_none()
                && crate::features::media::catalog_paths::scan_excluded_root(destination).is_none()
                && self
                    .state
                    .media_scans
                    .policy()?
                    .scope
                    .as_ref()
                    .is_none_or(|scope| scope.contains(destination))
            {
                let items = self.path_identities(source)?;
                self.state
                    .files
                    .external_move(source, destination, items)
                    .await
                    .map_err(|error| error.to_string())?;
                if destination.is_dir() {
                    if !self.state.media_scans.wait_idle().await? {
                        return Ok(());
                    }
                    self.scan_directories(
                        vec![destination.to_string_lossy().into_owned()],
                        Purpose::Reconcile,
                    )
                    .await?;
                } else {
                    self.state
                        .file_platform
                        .scan(&[destination.to_string_lossy().into_owned()])
                        .await
                        .map_err(|error| error.to_string())?;
                }
                return Ok(());
            }
        }
        let policy = self.state.media_scans.policy()?;
        for path in event.paths {
            if crate::features::media::catalog_paths::scan_excluded_root(&path).is_some() {
                continue;
            }
            if policy
                .scope
                .as_ref()
                .is_some_and(|scope| !scope.contains(&path))
            {
                continue;
            }
            let excluded = crate::features::media::catalog_paths::is_file_excluded_at(
                &path.to_string_lossy(),
                &self.state.directory,
                &policy.cache_directory,
                &policy.excluded_roots,
            );
            let effect = effect_for(&path, &event.kind, excluded);
            if matches!(effect, Effect::Forget | Effect::RescanSubtree)
                && !self.state.media_scans.wait_idle().await?
            {
                return Ok(());
            }
            match effect {
                Effect::Index => {
                    self.state
                        .file_platform
                        .scan(&[path.to_string_lossy().into_owned()])
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Effect::Forget => {
                    if !path.try_exists().map_err(|error| error.to_string())? {
                        let items = self.path_identities(&path)?;
                        self.state
                            .files
                            .external_remove(&path, items)
                            .await
                            .map_err(|error| error.to_string())?;
                    } else if let Some(parent) = path.parent().filter(|parent| parent.is_dir()) {
                        self.scan_directories(
                            vec![parent.to_string_lossy().into_owned()],
                            Purpose::Reconcile,
                        )
                        .await?;
                    }
                }
                Effect::RescanSubtree => {
                    let root = if path.is_dir() {
                        path.as_path()
                    } else {
                        path.parent().ok_or("Media directory parent required")?
                    };
                    self.scan_directories(
                        vec![root.to_string_lossy().into_owned()],
                        Purpose::Reconcile,
                    )
                    .await?;
                }
                Effect::Ignore => {}
            }
        }
        Ok(())
    }

    #[cfg(feature = "runtime-media")]
    fn path_identities(
        &self,
        root: &std::path::Path,
    ) -> Result<Vec<crate::features::services::files::file_task_media::Identity>, String> {
        let root = root.to_str().ok_or("Invalid media path encoding")?;
        let prefix = format!("{}/", root.trim_end_matches('/'));
        self.state.db.with_conn(|connection| {
            connection.prepare("SELECT data_type,id,path FROM file_catalog WHERE missing=0 AND is_available=1 AND trashed=0 AND (path=?1 OR substr(path,1,length(?2))=?2)")?
                .query_map(plain_libs::rusqlite::params![root,prefix], |row| Ok(crate::features::services::files::file_task_media::Identity { media_type:row.get(0)?, file_id:row.get(1)?, path:row.get(2)? }))?
                .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
        }).map_err(|error| error.to_string())
    }

    #[cfg(feature = "runtime-media")]
    pub async fn scan_source_directories(
        &self,
        root: &str,
    ) -> Result<crate::features::services::files::media_scans::Snapshot, String> {
        self.scan_directories(self.source_references(root)?, Purpose::Rebuild)
            .await
    }

    #[cfg(feature = "runtime-media")]
    fn source_references(&self, root: &str) -> Result<Vec<String>, String> {
        let sources = self.source_directories()?;
        let roots =
            crate::features::media::catalog_paths::selected_roots(root.to_owned(), &sources);
        plain_libs::filesystem::roots::prune_nested(roots, |root| {
            root.ends_with("Library/Mobile Documents/com~apple~CloudDocs")
        })
        .into_iter()
        .map(|path| {
            path.into_os_string()
                .into_string()
                .map_err(|_| "Invalid media directory path encoding".to_string())
        })
        .collect::<Result<Vec<_>, _>>()
    }

    #[cfg(feature = "runtime-media")]
    pub fn start_source_scan(
        &self,
        root: &str,
    ) -> Result<crate::features::services::files::media_scans::Snapshot, String> {
        self.start_scan(
            self.source_references(root)?,
            self.state.media_scans.policy()?,
            false,
        )
    }

    #[cfg(feature = "runtime-media")]
    pub fn start_volume_scan(
        &self,
        id: &str,
    ) -> Result<Option<crate::features::services::files::media_scans::Snapshot>, String> {
        let mut policy = (*self.state.media_scans.policy()?).clone();
        let scope = policy
            .scope
            .take()
            .ok_or("Index scope not configured")?
            .for_volume(id)?;
        if scope.directories.is_empty() {
            return Ok(None);
        }
        let references = scope.directories.clone();
        policy.scope = Some(scope);
        self.start_scan(references, std::sync::Arc::new(policy), false)
            .map(Some)
    }

    #[cfg(feature = "runtime-media")]
    fn start_scan(
        &self,
        references: Vec<String>,
        policy: std::sync::Arc<crate::features::services::files::media_scan_policy::Policy>,
        background: bool,
    ) -> Result<crate::features::services::files::media_scans::Snapshot, String> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|error| error.to_string())?;
        let session = self.state.media_scans.begin_discovery(Purpose::Rebuild)?;
        let catalog = self.clone();
        self.state
            .media_scans
            .spawn(&runtime, session, move |session| {
                Box::pin(async move {
                    if let Err(error) = catalog
                        .scan_directory_session(references, policy, session, background)
                        .await
                    {
                        log::error!("Media scan task failed: {error}");
                    }
                })
            })
    }

    #[cfg(feature = "runtime-media")]
    pub async fn scan_directories(
        &self,
        references: Vec<String>,
        purpose: Purpose,
    ) -> Result<crate::features::services::files::media_scans::Snapshot, String> {
        self.state.native_client_id()?;
        let policy = self.state.media_scans.policy()?;
        let session = self.state.media_scans.begin_discovery(purpose)?;
        let snapshot = self
            .scan_directory_session(references, policy, session, purpose == Purpose::Reconcile)
            .await?;
        if purpose == Purpose::Reconcile
            && let Some(error) = &snapshot.error
        {
            return Err(error.clone());
        }
        Ok(snapshot)
    }

    #[cfg(feature = "runtime-media")]
    async fn scan_directory_session(
        &self,
        references: Vec<String>,
        policy: std::sync::Arc<crate::features::services::files::media_scan_policy::Policy>,
        session: crate::features::services::files::media_scans::Session,
        background: bool,
    ) -> Result<crate::features::services::files::media_scans::Snapshot, String> {
        let mut cancel = session.cancel.clone();
        let mut stop = self.state.stop.clone();
        let worker_cancel = cancel.clone();
        let worker_stop = stop.clone();
        use crate::features::{
            media::catalog_paths::requires_foreground_scan,
            services::files::media_scans::CoverageState,
        };
        session.sources(&references)?;
        let result = async {
            let ready = self.state.media_watch.lock()
                .map_err(|_| "Media watcher unavailable")?
                .as_ref()
                .map(|watcher| watcher.ready.clone());
            if let Some(mut ready) = ready {
                ready.wait_for(|ready| *ready).await
                    .map_err(|_| "Media watcher stopped")?;
            }
            self.state
                .db
                .refresh_volume_availability()
                .map_err(|error| error.to_string())?;
            let volumes = self.state.db.volumes().map_err(|error| error.to_string())?;
            let unavailable_volumes = volumes
                .iter()
                .filter(|volume| {
                    !volume.is_available
                        && !volumes
                            .iter()
                            .any(|current| current.is_available && current.path == volume.path)
                })
                .map(|volume| std::path::PathBuf::from(&volume.path))
                .collect::<Vec<_>>();
            let mut total_files = 0usize;
            let mut prepared_roots = Vec::new();
            let mut completed_roots = Vec::new();
            let scan_id = crate::features::record_id::generate();
            let mut skipped_scan_roots = Vec::new();
            let mut media_queue = Vec::new();
            let mut errors = Vec::new();
            let mut discovery_failed = false;
            for reference in references {
                session.wait_running().await?;
                if background && requires_foreground_scan(std::path::Path::new(&reference)) {
                    skipped_scan_roots.push(std::path::PathBuf::from(&reference));
                    session.source_state(&reference, CoverageState::Excluded, None)?;
                    continue;
                }
                if policy
                    .scope
                    .as_ref()
                    .is_some_and(|scope| !scope.contains(std::path::Path::new(&reference)))
                {
                    session.source_state(&reference, CoverageState::Excluded, None)?;
                    continue;
                }
                session.source_state(&reference, CoverageState::Scanning, None)?;
                let root = match crate::features::services::files::file_access::resolve_native(
                    &self.state,
                    &reference,
                )
                .await
                {
                    Ok(path) => path,
                    Err(error)
                        if error.downcast_ref::<std::io::Error>().is_some_and(|error| {
                            error.kind() == std::io::ErrorKind::PermissionDenied
                        }) =>
                    {
                        session.source_state(&reference, CoverageState::Excluded, None)?;
                        continue;
                    }
                    Err(error) => {
                        let error = error.to_string();
                        session.source_state(
                            &reference,
                            CoverageState::Failed,
                            Some(error.clone()),
                        )?;
                        discovery_failed = true;
                        errors.push(error);
                        continue;
                    }
                };
                let canonical_root = match tokio::fs::canonicalize(&root).await {
                    Ok(path) if background && requires_foreground_scan(&path) => {
                        skipped_scan_roots.push(path);
                        session.source_state(&reference, CoverageState::Excluded, None)?;
                        continue;
                    }
                    Ok(path) if path.is_dir() => path,
                    Ok(_) => {
                        let error = "Real scan directory required".to_string();
                        session.source_state(
                            &reference,
                            CoverageState::Failed,
                            Some(error.clone()),
                        )?;
                        discovery_failed = true;
                        errors.push(error);
                        continue;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                        session.source_state(&reference, CoverageState::Excluded, None)?;
                        continue;
                    }
                    Err(error) => {
                        let error = format!("{reference}: {error}");
                        session.source_state(
                            &reference,
                            CoverageState::Failed,
                            Some(error.clone()),
                        )?;
                        discovery_failed = true;
                        errors.push(error);
                        continue;
                    }
                };
                let excluded =
                    crate::features::media::catalog_paths::scan_excluded_root(&canonical_root)
                        .is_some();
                let current_volume = match plain_libs::platform::volumes::locate(&canonical_root) {
                    Ok(location) => location.volume,
                    Err(error) => {
                        let error = format!("{reference}: {error}");
                        session.source_state(
                            &reference,
                            CoverageState::Failed,
                            Some(error.clone()),
                        )?;
                        discovery_failed = true;
                        errors.push(error);
                        continue;
                    }
                };
                if unavailable_volumes.iter().any(|volume| {
                    canonical_root.starts_with(volume) && current_volume.path != *volume
                }) {
                    let error = format!("File source volume is offline: {reference}");
                    session.source_state(&reference, CoverageState::Failed, Some(error.clone()))?;
                    discovery_failed = true;
                    errors.push(error);
                    continue;
                }
                if excluded {
                    session.source_state(&reference, CoverageState::Excluded, None)?;
                    completed_roots.push((canonical_root, current_volume));
                } else {
                    prepared_roots.push((reference, canonical_root, current_volume));
                }
            }
            let directory = tokio::fs::canonicalize(&self.state.directory)
                .await
                .map_err(|error| error.to_string())?;
            let mut counted_roots = Vec::new();
            for (reference, root, volume) in prepared_roots {
                let (mut receive, producer) = directory_walk::entries(
                    root.clone(),
                    directory.clone(),
                    policy.clone(),
                    background,
                    worker_cancel.clone(),
                    worker_stop.clone(),
                );
                let mut total = 0usize;
                while let Some(path) = receive.recv().await {
                    session.wait_running().await?;
                    total = total
                        .checked_add(usize::from(path.is_file()))
                        .ok_or("Media scan file count overflow")?;
                }
                match producer.await.map_err(|error| error.to_string())? {
                    Ok(roots) => skipped_scan_roots.extend(roots),
                    Err(error) => {
                        session.source_state(
                            &reference,
                            CoverageState::Failed,
                            Some(error.clone()),
                        )?;
                        discovery_failed = true;
                        errors.push(error);
                        continue;
                    }
                }
                total_files = total_files
                    .checked_add(total)
                    .ok_or("Media scan file count overflow")?;
                counted_roots.push((reference, root, volume));
            }
            if !discovery_failed {
                session.discovered(total_files)?;
            }
            for (reference, canonical_root, volume) in counted_roots {
                let (mut receive, producer) = directory_walk::entries(
                    canonical_root.clone(),
                    directory.clone(),
                    policy.clone(),
                    background,
                    worker_cancel.clone(),
                    worker_stop.clone(),
                );
                let mut paths = Vec::with_capacity(SCAN_BATCH_SIZE);
                let mut processed = 0;
                while let Some(path) = receive.recv().await {
                    session.wait_running().await?;
                    let actual = match tokio::fs::canonicalize(&path).await {
                        Ok(path) => path,
                        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                            skipped_scan_roots.push(path);
                            continue;
                        }
                        Err(error) => return Err(error.to_string()),
                    };
                    if !actual.starts_with(&canonical_root) {
                        return Err("Media file escaped scan directory".into());
                    }
                    let path = actual
                        .to_str()
                        .ok_or("Invalid file path encoding")?
                        .to_owned();
                    let file_count = usize::from(actual.is_file());
                    let data_type = if file_count > 0
                        && !crate::features::media::catalog_paths::is_media_excluded_at(
                            &path,
                            &directory,
                            &policy.cache_directory,
                            &policy.excluded_roots,
                        ) {
                        let kind = crate::features::media::catalog_paths::infer_type(&path);
                        [
                            DataType::Audio,
                            DataType::Video,
                            DataType::Image,
                            DataType::Doc,
                        ]
                        .into_iter()
                        .find(|data_type| data_type.media_type_str() == Some(kind))
                        .unwrap_or(DataType::File)
                    } else {
                        DataType::File
                    };
                    let kind = crate::infrastructure::db::FileKind::try_from(data_type)
                        .map_err(|error| error.to_string())?;
                    paths.push((path, kind));
                    if data_type == DataType::File {
                        processed += file_count;
                    }
                    if paths.len() == SCAN_BATCH_SIZE {
                        session
                            .record_when_running(processed, || {
                                self.persist_scan_paths(&paths, &scan_id)
                            })
                            .await?;
                        paths.clear();
                        processed = 0;
                    }
                    if data_type != DataType::File {
                        media_queue.push((data_type, actual));
                    }
                }
                if !paths.is_empty() {
                    session
                        .record_when_running(processed, || {
                            self.persist_scan_paths(&paths, &scan_id)
                        })
                        .await?;
                }
                match producer.await.map_err(|error| error.to_string())? {
                    Ok(roots) => skipped_scan_roots.extend(roots),
                    Err(error) => {
                        session.source_state(
                            &reference,
                            CoverageState::Failed,
                            Some(error.clone()),
                        )?;
                        errors.push(error);
                        continue;
                    }
                };
                session.source_state(&reference, CoverageState::Completed, None)?;
                completed_roots.push((canonical_root, volume));
            }
            let mut metadata_batch = Vec::with_capacity(SCAN_BATCH_SIZE);
            let mut last_metadata_flush = std::time::Instant::now();
            for (data_type, path) in media_queue {
                session.wait_running().await?;
                match self
                    .read_file(
                        data_type,
                        path.to_str().ok_or("Invalid media file path encoding")?,
                    )
                    .await
                {
                    Ok(metadata) => {
                        metadata_batch.push(metadata);
                        if metadata_batch.len() == SCAN_BATCH_SIZE
                            || last_metadata_flush.elapsed() >= std::time::Duration::from_secs(1)
                        {
                            session
                                .record_when_running(metadata_batch.len(), || {
                                    self.persist_scan_metadata(
                                        &metadata_batch,
                                        policy.scope.as_ref(),
                                    )
                                })
                                .await?;
                            metadata_batch.clear();
                            last_metadata_flush = std::time::Instant::now();
                        }
                    }
                    Err(error) => errors.push(format!("{}: {error}", path.display())),
                }
            }
            if !metadata_batch.is_empty() {
                session
                    .record_when_running(metadata_batch.len(), || {
                        self.persist_scan_metadata(&metadata_batch, policy.scope.as_ref())
                    })
                    .await?;
            }
            for (root, volume) in completed_roots {
                if !tokio::fs::metadata(&root)
                    .await
                    .map_err(|error| error.to_string())?
                    .is_dir()
                {
                    return Err("Media source became unavailable during reconciliation".into());
                }
                if plain_libs::platform::volumes::locate(&root)
                    .map_err(|error| error.to_string())?
                    .volume
                    != volume
                {
                    return Err("Media source volume changed during reconciliation".into());
                }
                let mut after = String::new();
                loop {
                    let mut removed = 0;
                    session
                        .record_when_running(0, || {
                            let types = crate::features::files::deletes::reconcile_scan_batch(
                                &self.state.db,
                                &root,
                                &scan_id,
                                &skipped_scan_roots,
                                SCAN_BATCH_SIZE as u32,
                                &after,
                            )
                            .map_err(|error| error.to_string())?;
                            removed = types.len();
                            if let Some(last) = types.last() {
                                after = last.id.clone();
                            }
                            self.state
                                .media_scans
                                .publish_catalog_types(types.iter().map(|item| item.media_type));
                            Ok(())
                        })
                        .await?;
                    if removed < SCAN_BATCH_SIZE {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            }
            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors.join("; "))
            }
        };
        let result = tokio::select! {
            biased;
            _ = crate::features::services::files::media_scans::wait_stop(&mut cancel) => Err("Media scan cancelled".into()),
            _ = crate::features::services::files::media_scans::wait_stop(&mut stop) => {
                self.state.media_scans.cancel()?;
                Err("Rust core stopped".into())
            },
            result = result => result,
        };
        session.finish(result)
    }

    #[cfg(feature = "runtime-media")]
    pub fn pause_file_scan(
        &self,
    ) -> Result<Option<crate::features::services::files::media_scans::Snapshot>, String> {
        self.state.native_client_id()?;
        self.state.media_scans.pause()
    }

    #[cfg(feature = "runtime-media")]
    pub fn resume_file_scan(
        &self,
    ) -> Result<Option<crate::features::services::files::media_scans::Snapshot>, String> {
        self.state.native_client_id()?;
        self.state.media_scans.resume()
    }

    #[cfg(feature = "runtime-media")]
    pub fn cancel_file_scan(
        &self,
    ) -> Result<Option<crate::features::services::files::media_scans::Snapshot>, String> {
        self.state.native_client_id()?;
        self.state.media_scans.cancel()
    }

    #[cfg(feature = "runtime-media")]
    pub fn file_scan_status(
        &self,
    ) -> Result<Option<crate::features::services::files::media_scans::Snapshot>, String> {
        self.state.native_client_id()?;
        self.state.media_scans.snapshot()
    }

    pub fn record_files(&self, files: &[MediaFileMetadata]) -> Result<Vec<MediaFileRow>, String> {
        self.state.native_client_id()?;
        let rows = self
            .state
            .db
            .record_media_files(files)
            .map_err(|error| error.to_string())?;
        #[cfg(feature = "runtime-media")]
        self.state.media_scans.record_with_scope(|scope| {
            if let Some(scope) = scope {
                let paths = files
                    .iter()
                    .map(|file| file.path.clone())
                    .collect::<Vec<_>>();
                self.state
                    .db
                    .update_index_visibility(scope, Some(&paths))
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        })?;
        crate::features::services::files::catalog_events::publish(
            &self.state.events,
            rows.iter().map(|row| row.data_type),
        );
        Ok(rows)
    }

    pub fn record_file_reads(
        &self,
        files: &[crate::infrastructure::db::MediaFileMetadataRead],
    ) -> Result<Vec<MediaFileRow>, String> {
        #[cfg(feature = "runtime-media")]
        let rows = self
            .state
            .media_scans
            .record_with_scope(|scope| self.persist_file_reads(files, scope))?;
        #[cfg(not(feature = "runtime-media"))]
        let rows = self.persist_file_reads(files, None)?;
        crate::features::services::files::catalog_events::publish(
            &self.state.events,
            rows.iter().map(|row| row.data_type),
        );
        Ok(rows)
    }

    #[cfg(feature = "runtime-media")]
    fn persist_scan_paths(
        &self,
        paths: &[(String, crate::infrastructure::db::FileKind)],
        scan_id: &str,
    ) -> Result<(), String> {
        self.state
            .db
            .record_scan_paths(paths, scan_id)
            .map_err(|error| error.to_string())?;
        self.state
            .media_scans
            .publish_catalog_types(paths.iter().map(|(_, kind)| kind.data_type().kind()));
        Ok(())
    }

    #[cfg(feature = "runtime-media")]
    fn persist_scan_metadata(
        &self,
        files: &[crate::infrastructure::db::MediaFileMetadataRead],
        scope: Option<&crate::features::files::index_scope::Scope>,
    ) -> Result<(), String> {
        let rows = self.persist_file_reads(files, scope)?;
        self.state
            .media_scans
            .publish_catalog_types(rows.iter().map(|row| row.data_type));
        Ok(())
    }

    fn persist_file_reads(
        &self,
        files: &[crate::infrastructure::db::MediaFileMetadataRead],
        scope: Option<&crate::features::files::index_scope::Scope>,
    ) -> Result<Vec<MediaFileRow>, String> {
        self.state.native_client_id()?;
        let rows = self
            .state
            .db
            .record_media_file_reads(files)
            .map_err(|error| error.to_string())?;
        if let Some(scope) = scope {
            let paths = files
                .iter()
                .map(|file| file.metadata.path.clone())
                .collect::<Vec<_>>();
            self.state
                .db
                .update_index_visibility(scope, Some(&paths))
                .map_err(|error| error.to_string())?;
        }
        Ok(rows)
    }

    #[cfg(test)]
    fn forget_path(&self, path: &str) -> Result<usize, String> {
        self.state.native_client_id()?;
        let types = self
            .state
            .db
            .forget_media_path_types(path)
            .map_err(|error| error.to_string())?;
        let count = types.len();
        crate::features::services::files::catalog_events::publish(&self.state.events, types);
        Ok(count)
    }

    pub fn by_path(&self, data_type: DataType, path: &str) -> Result<Option<MediaFileRow>, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .media_file_by_path(data_type, path)
            .map_err(|error| error.to_string())
    }

    pub fn list(
        &self,
        data_type: DataType,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MediaFileRow>, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .media_file_list(data_type, limit, offset)
            .map_err(|error| error.to_string())
    }

    pub fn search(
        &self,
        data_type: DataType,
        query: &str,
        sort: crate::features::prefs::settings::FileSortBy,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MediaFileRow>, String> {
        self.state.native_client_id()?;
        crate::features::services::files::media_catalog::rows(
            &self.state.db,
            data_type,
            query,
            sort,
            limit,
            offset,
        )
        .map_err(|error| error.to_string())
    }

    pub fn count_matching(&self, data_type: DataType, query: &str) -> Result<i64, String> {
        self.state.native_client_id()?;
        crate::features::services::files::media_catalog::count(&self.state.db, data_type, query)
            .map_err(|error| error.to_string())
    }

    pub fn count(&self, data_type: DataType) -> Result<i64, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .media_file_count(data_type)
            .map_err(|error| error.to_string())
    }
}

#[cfg(all(test, feature = "runtime-media"))]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/catalog_scan.rs"]
mod tests;
