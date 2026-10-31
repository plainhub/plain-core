use super::*;

fn store_with_import(dir: &Path, contents: &[u8]) -> (Arc<Db>, Arc<FileStore>, String, PathBuf) {
    let db = Arc::new(Db::open(Path::new(":memory:")).unwrap());
    let store = FileStore::new(db.clone(), dir.to_path_buf());
    let source = dir.join("source.bin");
    std::fs::write(&source, contents).unwrap();
    let file = crate::chat::attachments::app_file_store::import_file(
        &db,
        dir,
        &source,
        "source.bin",
        "application/octet-stream",
    )
    .unwrap();
    let relative = PathBuf::from(file.real_path.clone());
    (db, store, file.id, relative)
}

#[test]
fn resolve_returns_the_stored_file_for_its_own_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let (_, store, _, relative) = store_with_import(dir.path(), b"payload");

    let suffix = relative.file_name().unwrap().to_str().unwrap();

    assert_eq!(store.resolve(suffix).unwrap(), dir.path().join(&relative));
}

#[test]
fn resolve_rejects_anything_that_is_not_a_stored_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let (_, store, id, _) = store_with_import(dir.path(), b"payload");

    for rejected in ["../../secret", &id, "", "not-a-hash.bin"] {
        assert!(
            store.resolve(rejected).is_err(),
            "{rejected} must not resolve to a file"
        );
    }
}

#[test]
fn resolve_rejects_a_suffix_that_does_not_match_the_stored_name() {
    let dir = tempfile::tempdir().unwrap();
    let (_, store, id, _) = store_with_import(dir.path(), b"payload");

    // A well-formed hash naming a row's real extension but not its file name
    // must not smuggle in a different path.
    let forged = format!("{id}.txt");

    assert!(store.resolve(&forged).is_err());
}

#[test]
fn stored_paths_are_owned_by_the_file_store_without_requiring_the_file_to_exist() {
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(Path::new(":memory:")).unwrap());
    let store = FileStore::new(db, directory.path().to_path_buf());
    assert_eq!(
        store.stored_path("files/aa/bb/file.jpg").unwrap(),
        directory.path().join("files/aa/bb/file.jpg")
    );
    assert_eq!(
        store
            .stored_path(directory.path().join("files/photo.jpg").to_str().unwrap())
            .unwrap(),
        directory.path().join("files/photo.jpg")
    );
    for stored in [
        "../outside",
        "files/../../outside",
        "other/file.jpg",
        "files\\outside",
    ] {
        assert!(store.stored_path(stored).is_err(), "{stored}");
    }
}
