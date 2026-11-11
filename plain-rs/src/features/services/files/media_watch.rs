use crate::features::media::catalog_paths::requires_foreground_scan;
use std::path::Path;

use plain_libs::file_events::event::{EventKind, ModifyKind};
use plain_libs::file_events::{Event, RecursiveMode, Watcher};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Index,
    Forget,
    RescanSubtree,
    Ignore,
}

pub fn effect_for(path: &Path, kind: &EventKind, is_excluded: bool) -> Effect {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Effect::Ignore;
    };
    if name == ".nomedia" {
        return Effect::RescanSubtree;
    }
    match kind {
        EventKind::Create(_) => {
            if path.is_file() {
                if is_excluded {
                    Effect::Forget
                } else {
                    Effect::Index
                }
            } else if path.is_dir() {
                Effect::RescanSubtree
            } else {
                Effect::Ignore
            }
        }
        EventKind::Remove(_) => Effect::Forget,
        EventKind::Modify(ModifyKind::Name(_)) => {
            if path.is_dir() {
                Effect::RescanSubtree
            } else if path.is_file() && !is_excluded {
                Effect::Index
            } else {
                Effect::Forget
            }
        }
        EventKind::Modify(_) => {
            if path.is_file() {
                if is_excluded {
                    Effect::Forget
                } else {
                    Effect::Index
                }
            } else {
                Effect::Ignore
            }
        }
        _ => Effect::Ignore,
    }
}

fn completed_event(event: Event) -> Option<Event> {
    #[cfg(target_os = "linux")]
    let mut event = event;
    #[cfg(target_os = "linux")]
    {
        use plain_libs::file_events::event::{AccessKind, AccessMode, CreateKind};
        match event.kind {
            EventKind::Create(CreateKind::File) | EventKind::Modify(ModifyKind::Data(_)) => {
                return None;
            }
            EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
                event.kind = EventKind::Modify(ModifyKind::Any);
            }
            EventKind::Access(_) => return None,
            _ => {}
        }
    }
    Some(event)
}

fn enqueue_event(
    send: &tokio::sync::mpsc::Sender<Event>,
    scope: Option<&crate::features::files::index_scope::Scope>,
    mut event: Event,
) -> Result<(), tokio::sync::mpsc::error::TrySendError<Event>> {
    if !event.need_rescan() {
        let Some(completed) = completed_event(event) else {
            return Ok(());
        };
        event = completed;
        if !event.paths.is_empty() {
            event.paths.retain(|path| !requires_foreground_scan(path));
            if event.paths.iter().all(|path| {
                crate::features::media::catalog_paths::scan_excluded_root(path).is_some()
                    || scope.is_some_and(|scope| !scope.contains(path))
            }) {
                return Ok(());
            }
        }
    }
    send.try_send(event)
}

pub struct WatcherHandle {
    task: tokio::task::JoinHandle<()>,
    stop: tokio::sync::watch::Sender<bool>,
    pub(super) ready: tokio::sync::watch::Receiver<bool>,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        self.task.abort();
    }
}

pub fn start<F, Pending>(
    roots: &[std::path::PathBuf],
    scope: Option<crate::features::files::index_scope::Scope>,
    on_event: F,
) -> Result<WatcherHandle, String>
where
    F: Fn(Event) -> Pending + Send + 'static,
    Pending: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    start_with_watcher(
        roots,
        scope,
        plain_libs::file_events::recommended_watcher,
        on_event,
    )
}

