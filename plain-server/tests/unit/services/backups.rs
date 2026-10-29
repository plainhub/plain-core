use super::*;
use std::io::Read;

#[tokio::test]
async fn export_uses_owned_snapshots_and_preserves_attachment_bytes_without_host_calls() {
    let directory = tempfile::tempdir().unwrap();
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    let prefs = Arc::new(Prefs::load(&paths.prefs()).unwrap());
    prefs.set("device_name", "Backup fixture").unwrap();
    let db = Arc::new(Db::open(&paths.database()).unwrap());
    db.with_conn(|conn| conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE backup_values (id INTEGER PRIMARY KEY,value TEXT); INSERT INTO backup_values VALUES (1,'committed');").unwrap());
    let store = Arc::new(LazyDb::from_open(paths.kv(), db.key_values().unwrap()));
    store
        .get()
        .unwrap()
        .insert(b"binary", [0u8, 255, 3])
        .unwrap();
    let attachment = directory.path().join("files/aa/bb/blob.bin");
    std::fs::create_dir_all(attachment.parent().unwrap()).unwrap();
    std::fs::write(&attachment, [1u8, 2, 255]).unwrap();
    std::fs::write(paths.lock(), b"lock").unwrap();
    std::fs::create_dir(directory.path().join(".restore-stage-orphan")).unwrap();
    std::fs::write(
        directory.path().join(".restore-stage-orphan/unpublished"),
        b"private staging",
    )
    .unwrap();
    std::fs::create_dir_all(directory.path().join("cache")).unwrap();
    std::fs::write(
        directory.path().join("cache").join("not-backed-up"),
        b"cache",
    )
    .unwrap();
    let (_stop, stopped) = watch::channel(false);
    let archive = export(
        db,
        prefs,
        store,
        directory.path().to_path_buf(),
        directory.path().join("cache"),
        stopped,
    )
    .await
    .unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive).unwrap()).unwrap();
    let names: Vec<_> = zip.file_names().map(|name| name.unwrap().into_owned()).collect();
    assert!(names.contains(&"files/plain.db".into()));
    assert!(names.iter().any(|name| name.starts_with("files/fjall/")));
    assert!(names.iter().all(|name| !name.contains("cache/")
        && !name.ends_with("-wal")
        && !name.ends_with("-shm")));
    let restored = tempfile::tempdir().unwrap();
    zip.extract(restored.path()).unwrap();
    let snapshot = rusqlite::Connection::open(restored.path().join("files/plain.db")).unwrap();
    assert_eq!(
        snapshot
            .query_row("SELECT value FROM backup_values", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "committed"
    );
    let kv = plain_rs::kv::Db::open(&restored.path().join("files/fjall")).unwrap();
    assert_eq!(kv.get(b"binary").unwrap().unwrap().as_ref(), [0u8, 255, 3]);
    for (name, expected) in [("prefs.json", "Backup fixture")] {
        let mut json = String::new();
        zip.by_name(&format!("files/{name}"))
            .unwrap()
            .read_to_string(&mut json)
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap()["device_name"],
            expected
        );
    }
    assert_eq!(
        std::fs::read(restored.path().join("files/files/aa/bb/blob.bin")).unwrap(),
        [1u8, 2, 255]
    );
}
#[test]
fn cancellation_removes_staging_and_keeps_source_data() {
    let directory = tempfile::tempdir().unwrap();
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    let prefs = Prefs::load(&paths.prefs()).unwrap();
    let db = Db::open(&paths.database()).unwrap();
    let store = LazyDb::new(paths.kv());
    let roots = roots(directory.path(), &directory.path().join("cache"));
    let calls = std::cell::Cell::new(0usize);
    let result = snapshot(&db, &prefs, &store, directory.path(), &roots, || {
        let call = calls.get();
        calls.set(call + 1);
        call >= 2
    });
    assert!(result.is_err());
    assert!(paths.database().exists());
    assert_eq!(std::fs::read_dir(&roots.cache).unwrap().count(), 0);
}
#[cfg(unix)]
#[test]
fn enumeration_excludes_symlinks_and_rejects_sources_outside_the_root() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("private"), b"outside").unwrap();
    std::os::unix::fs::symlink(outside.path(), directory.path().join("link")).unwrap();
    std::fs::write(directory.path().join("inside"), b"inside").unwrap();
    let roots = roots(directory.path(), &directory.path().join("cache"));
    let mut files = Vec::new();
    collect(
        directory.path(),
        &roots.folders,
        &HashSet::new(),
        &mut files,
        &|| false,
    )
    .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].1, "files/inside");
    assert!(name(outside.path(), &roots.folders).is_err());
}
