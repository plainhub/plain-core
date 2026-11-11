//! The event→catalog mapping a media watcher depends on. Exercised without a
//! real watcher: what matters is which filesystem change turns into which
//! catalog operation.

use super::*;
use plain_libs::file_events::event::{RemoveKind, RenameMode};

fn watch_directory_fixture() -> plain_libs::temporary::TempDir {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
    plain_libs::temporary::Builder::new()
        .prefix("media-watch-")
        .tempdir_in(home)
        .unwrap()
}

#[test]
#[cfg(target_os = "linux")]
fn read_events_and_incomplete_writes_do_not_enter_the_reconciliation_queue() {
    use plain_libs::file_events::event::{AccessKind, AccessMode, CreateKind, DataChange};
    let (send, mut receive) = tokio::sync::mpsc::channel(1);
    for kind in [
        EventKind::Access(AccessKind::Open(AccessMode::Read)),
        EventKind::Access(AccessKind::Close(AccessMode::Read)),
        EventKind::Create(CreateKind::File),
        EventKind::Modify(ModifyKind::Data(DataChange::Any)),
    ] {
        enqueue_event(&send, None, Event::new(kind).add_path("/owned/file".into())).unwrap();
        assert!(matches!(
            receive.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));
    }
    enqueue_event(
        &send,
        None,
        Event::new(EventKind::Access(AccessKind::Close(AccessMode::Write)))
            .add_path("/owned/file".into()),
    )
    .unwrap();
    assert_eq!(
        receive.try_recv().unwrap().kind,
        EventKind::Modify(ModifyKind::Any)
    );
    assert!(matches!(
        receive.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));
}

#[derive(Default)]
struct DeniedWatcher {
    denied: std::path::PathBuf,
    calls: Vec<std::path::PathBuf>,
}
impl Watcher for DeniedWatcher {
    fn new<F: plain_libs::file_events::EventHandler>(
        _: F,
    ) -> plain_libs::file_events::Result<Self> {
        Ok(Self::default())
    }
    fn watch(&mut self, path: &Path, _: RecursiveMode) -> plain_libs::file_events::Result<()> {
        self.calls.push(path.to_owned());
        if path == self.denied {
            return Err(plain_libs::file_events::Error::io(
                std::io::ErrorKind::PermissionDenied.into(),
            ));
        }
        Ok(())
    }
    fn unwatch(&mut self, _: &Path) -> plain_libs::file_events::Result<()> {
        Ok(())
    }
}

#[test]
#[cfg(target_os = "linux")]
fn an_inaccessible_watch_descendant_is_skipped_before_descending() {
    let directory = watch_directory_fixture();
    let denied = directory.path().join("denied");
    let nested = denied.join("nested");
    let readable = directory.path().join("readable");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(&readable).unwrap();
    let mut watcher = DeniedWatcher {
        denied: denied.clone(),
        ..Default::default()
    };
    let mut watched = std::collections::BTreeSet::new();
    let (_, stopped) = tokio::sync::watch::channel(false);
    watch_tree(&mut watcher, &mut watched, directory.path(), None, &stopped).unwrap();
    assert!(watched.contains(directory.path()));
    assert!(watched.contains(&readable));
    assert!(!watched.contains(&denied));
    assert!(!watcher.calls.contains(&nested));
    assert_eq!(
        watcher.calls.iter().filter(|path| *path == &denied).count(),
        1
    );
}

#[test]
fn a_new_media_file_is_indexed() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("clip.mp4");
    std::fs::write(&path, b"x").unwrap();
    assert_eq!(
        effect_for(
            &path,
            &EventKind::Create(plain_libs::file_events::event::CreateKind::File),
            false
        ),
        Effect::Index
    );
}

#[test]
fn a_changed_media_file_is_reindexed() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("song.mp3");
    std::fs::write(&path, b"x").unwrap();
    assert_eq!(
        effect_for(
            &path,
            &EventKind::Modify(plain_libs::file_events::event::ModifyKind::Any),
            false
        ),
        Effect::Index
    );
}

