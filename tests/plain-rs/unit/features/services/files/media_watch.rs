//! The event→catalog mapping a media watcher depends on. Exercised without a
//! real watcher: what matters is which filesystem change turns into which
//! catalog operation.

use super::*;
use notify::event::{RemoveKind, RenameMode};

#[test]
#[cfg(target_os = "linux")]
fn read_events_and_incomplete_writes_do_not_enter_the_reconciliation_queue() {
    use notify::event::{AccessKind, AccessMode, CreateKind, DataChange};
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
    fn new<F: notify::EventHandler>(_: F, _: notify::Config) -> notify::Result<Self> {
        Ok(Self::default())
    }
    fn watch(&mut self, path: &Path, _: RecursiveMode) -> notify::Result<()> {
        self.calls.push(path.to_owned());
        if path == self.denied {
            return Err(notify::Error::io(
                std::io::ErrorKind::PermissionDenied.into(),
            ));
        }
        Ok(())
    }
    fn unwatch(&mut self, _: &Path) -> notify::Result<()> {
        Ok(())
    }
    fn kind() -> notify::WatcherKind {
        notify::WatcherKind::NullWatcher
    }
}

#[test]
#[cfg(target_os = "linux")]
fn an_inaccessible_watch_descendant_is_skipped_before_descending() {
    let directory = tempfile::tempdir().unwrap();
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
    watch_tree(&mut watcher, &mut watched, directory.path(), None).unwrap();
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
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clip.mp4");
    std::fs::write(&path, b"x").unwrap();
    assert_eq!(
        effect_for(
            &path,
            &EventKind::Create(notify::event::CreateKind::File),
            false
        ),
        Effect::Index
    );
}

#[test]
fn a_changed_media_file_is_reindexed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.mp3");
    std::fs::write(&path, b"x").unwrap();
    assert_eq!(
        effect_for(
            &path,
            &EventKind::Modify(notify::event::ModifyKind::Any),
            false
        ),
        Effect::Index
    );
}

#[test]
fn ordinary_files_are_catalogued_without_media_probes() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["notes.txt.bak.tmp", "archive.zip"] {
        let path = dir.path().join(name);
        std::fs::write(&path, b"x").unwrap();
        assert_eq!(
            effect_for(
                &path,
                &EventKind::Create(notify::event::CreateKind::File),
                false
            ),
            Effect::Index,
            "{name} must be searchable"
        );
    }
}

#[test]
fn dotfiles_are_indexed_and_a_nomedia_marker_rescans() {
    let dir = tempfile::tempdir().unwrap();
    let hidden = dir.path().join(".hidden.mp4");
    std::fs::write(&hidden, b"x").unwrap();
    assert_eq!(
        effect_for(
            &hidden,
            &EventKind::Create(notify::event::CreateKind::File),
            false
        ),
        Effect::Index
    );
    assert_eq!(
        effect_for(
            &dir.path().join(".nomedia"),
            &EventKind::Create(notify::event::CreateKind::File),
            false
        ),
        Effect::RescanSubtree,
        "a marker changes what a directory contains"
    );
}

#[test]
fn removed_files_and_directories_are_forgotten() {
    let dir = tempfile::tempdir().unwrap();
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
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cached.mp4");
    std::fs::write(&path, b"x").unwrap();
    for kind in [
        EventKind::Create(notify::event::CreateKind::File),
        EventKind::Modify(notify::event::ModifyKind::Any),
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
    let dir = tempfile::tempdir().unwrap();
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
    let root = tempfile::tempdir().unwrap();
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
            &EventKind::Create(notify::event::CreateKind::Folder),
            false
        ),
        Effect::RescanSubtree
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_file_writes_only_reach_indexing_after_the_writer_closes() {
    use notify::event::{AccessKind, AccessMode, CreateKind, DataChange};
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
        fn new<F: notify::EventHandler>(_: F, _: notify::Config) -> notify::Result<Self> {
            Ok(Self::default())
        }
        fn watch(&mut self, path: &Path, mode: RecursiveMode) -> notify::Result<()> {
            self.0.push((path.to_owned(), mode));
            Ok(())
        }
        fn unwatch(&mut self, _: &Path) -> notify::Result<()> {
            Ok(())
        }
        fn kind() -> notify::WatcherKind {
            notify::WatcherKind::NullWatcher
        }
    }
    let directory = tempfile::Builder::new()
        .prefix("media-watch-")
        .tempdir()
        .unwrap();
    let root = directory.path();
    std::fs::create_dir_all(root.join("project/src")).unwrap();
    std::fs::create_dir_all(root.join("project/.git/objects")).unwrap();
    let scope = crate::features::prefs::file_index::Scope {
        directories: vec![crate::features::prefs::file_index::normalized(root)],
        exclusions: Vec::new(),
        defaults: crate::features::prefs::file_index::Defaults {
            scan_directories: Vec::new(),
            excluded_directories: Vec::new(),
            excluded_names: vec![".git".into()],
        },
    };
    let mut watcher = RecordingWatcher::default();
    let mut watched = std::collections::BTreeSet::new();
    watch_tree(&mut watcher, &mut watched, root, Some(&scope)).unwrap();
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        assert_eq!(watcher.0, vec![(root.to_owned(), RecursiveMode::Recursive)]);
        watch_tree(
            &mut watcher,
            &mut watched,
            &root.join("project"),
            Some(&scope),
        )
        .unwrap();
        assert_eq!(watcher.0.len(), 1);
    } else {
        assert_eq!(watcher.0.len(), 3);
        assert!(
            watcher
                .0
                .iter()
                .all(|(path, mode)| !path.to_string_lossy().contains(".git")
                    && *mode == RecursiveMode::NonRecursive)
        );
    }
}

#[test]
fn watcher_excluded_event_bursts_do_not_fill_the_watch_queue() {
    let directory = tempfile::Builder::new()
        .prefix("media-watch-")
        .tempdir()
        .unwrap();
    let root = directory.path();
    let scope = crate::features::prefs::file_index::Scope {
        directories: vec![crate::features::prefs::file_index::normalized(root)],
        exclusions: vec![crate::features::prefs::file_index::normalized(
            &root.join("build"),
        )],
        defaults: crate::features::prefs::file_index::Defaults {
            scan_directories: Vec::new(),
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
    let directory = tempfile::tempdir().unwrap();
    let mut watcher = DeniedWatcher {
        denied: directory.path().to_owned(),
        ..Default::default()
    };
    let mut watched = std::collections::BTreeSet::new();
    watch_tree(&mut watcher, &mut watched, directory.path(), None).unwrap();
    assert!(watched.is_empty());
    assert_eq!(watcher.calls, vec![directory.path().to_owned()]);
}
