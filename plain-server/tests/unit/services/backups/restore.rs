use super::*;
use std::io::Cursor;

fn archive() -> Vec<u8> {
    let source = tempfile::tempdir().unwrap();
    let paths = plain_rs::data_paths::DataPaths::new(source.path());
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&paths.prefs()).unwrap());
    plain_rs::prefs::identity::ensure_identity(&prefs);
    prefs.set("device_name", "restored device").unwrap();
    let db = plain_rs::db::Db::open(&paths.database()).unwrap();
    db.with_conn(|conn| conn.execute_batch("CREATE TABLE restore_fixture (value TEXT); INSERT INTO restore_fixture VALUES ('snapshot');").unwrap());
    let store = plain_rs::kv::LazyDb::new(paths.database());
    store.get().unwrap().insert("restored", "snapshot").unwrap();
    std::fs::create_dir(source.path().join("files")).unwrap();
    std::fs::write(source.path().join("files/blob"), b"attachment").unwrap();
    let (_staging, files) = super::super::snapshot(
        &db,
        &prefs,
        source.path(),
        &super::super::roots(source.path(), &source.path().join("cache")),
        || false,
    )
    .unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, name) in files {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&std::fs::read(path).unwrap()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
fn prepare(bytes: Vec<u8>, directory: &Path) -> Result<(), String> {
    let (_stop, stopped) = watch::channel(false);
    stage_reader(
        Cursor::new(bytes),
        directory,
        &stopped,
        &AtomicBool::new(false),
    )
}
fn pending(directory: &Path) -> Pending {
    serde_json::from_reader(
        File::open(plain_rs::data_paths::DataPaths::new(directory).pending_restore()).unwrap(),
    )
    .unwrap()
}
fn assert_restored(directory: &Path) {
    let paths = plain_rs::data_paths::DataPaths::new(directory);
    assert!(!directory.join("plain.db-wal").exists());
    assert!(!directory.join("plain.db-shm").exists());
    let db = rusqlite::Connection::open_with_flags(
        paths.database(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    assert_eq!(
        db.query_row("SELECT value FROM restore_fixture", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "snapshot"
    );
    let prefs = plain_rs::prefs::Prefs::load(&paths.prefs()).unwrap();
    assert_eq!(
        prefs.get::<String>("device_name").unwrap().as_deref(),
        Some("restored device")
    );
    let kv = plain_rs::kv::Db::open(&paths.database()).unwrap();
    assert_eq!(kv.get("restored").unwrap().unwrap(), "snapshot");
    assert!(kv.get("old-only").unwrap().is_none());
    assert_eq!(
        std::fs::read(directory.join("files/blob")).unwrap(),
        b"attachment"
    );
    assert!(!paths.pending_restore().exists());
}
#[test]
fn staging_keeps_live_stores_untouched_and_startup_replaces_closed_stores() {
    let directory = tempfile::tempdir().unwrap();
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&paths.prefs()).unwrap());
    prefs.set("device_name", "live device").unwrap();
    let db = plain_rs::db::Db::open(&paths.database()).unwrap();
    db.with_conn(|conn| conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE live_fixture (value TEXT); INSERT INTO live_fixture VALUES ('live');").unwrap());
    let kv = plain_rs::kv::Db::open(&paths.database()).unwrap();
    kv.insert("old-only", "live").unwrap();
    prepare(archive(), directory.path()).unwrap();
    assert_eq!(
        db.with_conn(
            |conn| conn.query_row("SELECT value FROM live_fixture", [], |row| row
                .get::<_, String>(0))
        )
        .unwrap(),
        "live"
    );
    assert_eq!(
        prefs.get::<String>("device_name").unwrap().as_deref(),
        Some("live device")
    );
    assert!(kv.get("restored").unwrap().is_none());
    assert!(paths.pending_restore().exists());
    drop(kv);
    drop(db);
    drop(prefs);
    std::fs::write(directory.path().join("plain.db-wal"), b"stale WAL").unwrap();
    std::fs::write(directory.path().join("plain.db-shm"), b"stale SHM").unwrap();
    let cache = tempfile::tempdir().unwrap();
    std::fs::write(cache.path().join("stale"), b"stale cache").unwrap();
    let _lease =
        crate::assembly::data_directory::DataDirectory::open(directory.path(), cache.path())
            .unwrap();
    assert_restored(directory.path());
    assert!(cache.path().is_dir());
    assert!(!cache.path().join("stale").exists());
    assert!(paths.note_images().is_dir());
    assert!(paths.audio().is_dir());
}
#[test]
fn startup_resumes_after_completed_and_interrupted_entry_swaps() {
    let directory = tempfile::tempdir().unwrap();
    prepare(archive(), directory.path()).unwrap();
    let pending = pending(directory.path());
    let stage = directory.path().join(&pending.staging);
    let completed = pending.entries.first().unwrap();
    std::fs::rename(
        stage.join("payload").join(completed),
        directory.path().join(completed),
    )
    .unwrap();
    std::fs::write(directory.path().join("plain.db"), b"old database").unwrap();
    std::fs::rename(
        directory.path().join("plain.db"),
        stage.join("old/plain.db"),
    )
    .unwrap();
    apply(directory.path()).unwrap();
    assert_restored(directory.path());
    apply(directory.path()).unwrap();
    assert_eq!(
        std::fs::read(directory.path().join("files/blob")).unwrap(),
        b"attachment"
    );
}
#[test]
fn invalid_backup_paths_duplicates_and_missing_stores_never_publish_a_restore() {
    for names in [
        vec!["../outside"],
        vec!["files/../outside"],
        vec!["files/.core.lock"],
        vec!["files/plain.db-wal"],
        vec!["files/same", "files/same/"],
        vec!["files/plain.db"],
    ] {
        let directory = tempfile::tempdir().unwrap();
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in names {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"not a backup").unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        assert!(prepare(bytes, directory.path()).is_err());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}
#[test]
fn cancelled_and_competing_restores_preserve_the_published_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = archive();
    let (_stop, stopped) = watch::channel(false);
    assert!(
        stage_reader(
            Cursor::new(&bytes),
            directory.path(),
            &stopped,
            &AtomicBool::new(true)
        )
        .is_err()
    );
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    prepare(bytes.clone(), directory.path()).unwrap();
    let first = pending(directory.path()).staging;
    assert!(prepare(bytes, directory.path()).is_err());
    assert_eq!(pending(directory.path()).staging, first);
    apply(directory.path()).unwrap();
    assert_restored(directory.path());
}
#[test]
fn invalid_restore_plan_does_not_replace_current_data() {
    let directory = tempfile::tempdir().unwrap();
    prepare(archive(), directory.path()).unwrap();
    let mut marker = pending(directory.path());
    marker.entries.push("../outside".into());
    std::fs::write(directory.path().join("plain.db"), b"current data").unwrap();
    std::fs::write(
        plain_rs::data_paths::DataPaths::new(directory.path()).pending_restore(),
        serde_json::to_vec(&marker).unwrap(),
    )
    .unwrap();
    assert!(apply(directory.path()).is_err());
    assert_eq!(
        std::fs::read(directory.path().join("plain.db")).unwrap(),
        b"current data"
    );
}
#[cfg(unix)]
#[test]
fn staged_symbolic_links_are_rejected_before_any_target_is_changed() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    prepare(archive(), directory.path()).unwrap();
    let marker = pending(directory.path());
    std::os::unix::fs::symlink(
        outside.path(),
        directory
            .path()
            .join(marker.staging)
            .join("payload/files/escape"),
    )
    .unwrap();
    assert!(apply(directory.path()).is_err());
    assert!(!directory.path().join("plain.db").exists());
}

#[tokio::test]
async fn native_restore_stages_without_overwriting_the_running_database() {
    let directory = tempfile::tempdir().unwrap();
    let archive_path = directory.path().join("selected-backup.zip");
    std::fs::write(&archive_path, archive()).unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state.db.with_conn(|conn| {
        conn.execute_batch(
            "CREATE TABLE live_fixture (value TEXT); INSERT INTO live_fixture VALUES ('live');",
        )
        .unwrap()
    });
    assert!(
        core.native_management()
            .unwrap()
            .stage_app_restore("app://selected-backup.zip".into())
            .await
            .unwrap()
    );
    assert_eq!(
        state
            .db
            .with_conn(
                |conn| conn.query_row("SELECT value FROM live_fixture", [], |row| row
                    .get::<_, String>(0))
            )
            .unwrap(),
        "live"
    );
    assert!(
        crate::assembly::data_directory::DataDirectory::open(
            directory.path(),
            core.cache_paths().root()
        )
        .err()
        .unwrap()
        .contains("process restart")
    );
    drop(state);
    core.shutdown().await;
}

#[tokio::test]
async fn uri_restore_reads_one_bounded_descriptor_without_copying_the_archive_to_cache() {
    for uri in ["content://backup/current", "ph://backup/current"] {
        let directory = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let bytes = archive();
        let path = source.path().join("wrapped-backup");
        let mut wrapped = b"unrelated-prefix".to_vec();
        let offset = wrapped.len() as i64;
        wrapped.extend_from_slice(&bytes);
        wrapped.extend_from_slice(b"unrelated-suffix");
        std::fs::write(&path, wrapped).unwrap();
        let host = Arc::new(crate::host::bridge::Host::default());
        let (generation, mut requests) = host.connect();
        let resources = Arc::new(NativeResources::new(host.clone()));
        let (_stop, stopped) = watch::channel(false);
        let target = directory.path().to_path_buf();
        let reader = resources.clone();
        let restoring = tokio::spawn(async move {
            stage(
                uri.to_owned(),
                target.clone(),
                target.join("cache"),
                reader,
                stopped,
            )
            .await
        });
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "fileResourceStream");
        let stream = resources
            .attach_native(request["params"]["id"].as_str().unwrap(), generation)
            .unwrap();
        host.reply(
            generation,
            serde_json::json!({"id":request["id"],"result":null}),
        )
        .unwrap();
        let metadata: serde_json::Value =
            serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
        assert_eq!(metadata, serde_json::json!({"path":uri,"file":true}));
        stream
            .file(
                crate::native::resource_stream::NativeResourceFile::new(
                    File::open(&path).unwrap(),
                    offset,
                    bytes.len() as i64,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        restoring.await.unwrap().unwrap();
        assert!(requests.try_recv().is_err());
        let paths = plain_rs::data_paths::DataPaths::new(directory.path());
        assert!(!directory.path().join("cache").exists());
        assert!(paths.pending_restore().is_file());
        assert!(!paths.database().exists());
        apply(directory.path()).unwrap();
        assert_restored(directory.path());
    }
}