#[test]
fn ordinary_files_are_catalogued_without_media_probes() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    for name in ["notes.txt.bak.tmp", "archive.zip"] {
        let path = dir.path().join(name);
        std::fs::write(&path, b"x").unwrap();
        assert_eq!(
            effect_for(
                &path,
                &EventKind::Create(plain_libs::file_events::event::CreateKind::File),
                false
            ),
            Effect::Index,
            "{name} must be searchable"
        );
    }
}

#[test]
fn dotfiles_are_indexed_and_a_nomedia_marker_rescans() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let hidden = dir.path().join(".hidden.mp4");
    std::fs::write(&hidden, b"x").unwrap();
    assert_eq!(
        effect_for(
            &hidden,
            &EventKind::Create(plain_libs::file_events::event::CreateKind::File),
            false
        ),
        Effect::Index
    );
    assert_eq!(
        effect_for(
            &dir.path().join(".nomedia"),
            &EventKind::Create(plain_libs::file_events::event::CreateKind::File),
            false
        ),
        Effect::RescanSubtree,
        "a marker changes what a directory contains"
    );
}

#[test]
fn removed_files_and_directories_are_forgotten() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let file = dir.path().join("gone.mp4");
    std::fs::write(&file, b"x").unwrap();
    assert_eq!(
        effect_for(&file, &EventKind::Remove(RemoveKind::File), false),
        Effect::Forget
    );
    assert_eq!(
        effect_for(
            &dir.path().join("album"),
            &EventKind::Remove(RemoveKind::Folder),
            false
        ),
        Effect::Forget,
        "a deleted directory cannot be scanned"
    );
}

#[test]
fn an_excluded_path_is_forgotten_rather_than_indexed() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("cached.mp4");
    std::fs::write(&path, b"x").unwrap();
    for kind in [
        EventKind::Create(plain_libs::file_events::event::CreateKind::File),
        EventKind::Modify(plain_libs::file_events::event::ModifyKind::Any),
    ] {
        assert_eq!(
            effect_for(&path, &kind, true),
            Effect::Forget,
            "a file the policy excludes must leave the catalog, not enter it"
        );
    }
}

#[test]
fn a_rename_forgets_the_old_path_and_indexes_the_current_file() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let old = dir.path().join("old.mp4");
    let new = dir.path().join("new.mp4");
    std::fs::write(&old, b"x").unwrap();
    std::fs::rename(&old, &new).unwrap();
    for mode in [
        RenameMode::From,
        RenameMode::To,
        RenameMode::Both,
        RenameMode::Any,
    ] {
        let kind = EventKind::Modify(ModifyKind::Name(mode));
        assert_eq!(effect_for(&old, &kind, false), Effect::Forget);
        assert_eq!(effect_for(&new, &kind, false), Effect::Index);
        assert_eq!(effect_for(&new, &kind, true), Effect::Forget);
    }
}