fn start_with_watcher<W, F, Pending>(
    roots: &[std::path::PathBuf],
    scope: Option<crate::features::files::index_scope::Scope>,
    create: impl FnOnce(
        Box<dyn FnMut(plain_libs::file_events::Result<Event>) + Send>,
    ) -> plain_libs::file_events::Result<W>,
    on_event: F,
) -> Result<WatcherHandle, String>
where
    W: Watcher + Send + 'static,
    F: Fn(Event) -> Pending + Send + 'static,
    Pending: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    use plain_libs::file_events::event::RenameMode;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;
    let runtime = tokio::runtime::Handle::try_current().map_err(|error| error.to_string())?;
    let (send, mut receive) = tokio::sync::mpsc::channel::<Event>(256);
    let dirty = Arc::new(AtomicBool::new(false));
    let changed = Arc::new(tokio::sync::Notify::new());
    let overflow = dirty.clone();
    let wake = changed.clone();
    let event_scope = scope.clone();
    let mut watcher = create(Box::new(
        move |result: plain_libs::file_events::Result<Event>| match result {
            Ok(event) if event.need_rescan() => {
                overflow.store(true, Ordering::Release);
                wake.notify_one();
            }
            Ok(event) => match enqueue_event(&send, event_scope.as_ref(), event) {
                Ok(()) => {}
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                    overflow.store(true, Ordering::Release);
                    wake.notify_one();
                }
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {}
            },
            Err(error) => {
                log::warn!("Media watcher: {error}");
                overflow.store(true, Ordering::Release);
                wake.notify_one();
            }
        },
    ))
    .map_err(|error| error.to_string())?;
    let mut watched = std::collections::BTreeSet::new();
    let roots = roots
        .iter()
        .filter(|path| !requires_foreground_scan(path))
        .cloned()
        .collect::<Vec<_>>();
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let (started, ready) = tokio::sync::watch::channel(false);
    let watch_stop = stopped.clone();
    let worker_runtime = runtime.clone();
    let task = runtime.spawn_blocking(move || worker_runtime.block_on(async move {
        let run = async move {
            for root in &roots {
                if let Err(error) = watch_tree(&mut watcher, &mut watched, root, scope.as_ref(), &watch_stop) {
                    log::warn!("File watch source: {error}");
                    dirty.store(true, Ordering::Release);
                }
            }
            if *watch_stop.borrow() { return; }
            started.send_replace(true);
            let mut root_keys: Vec<_> = roots.iter().map(|root| root_key(root)).collect();
            for error in root_keys.iter().filter_map(|key| key.as_ref().err()) { log::warn!("File watch source: {error}"); }
            let mut refresh = tokio::time::interval(Duration::from_secs(60));
            refresh.tick().await;
            let mut pending: Vec<(Event, tokio::time::Instant)> = Vec::new();
            loop {
                let deadline = pending.iter().map(|(_, deadline)| *deadline).min()
                    .unwrap_or_else(|| tokio::time::Instant::now() + Duration::from_secs(60));
                let event = tokio::select! {
                    _ = refresh.tick() => {
                        for (index, root) in roots.iter().enumerate() {
                            let key = root_key(root);
                            if key != root_keys[index] {
                                let stale: Vec<_> = watched.iter().filter(|path| path.starts_with(root)).cloned().collect();
                                for path in stale {
                                    if *watch_stop.borrow() { return; }
                                    if let Err(error) = watcher.unwatch(&path) {
                                        if !matches!(error.kind, plain_libs::file_events::ErrorKind::WatchNotFound | plain_libs::file_events::ErrorKind::PathNotFound) { log::warn!("File watch removal: {error}"); }
                                    }
                                    watched.remove(&path);
                                }
                                match watch_tree(&mut watcher, &mut watched, root, scope.as_ref(), &watch_stop) {
                                    Ok(()) => { root_keys[index] = key; },
                                    Err(error) => log::warn!("File watch reconnect: {error}"),
                                }
                                dirty.store(true, Ordering::Release);
                            }
                        }
                        None
                    },
                    _ = changed.notified() => None,
                    _ = tokio::time::sleep(Duration::from_secs(5)), if dirty.load(Ordering::Acquire) => None,
                    _ = tokio::time::sleep_until(deadline), if !pending.is_empty() => {
                        let index = pending.iter().position(|(_, end)| *end <= tokio::time::Instant::now());
                        index.map(|index| { let mut event = pending.remove(index).0; event.kind = EventKind::Remove(plain_libs::file_events::event::RemoveKind::Any); event })
                    },
                    event = receive.recv() => match event { Some(event) => Some(event), None => return },
                };
                if dirty.swap(false, Ordering::AcqRel) {
                    for root in &roots {
                        if let Err(error) = watch_tree(&mut watcher, &mut watched, root, scope.as_ref(), &watch_stop) {
                            log::warn!("File watch reconciliation: {error}");
                            dirty.store(true, Ordering::Release);
                        }
                    }
                    if *watch_stop.borrow() { return; }
                    let mut rescan = Event::new(EventKind::Other);
                    rescan.paths = roots.clone();
                    if let Err(error) = on_event(rescan).await {
                        log::warn!("Media watcher reconciliation: {error}");
                        dirty.store(true, Ordering::Release);
                    }
                }
                let Some(event) = event else { continue };
                if let Some(scope) = &scope {
                    if event.paths.iter().all(|path| !scope.contains(path)) { continue; }
                }
                for path in &event.paths {
                    if path.is_dir() && !watched.contains(path) {
                        if let Err(error) = watch_tree(&mut watcher, &mut watched, path, scope.as_ref(), &watch_stop) {
                            log::warn!("File watch directory: {error}");
                            dirty.store(true, Ordering::Release);
                        }
                    }
                }
                if *watch_stop.borrow() { return; }
                if matches!(event.kind, EventKind::Modify(ModifyKind::Name(RenameMode::From))) {
                    if let Some(index) = pending.iter().position(|(current, _)| current.paths == event.paths) {
                        let expired = pending.remove(index).0;
                        if let Err(error) = on_event(expired).await { log::warn!("Media watcher: {error}"); dirty.store(true, Ordering::Release); }
                    }
                    pending.push((event, tokio::time::Instant::now() + Duration::from_secs(1)));
                    continue;
                }
                if matches!(event.kind, EventKind::Modify(ModifyKind::Name(RenameMode::Both))) {
                    pending.retain(|(current, _)| current.paths.first() != event.paths.first());
                }
                if let Err(error) = on_event(event).await {
                    log::warn!("Media watcher: {error}");
                    dirty.store(true, Ordering::Release);
                }
            }
        };
        tokio::select! {
            biased;
            _ = stopped.wait_for(|stopped| *stopped) => {},
            _ = run => {},
        }
    }));
    Ok(WatcherHandle { task, stop, ready })
}

