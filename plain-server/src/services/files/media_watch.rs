use std::path::Path;

use notify::event::{EventKind, ModifyKind};
use notify::{Event, RecursiveMode, Watcher};
use plain_rs::enums::DataType;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Effect {
    Index,
    Forget,
    RescanSubtree,
    Ignore,
}

fn media_type_of(path: &Path) -> Option<DataType> {
    let name = path.file_name()?.to_str()?;
    let kind = plain_rs::media::catalog_paths::infer_type(name);
    [
        DataType::Audio,
        DataType::Video,
        DataType::Image,
        DataType::Doc,
    ]
    .into_iter()
    .find(|data_type| data_type.media_type_str() == Some(kind))
}

pub(crate) fn effect_for(path: &Path, kind: &EventKind, is_excluded: bool) -> Effect {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Effect::Ignore;
    };
    if name == ".nomedia" {
        return Effect::RescanSubtree;
    }
    if name.starts_with('.') {
        return Effect::Ignore;
    }
    match kind {
        EventKind::Create(_) => {
            if path.is_file() && media_type_of(path).is_some() {
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
            } else if path.is_file() && media_type_of(path).is_some() && !is_excluded {
                Effect::Index
            } else {
                Effect::Forget
            }
        }
        EventKind::Modify(_) => {
            if path.is_file() && media_type_of(path).is_some() {
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

pub(crate) struct WatcherHandle {
    _watcher: Box<dyn Watcher + Send>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(crate) fn start<F, Pending>(
    roots: &[std::path::PathBuf],
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
    for root in roots {
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|error| format!("watch {}: {error}", root.display()))?;
    }
    let roots = roots.to_vec();
    let task = runtime.spawn(async move {
        let mut pending: Vec<(Event, tokio::time::Instant)> = Vec::new();
        loop {
            let deadline = pending.iter().map(|(_, deadline)| *deadline).min()
                .unwrap_or_else(|| tokio::time::Instant::now() + Duration::from_secs(60));
            let event = tokio::select! {
                _ = changed.notified() => None,
                _ = tokio::time::sleep(Duration::from_secs(5)), if dirty.load(Ordering::Acquire) => None,
                _ = tokio::time::sleep_until(deadline), if !pending.is_empty() => {
                    let index = pending.iter().position(|(_, end)| *end <= tokio::time::Instant::now());
                    index.map(|index| { let mut event = pending.remove(index).0; event.kind = EventKind::Remove(notify::event::RemoveKind::Any); event })
                },
                event = receive.recv() => match event { Some(event) => Some(event), None => return },
            };
            if dirty.swap(false, Ordering::AcqRel) {
                let mut rescan = Event::new(EventKind::Other);
                rescan.paths = roots.clone();
                if let Err(error) = on_event(rescan).await {
                    log::warn!("Media watcher reconciliation: {error}");
                    dirty.store(true, Ordering::Release);
                }
            }
            let Some(event) = event.and_then(completed_event) else { continue };
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
    Ok(WatcherHandle {
        _watcher: Box::new(watcher),
        task,
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/media_watch.rs"]
mod tests;