#[test]
fn a_directory_arriving_by_create_or_rename_is_scanned() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let old = root.path().join("old");
    let new = root.path().join("new");
    std::fs::create_dir(&old).unwrap();
    std::fs::rename(&old, &new).unwrap();
    let kind = EventKind::Modify(ModifyKind::Name(RenameMode::Both));
    assert_eq!(effect_for(&old, &kind, false), Effect::Forget);
    assert_eq!(effect_for(&new, &kind, false), Effect::RescanSubtree);
    assert_eq!(
        effect_for(
            &new,
            &EventKind::Create(plain_libs::file_events::event::CreateKind::Folder),
            false
        ),
        Effect::RescanSubtree
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_file_writes_only_reach_indexing_after_the_writer_closes() {
    use plain_libs::file_events::event::{AccessKind, AccessMode, CreateKind, DataChange};
    assert!(completed_event(Event::new(EventKind::Create(CreateKind::File))).is_none());
    assert!(
        completed_event(Event::new(EventKind::Modify(ModifyKind::Data(
            DataChange::Content
        ))))
        .is_none()
    );
    assert!(
        completed_event(Event::new(EventKind::Access(AccessKind::Open(
            AccessMode::Read
        ))))
        .is_none()
    );
    let event = Event::new(EventKind::Access(AccessKind::Close(AccessMode::Write)))
        .add_path("/media/closed.png".into());
    let event = completed_event(event).unwrap();
    assert_eq!(event.kind, EventKind::Modify(ModifyKind::Any));
    assert_eq!(
        event.paths,
        vec![std::path::PathBuf::from("/media/closed.png")]
    );
}

#[test]
fn watch_registration_uses_native_recursion_or_only_allowed_directory_watches() {
    #[derive(Default)]
    struct RecordingWatcher(Vec<(std::path::PathBuf, RecursiveMode)>);
    impl Watcher for RecordingWatcher {
        fn new<F: plain_libs::file_events::EventHandler>(
            _: F,
        ) -> plain_libs::file_events::Result<Self> {
            Ok(Self::default())
        }
        fn watch(
            &mut self,
            path: &Path,
            mode: RecursiveMode,
        ) -> plain_libs::file_events::Result<()> {
            self.0.push((path.to_owned(), mode));
            Ok(())
        }
        fn unwatch(&mut self, _: &Path) -> plain_libs::file_events::Result<()> {
            Ok(())
        }
    }
    let directory = watch_directory_fixture();
    let root = directory.path();
    std::fs::create_dir_all(root.join("project/src")).unwrap();
    std::fs::create_dir_all(root.join("project/cache/objects")).unwrap();
    std::fs::create_dir_all(root.join("repository/.git/objects")).unwrap();
    let scope = crate::features::files::index_scope::Scope {
        volumes: Vec::new(),
        directories: vec![crate::features::files::index_scope::normalized(root)],
        exclusions: Vec::new(),
        defaults: crate::features::files::index_scope::Defaults {
            excluded_directories: Vec::new(),
            excluded_names: vec!["cache".into()],
        },
    };
    let mut watcher = RecordingWatcher::default();
    let mut watched = std::collections::BTreeSet::new();
    let (_, stopped) = tokio::sync::watch::channel(false);
    watch_tree(&mut watcher, &mut watched, root, Some(&scope), &stopped).unwrap();
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        assert_eq!(watcher.0, vec![(root.to_owned(), RecursiveMode::Recursive)]);
        watch_tree(
            &mut watcher,
            &mut watched,
            &root.join("project"),
            Some(&scope),
            &stopped,
        )
        .unwrap();
        assert_eq!(watcher.0.len(), 1);
    } else {
        assert_eq!(watcher.0.len(), 3);
        assert!(
            watcher
                .0
                .iter()
                .all(|(path, mode)| !path.starts_with(root.join("project/cache"))
                    && !path.starts_with(root.join("repository"))
                    && *mode == RecursiveMode::NonRecursive)
        );
    }
}

#[test]
fn watcher_excluded_event_bursts_do_not_fill_the_watch_queue() {
    let directory = watch_directory_fixture();
    let root = directory.path();
    let scope = crate::features::files::index_scope::Scope {
        volumes: Vec::new(),
        directories: vec![crate::features::files::index_scope::normalized(root)],
        exclusions: vec![crate::features::files::index_scope::normalized(
            &root.join("build"),
        )],
        defaults: crate::features::files::index_scope::Defaults {
            excluded_directories: Vec::new(),
            excluded_names: vec!["lost+found".into()],
        },
    };
    let (send, mut receive) = tokio::sync::mpsc::channel(256);
    for path in [
        ".git/objects/item",
        ".internal/cache/item",
        ".plain-trash/nested/deleted.txt",
        "lost+found/item",
    ] {
        let event = Event::new(EventKind::Modify(ModifyKind::Any)).add_path(root.join(path));
        enqueue_event(&send, Some(&scope), event).unwrap();
    }
    assert!(receive.try_recv().is_err());
    let ordinary_trash =
        Event::new(EventKind::Modify(ModifyKind::Any)).add_path(root.join("trash/kept.txt"));
    enqueue_event(&send, Some(&scope), ordinary_trash.clone()).unwrap();
    assert_eq!(receive.try_recv().unwrap(), ordinary_trash);
    for index in 0..4096 {
        let event = Event::new(EventKind::Modify(ModifyKind::Any))
            .add_path(root.join(format!("build/{index}")));
        enqueue_event(&send, Some(&scope), event).unwrap();
    }
    assert!(receive.try_recv().is_err());
    let event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
        .add_path(root.join("build/old"))
        .add_path(root.join("new"));
    enqueue_event(&send, Some(&scope), event.clone()).unwrap();
    assert_eq!(receive.try_recv().unwrap(), event);
    enqueue_event(&send, Some(&scope), Event::new(EventKind::Other)).unwrap();
    assert_eq!(receive.try_recv().unwrap().kind, EventKind::Other);
}

