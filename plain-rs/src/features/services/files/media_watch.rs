use std::path::Path;

use notify::event::{EventKind, ModifyKind};
use notify::{Event, RecursiveMode, Watcher};

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
        use notify::event::{AccessKind, AccessMode, CreateKind};
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

pub struct WatcherHandle {
    task: tokio::task::JoinHandle<()>,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn start<F, Pending>(
    roots: &[std::path::PathBuf],
    scope: Option<crate::features::prefs::file_index::Scope>,
    on_event: F,
) -> Result<WatcherHandle, String>
where
    F: Fn(Event) -> Pending + Send + 'static,
    Pending: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    use notify::event::RenameMode;
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
    let mut watcher =
        notify::recommended_watcher(move |result: notify::Result<Event>| match result {
            Ok(event) => match send.try_send(event) {
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
        })
        .map_err(|error| error.to_string())?;
    let mut watched = std::collections::BTreeSet::new();
    for root in roots {
        if let Err(error) = watch_tree(&mut watcher, &mut watched, root, scope.as_ref()) {
            log::warn!("File watch source: {error}");
            dirty.store(true, Ordering::Release);
        }
    }
    let roots = roots.to_vec();
    let task = runtime.spawn(async move {
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
                                if let Err(error) = watcher.unwatch(&path) {
                                    if !matches!(error.kind, notify::ErrorKind::WatchNotFound | notify::ErrorKind::PathNotFound) { log::warn!("File watch removal: {error}"); }
                                }
                                watched.remove(&path);
                            }
                            match watch_tree(&mut watcher, &mut watched, root, scope.as_ref()) {
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
                    index.map(|index| { let mut event = pending.remove(index).0; event.kind = EventKind::Remove(notify::event::RemoveKind::Any); event })
                },
                event = receive.recv() => match event { Some(event) => Some(event), None => return },
            };
            if dirty.swap(false, Ordering::AcqRel) {
                for root in &roots {
                    if let Err(error) = watch_tree(&mut watcher, &mut watched, root, scope.as_ref()) {
                        log::warn!("File watch reconciliation: {error}");
                        dirty.store(true, Ordering::Release);
                    }
                }
                let mut rescan = Event::new(EventKind::Other);
                rescan.paths = roots.clone();
                if let Err(error) = on_event(rescan).await {
                    log::warn!("Media watcher reconciliation: {error}");
                    dirty.store(true, Ordering::Release);
                }
            }
            let Some(event) = event.and_then(completed_event) else { continue };
            if let Some(scope) = &scope {
                if event.paths.iter().all(|path| !scope.contains(path)) { continue; }
            }
            for path in &event.paths {
                if path.is_dir() && !watched.contains(path) {
                    if let Err(error) = watch_tree(&mut watcher, &mut watched, path, scope.as_ref()) {
                        log::warn!("File watch directory: {error}");
                        dirty.store(true, Ordering::Release);
                    }
                }
            }
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
    });
    Ok(WatcherHandle { task })
}

fn root_key(root: &Path) -> Result<Option<String>, String> {
    match crate::libs::utils::file_identity::key(root) {
        Ok(key) => Ok(key),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", root.display())),
    }
}

fn watch_tree(
    watcher: &mut dyn Watcher,
    watched: &mut std::collections::BTreeSet<std::path::PathBuf>,
    root: &Path,
    scope: Option<&crate::features::prefs::file_index::Scope>,
) -> Result<(), String> {
    if !root.is_dir() || scope.is_some_and(|scope| !scope.contains(root)) {
        return Ok(());
    }
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        if !watched.iter().any(|path| root.starts_with(path)) {
            watcher
                .watch(root, RecursiveMode::Recursive)
                .map_err(|error| error.to_string())?;
            watched.insert(root.to_owned());
        }
        return Ok(());
    }
    let mut pending = vec![root.to_owned()];
    let mut failure = None;
    while let Some(path) = pending.pop() {
        if scope.is_some_and(|scope| !scope.contains_entry(&path, true)) {
            continue;
        }
        if !watched.contains(&path) {
            match watcher.watch(&path, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    watched.insert(path.clone());
                }
                Err(error) => {
                    failure = Some(error.to_string());
                }
            }
        }
        match std::fs::read_dir(&path) {
            Ok(entries) => {
                for entry in entries {
                    match entry.and_then(|entry| Ok((entry.file_type()?, entry.path()))) {
                        Ok((kind, path)) if kind.is_dir() => pending.push(path),
                        Ok(_) => {}
                        Err(error) => {
                            failure = Some(error.to_string());
                        }
                    }
                }
            }
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
