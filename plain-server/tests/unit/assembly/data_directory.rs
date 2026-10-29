use super::*;

#[test]
fn a_data_directory_is_initialized_once_and_remains_locked_for_the_process() {
    let directory = tempfile::tempdir().unwrap();
    let first = DataDirectory::open(directory.path(), &directory.path().join("cache")).unwrap();
    let second =
        DataDirectory::open(&directory.path().join("."), &directory.path().join("cache")).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    for path in [
        first.cache_paths().root().to_owned(),
        paths.note_images(),
        paths.audio(),
    ] {
        assert!(path.is_dir(), "{}", path.display());
    }
    assert!(!paths.database().exists());
    assert!(!paths.database().exists());
    let other = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(paths.lock())
        .unwrap();
    assert!(other.try_lock().is_err());
    drop(first);
    drop(second);
    assert!(other.try_lock().is_err());
    std::fs::write(paths.pending_restore(), b"pending").unwrap();
    assert!(
        DataDirectory::open(directory.path(), &directory.path().join("cache"))
            .err()
            .unwrap()
            .contains("process restart")
    );
}

#[test]
fn invalid_fixed_directory_prevents_store_initialization() {
    let directory = tempfile::tempdir().unwrap();
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    std::fs::write(paths.audio(), b"file in place of directory").unwrap();
    assert!(DataDirectory::open(directory.path(), &directory.path().join("cache")).is_err());
    assert!(!paths.database().exists());
    assert!(!paths.prefs().exists());
    assert!(!paths.database().exists());
    assert_eq!(
        std::fs::read(paths.audio()).unwrap(),
        b"file in place of directory"
    );
}
#[test]
fn invalid_pending_restore_prevents_store_initialization() {
    let directory = tempfile::tempdir().unwrap();
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    std::fs::write(paths.pending_restore(), b"invalid marker").unwrap();
    assert!(DataDirectory::open(directory.path(), &directory.path().join("cache")).is_err());
    assert!(!paths.database().exists());
    assert!(!paths.prefs().exists());
    assert!(!paths.database().exists());
}
#[test]
fn existing_lock_ownership_prevents_restore_application() {
    let directory = tempfile::tempdir().unwrap();
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(paths.lock())
        .unwrap();
    lock.try_lock().unwrap();
    std::fs::write(paths.pending_restore(), b"invalid marker").unwrap();
    assert!(
        DataDirectory::open(directory.path(), &directory.path().join("cache"))
            .err()
            .unwrap()
            .starts_with("Data directory is already in use:")
    );
    assert!(paths.pending_restore().exists());
    assert!(!paths.database().exists());
}