#[test]
fn an_inaccessible_watch_root_is_skipped_without_marking_it_watched() {
    let directory = watch_directory_fixture();
    let mut watcher = DeniedWatcher {
        denied: directory.path().to_owned(),
        ..Default::default()
    };
    let mut watched = std::collections::BTreeSet::new();
    let (_, stopped) = tokio::sync::watch::channel(false);
    watch_tree(&mut watcher, &mut watched, directory.path(), None, &stopped).unwrap();
    assert!(watched.is_empty());
    assert_eq!(watcher.calls, vec![directory.path().to_owned()]);
}

#[test]
fn git_project_events_are_skipped_without_disabling_other_file_events() {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
    let directory = plain_libs::temporary::Builder::new()
        .prefix("watch-scope-")
        .tempdir_in(home)
        .unwrap();
    assert!(crate::features::media::catalog_paths::scan_excluded_root(directory.path()).is_none());
    let repo = directory.path().join("project");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let (send, mut receive) = tokio::sync::mpsc::channel(8);
    let excluded = Event::new(EventKind::Remove(
        plain_libs::file_events::event::RemoveKind::File,
    ))
    .add_path(repo.join("file.txt"));
    enqueue_event(&send, None, excluded).unwrap();
    assert!(receive.try_recv().is_err());
    let included = Event::new(EventKind::Remove(
        plain_libs::file_events::event::RemoveKind::File,
    ))
    .add_path(directory.path().join("report.txt"));
    enqueue_event(&send, None, included.clone()).unwrap();
    assert_eq!(receive.try_recv().unwrap().paths, included.paths);
}

struct GatedWatcher {
    entered: Option<tokio::sync::oneshot::Sender<()>>,
    release: std::sync::mpsc::Receiver<()>,
    dropped: Option<tokio::sync::oneshot::Sender<()>>,
    calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    callback: Box<dyn FnMut(plain_libs::file_events::Result<Event>) + Send>,
}

impl Watcher for GatedWatcher {
    fn new<F: plain_libs::file_events::EventHandler>(
        _: F,
    ) -> plain_libs::file_events::Result<Self> {
        unreachable!()
    }

    fn watch(&mut self, path: &Path, _: RecursiveMode) -> plain_libs::file_events::Result<()> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(entered) = self.entered.take() {
            entered.send(()).unwrap();
            self.release
                .recv_timeout(std::time::Duration::from_secs(5))
                .map_err(|_| {
                    plain_libs::file_events::Error::io(std::io::ErrorKind::TimedOut.into())
                })?;
            (self.callback)(Ok(
                Event::new(EventKind::Modify(ModifyKind::Any)).add_path(path.join("changed.txt"))
            ));
        }
        Ok(())
    }

    fn unwatch(&mut self, _: &Path) -> plain_libs::file_events::Result<()> {
        Ok(())
    }
}

impl Drop for GatedWatcher {
    fn drop(&mut self) {
        if let Some(dropped) = self.dropped.take() {
            let _ = dropped.send(());
        }
    }
}

