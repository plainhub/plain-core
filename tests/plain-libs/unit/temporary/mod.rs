use super::*;

#[test]
fn collision_keeps_existing_destination_and_allows_retry() {
    let dir = tempdir().unwrap();
    let destination = dir.path().join("occupied");
    fs::write(&destination, b"original").unwrap();
    let mut file = NamedTempFile::new_in(dir.path()).unwrap();
    file.write_all(b"replacement").unwrap();
    let temporary = file.path().to_owned();

    let error = file.persist_noclobber(&destination).unwrap_err();
    assert_eq!(error.error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&destination).unwrap(), b"original");
    assert_eq!(fs::read(&temporary).unwrap(), b"replacement");

    error.file.persist(&destination).unwrap();
    assert_eq!(fs::read(&destination).unwrap(), b"replacement");
    assert!(!temporary.exists());
}

#[cfg(unix)]
#[test]
fn reopen_rejects_a_substituted_path_and_temp_entries_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let file = NamedTempFile::new_in(dir.path()).unwrap();
    for path in [dir.path(), file.path()] {
        assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o077, 0);
    }
    assert!(file.reopen().is_ok());
    fs::rename(file.path(), dir.path().join("original")).unwrap();
    fs::write(file.path(), b"substitute").unwrap();
    assert_eq!(file.reopen().unwrap_err().kind(), io::ErrorKind::NotFound);
}

#[test]
fn path_owner_closes_file_and_removes_it_on_drop() {
    let dir = tempdir().unwrap();
    let mut file = NamedTempFile::new_in(dir.path()).unwrap();
    file.write_all(b"prepared").unwrap();
    let path = file.path().to_owned();
    let owner = file.into_temp_path();
    assert_eq!(owner.as_ref(), path);
    assert_eq!(fs::read(&path).unwrap(), b"prepared");
    assert_eq!(
        std::mem::size_of_val(&owner),
        std::mem::size_of::<PathBuf>()
    );
    drop(owner);
    assert!(!path.exists());
}
