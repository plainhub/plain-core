use super::*;

#[test]
fn destructive_path_validation_rejects_dot_targets_before_any_filesystem_operation() {
    let directory = tempfile::tempdir().unwrap();
    let preserved = directory.path().join("preserved.txt");
    fs::write(&preserved, b"owned fixture").unwrap();
    assert!(validate(&preserved).is_ok());
    for suffix in [".", ".."] {
        for trailing in ["", "/", "//"] {
            let path = format!(
                "{}{separator}{suffix}{trailing}",
                directory.path().display(),
                separator = std::path::MAIN_SEPARATOR
            );
            assert!(validate(Path::new(&path)).is_err(), "{path}");
        }
    }
    assert_eq!(fs::read(preserved).unwrap(), b"owned fixture");
}

#[cfg(windows)]
#[test]
fn windows_backslash_and_mixed_separator_dot_targets_are_rejected() {
    for path in [
        r"C:\",
        r"C:\owned\.",
        r"C:\owned\..",
        r"C:\owned\..\",
        r"C:\owned/..\",
        r"\\server\share\",
        r"\\server\share\owned\..\",
    ] {
        assert!(validate(Path::new(path)).is_err(), "{path}");
    }
    assert!(validate(Path::new(r"C:\owned\file.txt")).is_ok());
}
#[test]
fn receipts_include_only_physically_removed_nodes_without_following_symlinks() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("nested/file"), b"synthetic").unwrap();
    fs::write(outside.path().join("keep"), b"outside").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), root.join("link")).unwrap();
    let plan = Plan::inspect(&root).unwrap();
    assert_eq!(plan.files.len(), 1);
    let outcome = plan.execute();
    assert!(outcome.removed);
    assert!(outcome.failures.is_empty());
    assert!(!root.exists());
    assert!(outside.path().join("keep").is_file());
    assert!(!Plan::inspect(&root).unwrap().execute().removed);
    assert!(Plan::inspect(Path::new("/")).is_err());
    assert!(Plan::inspect(&temp.path().join(".")).is_err());
}
#[test]
fn changed_nodes_remain_failed_and_do_not_erase_receipts_for_other_successes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir(&root).unwrap();
    let blocked = root.join("blocked");
    fs::write(&blocked, b"old").unwrap();
    let removed = root.join("removed");
    fs::write(&removed, b"synthetic").unwrap();
    let plan = Plan::inspect(&root).unwrap();
    fs::remove_file(&blocked).unwrap();
    fs::create_dir(&blocked).unwrap();
    fs::write(blocked.join("keep"), b"new").unwrap();
    let outcome = plan.execute();
    assert!(!outcome.removed);
    assert!(!outcome.failures.is_empty());
    assert!(
        outcome
            .paths
            .contains(&removed.to_string_lossy().into_owned())
    );
    assert!(
        !outcome
            .paths
            .contains(&blocked.to_string_lossy().into_owned())
    );
    assert!(blocked.join("keep").is_file());
    assert!(!removed.exists());
}

#[test]
fn plan_does_not_remove_replaced_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("original");
    std::fs::write(&path, "old").unwrap();
    let plan = super::Plan::inspect(&path).unwrap();
    std::fs::rename(&path, dir.path().join("old")).unwrap();
    std::fs::write(&path, "replacement").unwrap();
    let result = plan.execute();
    assert!(!result.removed);
    assert!(result.paths.is_empty());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "replacement");
    assert_eq!(result.failures.len(), 1);
}