#[tokio::test]
async fn starting_a_watcher_returns_before_registration_and_signals_readiness() {
    let directory = watch_directory_fixture();
    std::fs::write(directory.path().join("changed.txt"), b"fixture").unwrap();
    let (entered, ready) = tokio::sync::oneshot::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let (dropped, gone) = tokio::sync::oneshot::channel();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (send, mut events) = tokio::sync::mpsc::unbounded_channel();
    let mut handle = start_with_watcher(
        &[directory.path().to_owned()],
        None,
        |callback| {
            Ok(GatedWatcher {
                entered: Some(entered),
                release: gate,
                dropped: Some(dropped),
                calls,
                callback,
            })
        },
        move |event| {
            send.send(event).unwrap();
            async { Ok(()) }
        },
    )
    .unwrap();
    ready.await.unwrap();
    assert!(!*handle.ready.borrow());
    assert!(
        events.try_recv().is_err(),
        "registration must still be gated after start returns"
    );
    release.send(()).unwrap();
    handle.ready.wait_for(|ready| *ready).await.unwrap();
    let change = tokio::time::timeout(std::time::Duration::from_secs(5), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(change.kind, EventKind::Modify(ModifyKind::Any));
    assert_eq!(change.paths, vec![directory.path().join("changed.txt")]);
    drop(handle);
    tokio::time::timeout(std::time::Duration::from_secs(5), gone)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn replacing_a_watcher_cancels_registration_before_the_next_root() {
    let directory = watch_directory_fixture();
    let roots: Vec<_> = ["first", "second"]
        .map(|name| directory.path().join(name))
        .into();
    for root in &roots {
        std::fs::create_dir(root).unwrap();
    }
    let (entered, ready) = tokio::sync::oneshot::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let (dropped, gone) = tokio::sync::oneshot::channel();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let handled = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed_handlers = handled.clone();
    let handle = start_with_watcher(
        &roots,
        None,
        |callback| {
            Ok(GatedWatcher {
                entered: Some(entered),
                release: gate,
                dropped: Some(dropped),
                calls,
                callback,
            })
        },
        move |_| {
            handled.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async { Ok(()) }
        },
    )
    .unwrap();
    ready.await.unwrap();
    drop(handle);
    release.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), gone)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observed.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        observed_handlers.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn dropping_a_watcher_cancels_a_pending_catalog_handler() {
    let directory = watch_directory_fixture();
    let (entered, ready) = tokio::sync::oneshot::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let (dropped, gone) = tokio::sync::oneshot::channel();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (send, mut processing) = tokio::sync::mpsc::unbounded_channel();
    let handle = start_with_watcher(
        &[directory.path().to_owned()],
        None,
        |callback| {
            Ok(GatedWatcher {
                entered: Some(entered),
                release: gate,
                dropped: Some(dropped),
                calls,
                callback,
            })
        },
        move |_| {
            send.send(()).unwrap();
            std::future::pending()
        },
    )
    .unwrap();
    ready.await.unwrap();
    release.send(()).unwrap();
    processing.recv().await.unwrap();
    drop(handle);
    tokio::time::timeout(std::time::Duration::from_secs(5), gone)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn directory_scanning_waits_for_watch_registration_and_remains_cancellable() {
    use crate::features::services::files::{catalog_scan::NativeMediaCatalog, media_scans::Status};
    use std::future::Future;
    for cancel in [false, true] {
        let directory = watch_directory_fixture();
        let source = directory.path().join("source");
        std::fs::create_dir(&source).unwrap();
        let document = source.join("document.txt");
        std::fs::write(&document, b"fixture").unwrap();
        let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
        let state = core.runtime_state();
        let catalog = NativeMediaCatalog::new(state.clone());
        let (started, ready) = tokio::sync::watch::channel(false);
        let (stop, _stopped) = tokio::sync::watch::channel(false);
        *state.media_watch.lock().unwrap() = Some(WatcherHandle {
            task: tokio::spawn(std::future::pending()),
            stop,
            ready,
        });
        let mut scan = Box::pin(catalog.scan_directory(source.to_str().unwrap()));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(scan.as_mut().poll(&mut context).is_pending());
        assert_eq!(
            started.receiver_count(),
            2,
            "the scan must wait on watcher readiness"
        );
        assert!(
            catalog
                .by_path(
                    crate::features::enums::DataType::Doc,
                    document.to_str().unwrap()
                )
                .unwrap()
                .is_none()
        );
        if cancel {
            state.media_scans.cancel().unwrap();
        } else {
            started.send_replace(true);
        }
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), scan)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.status,
            if cancel {
                Status::Cancelled
            } else {
                Status::Completed
            }
        );
        assert_eq!(
            catalog
                .by_path(
                    crate::features::enums::DataType::Doc,
                    document.to_str().unwrap()
                )
                .unwrap()
                .is_some(),
            !cancel
        );
        core.stop_core().await;
    }
}
