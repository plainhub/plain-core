//! The event→catalog mapping a media watcher depends on. Exercised without a
//! real watcher: what matters is which filesystem change turns into which
//! catalog operation.

use super::*;
use notify::event::{RemoveKind, RenameMode};

fn included(_: &Path) -> bool {
    false
}

fn excluded(_: &Path) -> bool {
    true
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