fn root_key(root: &Path) -> Result<Option<String>, String> {
    match plain_libs::utils::file_identity::key(root) {
        Ok(key) => Ok(key),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(format!("{}: {error}", root.display())),
    }
}

fn watch_tree(
    watcher: &mut dyn Watcher,
    watched: &mut std::collections::BTreeSet<std::path::PathBuf>,
    root: &Path,
    scope: Option<&crate::features::files::index_scope::Scope>,
    stopped: &tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    if *stopped.borrow()
        || !root.is_dir()
        || crate::features::media::catalog_paths::scan_excluded_root(root).is_some()
        || scope.is_some_and(|scope| !scope.contains(root))
    {
        return Ok(());
    }
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        if !watched.iter().any(|path| root.starts_with(path)) {
            match watcher.watch(root, RecursiveMode::Recursive) {
                Ok(()) => {
                    watched.insert(root.to_owned());
                }
                Err(error) if matches!(&error.kind, plain_libs::file_events::ErrorKind::Io(error) if error.kind() == std::io::ErrorKind::PermissionDenied) =>
                    {}
                Err(error) => return Err(error.to_string()),
            }
        }
        return Ok(());
    }
    let mut pending = vec![root.to_owned()];
    let mut failure = None;
    while let Some(path) = pending.pop() {
        if *stopped.borrow() {
            return Ok(());
        }
        if crate::features::media::catalog_paths::scan_excluded_root(&path).is_some()
            || scope.is_some_and(|scope| !scope.contains_entry(&path, true))
        {
            continue;
        }
        if !watched.contains(&path) {
            match watcher.watch(&path, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    watched.insert(path.clone());
                }
                Err(error)
                    if matches!(&error.kind,
                    plain_libs::file_events::ErrorKind::Io(error) if matches!(error.kind(), std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound)) =>
                {
                    continue;
                }
                Err(error) => {
                    failure = Some(error.to_string());
                }
            }
        }
        if *stopped.borrow() {
            return Ok(());
        }
        match std::fs::read_dir(&path) {
            Ok(entries) => {
                for entry in entries {
                    if *stopped.borrow() {
                        return Ok(());
                    }
                    match entry.and_then(|entry| Ok((entry.file_type()?, entry.path()))) {
                        Ok((kind, path)) if kind.is_dir() => pending.push(path),
                        Ok(_) => {}
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound
                            ) => {}
                        Err(error) => {
                            failure = Some(error.to_string());
                        }
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound
                ) => {}
            Err(error) => {
                failure = Some(error.to_string());
            }
        }
    }
    failure.map_or(Ok(()), Err)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/media_watch.rs"]
mod tests;
