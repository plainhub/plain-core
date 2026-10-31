use super::*;
use chrono::TimeZone;

fn store(directory: &Path) -> Trash {
    Trash::new(Arc::new(Db::open(&directory.join("plain.db")).unwrap()))
}

#[test]
fn bucket_path_layout() {
    let relative = compute_bucket_rel_path(
        "file",
        "12",
        "hello world.mp4",
        Utc.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap(),
    );
    assert!(relative.starts_with("data/2026/03/f_12_"));
}

#[tokio::test]
async fn unicode_trash_search_preserves_case_folding_before_paging() {
    let directory = tempfile::tempdir().unwrap();
    let trash = store(directory.path());
    let paths = [
        directory.path().join("Ωmega.txt"),
        directory.path().join("other.txt"),
    ];
    for path in &paths {
        std::fs::write(path, b"data").unwrap();
    }
    trash
        .trash_paths(
            paths
                .iter()
                .map(|path| path.to_str().unwrap().into())
                .collect(),
        )
        .await
        .unwrap();
    let found = trash.list_trash(0, 1, "ωMEGA", SortOrder::NameAsc).unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].original_path.ends_with("Ωmega.txt"));
    assert!(
        trash
            .list_trash(1, 1, "ωMEGA", SortOrder::NameAsc)
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn trash_and_restore_preserve_file_identity_favorite_and_text_store() {
    let directory = tempfile::tempdir().unwrap();
    let trash = store(directory.path());
    let path = directory.path().join("report.txt");
    std::fs::write(&path, b"restore").unwrap();
    let first = trash
        .db
        .record_file_paths(&[path.to_str().unwrap().into()], None)
        .unwrap()
        .remove(0);
    trash
        .db
        .with_conn(|conn| conn.execute("UPDATE files SET favorited=1 WHERE id=?1", [first.id]))
        .unwrap();
    let moved = trash
        .trash_paths(vec![path.to_str().unwrap().into()])
        .await
        .unwrap()
        .remove(0);
    assert!(!path.exists());
    assert_eq!(trash.trash_count().unwrap(), 1);
    let row = trash.load_item(&first.id.to_string()).unwrap().unwrap();
    assert_eq!(
        Path::new(&row.original_path).parent().unwrap(),
        directory.path().canonicalize().unwrap()
    );
    assert_eq!(
        Path::new(&row.original_path).file_name().unwrap(),
        "report.txt"
    );
    assert_eq!(Trash::physical_path(&row), Path::new(&moved));
    assert!(trash.db.file(first.id).unwrap().unwrap().favorited);
    std::fs::write(&path, b"existing").unwrap();
    let restored = trash
        .restore_paths(vec![first.id.to_string()])
        .await
        .unwrap()
        .remove(0);
    assert!(restored.ends_with("report (1).txt"));
    assert_eq!(std::fs::read(&restored).unwrap(), b"restore");
    assert_eq!(std::fs::read(&path).unwrap(), b"existing");
    assert_eq!(trash.trash_count().unwrap(), 0);
    let row = trash.db.file(first.id).unwrap().unwrap();
    assert!(row.favorited);
    assert!(row.trashed_at.is_none() && row.original_path.is_none());
    assert_eq!(trash.db.key_values().iter().count(), 0);
}

#[tokio::test]
async fn permanent_deletion_waits_for_filesystem_success() {
    let directory = tempfile::tempdir().unwrap();
    let trash = store(directory.path());
    let path = directory.path().join("report.txt");
    std::fs::write(&path, b"deleted").unwrap();
    let moved = trash
        .trash_paths(vec![path.to_str().unwrap().into()])
        .await
        .unwrap()
        .remove(0);
    let row = trash
        .list_trash(0, 10, "report", SortOrder::NameAsc)
        .unwrap()
        .remove(0);
    let id = row.id.parse().unwrap();
    let parent = Path::new(&moved).parent().unwrap();
    let saved = parent.with_extension("saved");
    std::fs::rename(parent, &saved).unwrap();
    std::fs::write(parent, b"blocked").unwrap();
    assert!(trash.delete_trash_by_path(&row.id).await.is_err());
    assert!(trash.db.file(id).unwrap().unwrap().trashed_at.is_some());
    std::fs::remove_file(parent).unwrap();
    std::fs::rename(&saved, parent).unwrap();
    trash.delete_trash_by_path(&row.id).await.unwrap();
    assert!(!Path::new(&moved).exists());
    assert!(trash.db.file(id).unwrap().is_none());
}

#[tokio::test]
async fn trash_lists_only_the_directory_root_and_restores_descendant_ids() {
    let directory = tempfile::tempdir().unwrap();
    let trash = store(directory.path());
    let root = directory.path().join("folder");
    std::fs::create_dir(&root).unwrap();
    let child = root.join("child.txt");
    std::fs::write(&child, b"child").unwrap();
    let original = trash
        .db
        .record_file_paths(
            &[
                root.to_str().unwrap().into(),
                child.to_str().unwrap().into(),
            ],
            None,
        )
        .unwrap();
    let moved = trash
        .trash_paths(vec![root.to_str().unwrap().into()])
        .await
        .unwrap()
        .remove(0);
    assert_eq!(trash.trash_count().unwrap(), 1);
    let trashed = trash.db.file(original[1].id).unwrap().unwrap();
    assert!(trashed.trashed_at.is_some());
    assert!(trashed.path.ends_with("child.txt"));
    trash.restore_paths(vec![moved]).await.unwrap();
    assert_eq!(std::fs::read(child).unwrap(), b"child");
    assert_eq!(
        trash.db.file(original[1].id).unwrap().unwrap().path,
        original[1].path
    );
}

#[test]
fn inaccessible_volume_trash_uses_the_selected_files_parent_without_changing_filesystems() {
    for kind in [
        std::io::ErrorKind::PermissionDenied,
        std::io::ErrorKind::ReadOnlyFilesystem,
        std::io::ErrorKind::NotFound,
    ] {
        let mut attempted = Vec::new();
        let selected = writable_disk(Path::new("/volume/owned/report.txt"), "/volume", |path| {
            attempted.push(path.to_owned());
            if path == "/volume" {
                Err(std::io::Error::from(kind))
            } else {
                Ok(())
            }
        })
        .unwrap();
        assert_eq!(attempted, ["/volume", "/volume/owned"]);
        assert_eq!(selected, "/volume/owned");
        assert_eq!(
            abs_trash_path(&selected, "data/file"),
            Path::new("/volume/owned/.plain-trash/data/file")
        );
    }
}

#[test]
fn volume_trash_failures_other_than_access_do_not_silently_select_another_root() {
    let mut calls = 0;
    let result = writable_disk(Path::new("/volume/owned/report.txt"), "/volume", |_| {
        calls += 1;
        Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
    });
    assert!(result.is_err());
    assert_eq!(calls, 1);
}

#[cfg(unix)]
#[test]
fn a_symlink_cannot_replace_an_owned_trash_root() {
    let directory = tempfile::tempdir().unwrap();
    let other = directory.path().join("other");
    std::fs::create_dir(&other).unwrap();
    std::os::unix::fs::symlink(&other, directory.path().join(".plain-trash")).unwrap();
    assert_eq!(
        prepare_trash_parent(directory.path().to_str().unwrap(), "data/file")
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert!(!other.join("data").exists());
}

#[cfg(unix)]
#[test]
fn nested_trash_symlinks_are_rejected_before_creating_any_outside_directories() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join(".plain-trash");
    let outside = directory.path().join("outside");
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("data")).unwrap();
    let error =
        prepare_trash_parent(directory.path().to_str().unwrap(), "data/2026/01/file").unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(!outside.join("2026").exists());
}
