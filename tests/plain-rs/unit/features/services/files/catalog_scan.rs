use super::*;

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn file_scan_cancel_aborts_pending_reads_and_keeps_catalog_unchanged() {
    use crate::features::services::files::media_scans::Status;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.txt");
    std::fs::write(&path, "document").unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let mut catalog = NativeMediaCatalog::new(core.runtime_state());
    let gate = std::sync::Arc::new((tokio::sync::Notify::new(), tokio::sync::Semaphore::new(0)));
    catalog.read_gate = Some(gate.clone());
    let (_generation, mut requests) =
        crate::test_support::HostRequests::connect(&core.runtime_state().host);
    let worker = tokio::spawn({
        let catalog = catalog.clone();
        let path = path.clone();
        async move {
            catalog
                .scan_files(
                    crate::features::enums::DataType::Doc,
                    vec![path.to_str().unwrap().into()],
                )
                .await
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), gate.0.notified())
        .await
        .unwrap();
    assert_eq!(
        catalog.file_scan_status().unwrap().unwrap().status,
        Status::Running
    );
    assert!(
        catalog
            .scan_files(crate::features::enums::DataType::Doc, vec![])
            .await
            .is_err()
    );
    assert_eq!(
        catalog.cancel_file_scan().unwrap().unwrap().status,
        Status::Cancelling
    );
    let result = worker.await.unwrap().unwrap();
    assert_eq!(result.status, Status::Cancelled);
    assert_eq!(result.processed_files, 0);
    assert!(requests.try_recv().is_err());
    assert_eq!(
        catalog
            .count(crate::features::enums::DataType::Doc)
            .unwrap(),
        0
    );
    assert_eq!(
        catalog
            .scan_files(crate::features::enums::DataType::Doc, vec![])
            .await
            .unwrap()
            .status,
        Status::Completed
    );
    core.stop_core().await;
    assert!(catalog.file_scan_status().is_err());
    assert!(core.runtime_state().media_scans.begin(1).is_err());
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn aborted_scan_future_releases_the_shared_job_and_failed_scans_report_partial_progress() {
    use crate::features::services::files::media_scans::Status;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.txt");
    std::fs::write(&path, "document").unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let mut catalog = NativeMediaCatalog::new(core.runtime_state());
    let gate = std::sync::Arc::new((tokio::sync::Notify::new(), tokio::sync::Semaphore::new(0)));
    catalog.read_gate = Some(gate.clone());
    let (_generation, mut requests) =
        crate::test_support::HostRequests::connect(&core.runtime_state().host);
    let worker = tokio::spawn({
        let catalog = catalog.clone();
        let path = path.clone();
        async move {
            catalog
                .scan_files(
                    crate::features::enums::DataType::Doc,
                    vec![path.to_str().unwrap().into()],
                )
                .await
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), gate.0.notified())
        .await
        .unwrap();
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    assert_eq!(
        catalog.file_scan_status().unwrap().unwrap().status,
        Status::Cancelled
    );
    catalog.read_gate = None;
    let result = catalog
        .scan_files(
            crate::features::enums::DataType::Doc,
            vec![path.to_str().unwrap().into(), "invalid-reference".into()],
        )
        .await
        .unwrap();
    assert_eq!(result.status, Status::Failed);
    assert_eq!(result.total_files, Some(2));
    assert_eq!(result.processed_files, 1);
    assert!(result.error.is_some());
    assert_eq!(
        catalog
            .count(crate::features::enums::DataType::Doc)
            .unwrap(),
        1
    );
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn directory_scan_cancellation_preserves_basic_facts_before_metadata() {
    use crate::features::services::files::media_scans::Status;
    let directory = directory_scan_fixture();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let mut catalog = NativeMediaCatalog::new(core.runtime_state());
    catalog
        .set_source_directories(&[directory.path().to_string_lossy().into_owned()])
        .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let root = directory.path().join("files");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("visible.txt"), b"data").unwrap();
    let gate = std::sync::Arc::new((tokio::sync::Notify::new(), tokio::sync::Semaphore::new(0)));
    catalog.read_gate = Some(gate.clone());
    let (_generation, mut requests) =
        crate::test_support::HostRequests::connect(&core.runtime_state().host);
    let worker = tokio::spawn({
        let catalog = catalog.clone();
        let root = root.to_str().unwrap().to_owned();
        async move { catalog.scan_directory(&root).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), gate.0.notified())
        .await
        .unwrap();
    assert_eq!(
        catalog.file_scan_status().unwrap().unwrap().total_files,
        Some(1)
    );
    assert!(catalog.configure_scan_directories(None).is_err());
    catalog.cancel_file_scan().unwrap();
    let result = worker.await.unwrap().unwrap();
    assert_eq!(result.status, Status::Cancelled);
    assert_eq!(result.processed_files, 1);
    assert_eq!(
        catalog
            .count(crate::features::enums::DataType::Doc)
            .unwrap(),
        1
    );
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn directory_scan_cancellation_after_discovery_preserves_all_basic_facts() {
    use crate::features::services::files::media_scans::Status;
    let directory = directory_scan_fixture();
    let root = directory.path().join("files");
    std::fs::create_dir(&root).unwrap();
    for index in 0..96 {
        std::fs::write(root.join(format!("{index}.txt")), b"data").unwrap();
    }
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let mut catalog = NativeMediaCatalog::new(core.runtime_state());
    catalog
        .set_source_directories(&[directory.path().to_string_lossy().into_owned()])
        .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let gate = std::sync::Arc::new((tokio::sync::Notify::new(), tokio::sync::Semaphore::new(0)));
    catalog.read_gate = Some(gate.clone());
    let (_generation, mut requests) =
        crate::test_support::HostRequests::connect(&core.runtime_state().host);
    let worker = tokio::spawn({
        let catalog = catalog.clone();
        let root = root.to_str().unwrap().to_owned();
        async move { catalog.scan_directory(&root).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), gate.0.notified())
        .await
        .unwrap();
    catalog.cancel_file_scan().unwrap();
    let result = worker.await.unwrap().unwrap();
    assert_eq!(result.status, Status::Cancelled);
    assert_eq!(result.processed_files, 96);
    assert_eq!(result.total_files, Some(96));
    assert!(requests.try_recv().is_err());
    assert_eq!(
        catalog
            .count(crate::features::enums::DataType::Doc)
            .unwrap(),
        96
    );
    core.stop_core().await;
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn source_directory_scan_keeps_one_session_across_roots_and_partial_failure() {
    use crate::features::services::files::media_scans::Status;
    let directory = directory_scan_fixture();
    let roots = [
        directory.path().join("first"),
        directory.path().join("second"),
    ];
    for root in &roots {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(root.join("document.txt"), b"document").unwrap();
    }
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let mut catalog = NativeMediaCatalog::new(core.runtime_state());
    catalog
        .set_source_directories(&[directory.path().to_string_lossy().into_owned()])
        .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let sources = roots
        .iter()
        .map(|root| root.to_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    catalog.set_source_directories(&sources).unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let gate = std::sync::Arc::new((tokio::sync::Notify::new(), tokio::sync::Semaphore::new(0)));
    catalog.read_gate = Some(gate.clone());
    let (_generation, mut requests) =
        crate::test_support::HostRequests::connect(&core.runtime_state().host);
    let worker = tokio::spawn({
        let catalog = catalog.clone();
        async move { catalog.scan_source_directories("/").await }
    });
    let mut session_id = None;
    for _ in 0..2 {
        tokio::time::timeout(std::time::Duration::from_secs(5), gate.0.notified())
            .await
            .unwrap();
        let snapshot = catalog.file_scan_status().unwrap().unwrap();
        assert_eq!(snapshot.status, Status::Running);
        assert_eq!(snapshot.processed_files, 2);
        assert_eq!(snapshot.total_files, Some(2));
        if let Some(id) = &session_id {
            assert_eq!(*id, snapshot.id);
        } else {
            session_id = Some(snapshot.id);
        }
        gate.1.add_permits(1);
    }
    let result = worker.await.unwrap().unwrap();
    assert_eq!(result.status, Status::Completed);
    assert_eq!(result.total_files, Some(2));
    assert_eq!(result.processed_files, 2);
    assert_eq!(
        catalog
            .count(crate::features::enums::DataType::Doc)
            .unwrap(),
        2
    );

    catalog.read_gate = None;
    catalog
        .set_source_directories(&[directory.path().to_string_lossy().into_owned()])
        .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let result = catalog
        .scan_directories(vec![
            sources[0].clone(),
            directory
                .path()
                .join("missing")
                .to_str()
                .unwrap()
                .to_owned(),
        ])
        .await
        .unwrap();
    assert!(requests.try_recv().is_err());
    assert_eq!(result.status, Status::Failed);
    assert_eq!(result.processed_files, 1);
    assert_eq!(result.total_files, None);
    assert!(result.error.is_some());
    assert_ne!(
        result.id,
        catalog.scan_directories(Vec::new()).await.unwrap().id
    );
    core.stop_core().await;
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn background_source_scan_is_busy_before_the_first_read_and_core_stop_joins_it() {
    let directory = directory_scan_fixture();
    let root = directory.path().join("library");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("document.txt"), b"document").unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let mut catalog = NativeMediaCatalog::new(core.runtime_state());
    catalog
        .set_source_directories(&[directory.path().to_string_lossy().into_owned()])
        .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    catalog
        .set_source_directories(&[root.to_str().unwrap().to_owned()])
        .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let gate = std::sync::Arc::new((tokio::sync::Notify::new(), tokio::sync::Semaphore::new(0)));
    catalog.read_gate = Some(gate.clone());
    let (_generation, mut requests) =
        crate::test_support::HostRequests::connect(&core.runtime_state().host);
    let started = catalog.start_source_scan("/").unwrap();
    assert!(catalog.start_source_scan("/").is_err());
    tokio::time::timeout(std::time::Duration::from_secs(5), gate.0.notified())
        .await
        .unwrap();
    assert_eq!(catalog.file_scan_status().unwrap().unwrap().id, started.id);
    core.stop_core().await;
    assert_eq!(
        core.runtime_state()
            .media_scans
            .snapshot()
            .unwrap()
            .unwrap()
            .status,
        crate::features::services::files::media_scans::Status::Cancelled
    );
    assert!(requests.try_recv().is_err());
    assert_eq!(
        crate::features::services::files::media_catalog::count(
            &core.runtime_state().db,
            crate::features::enums::DataType::Doc,
            ""
        )
        .unwrap(),
        1
    );
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn native_catalog_records_and_removal_publish_one_typed_change_per_batch() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("song.mp3");
    std::fs::write(&path, b"unrecognized audio bytes").unwrap();
    let path = path.canonicalize().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let catalog = NativeMediaCatalog::new(core.runtime_state());
    let mut events = core.runtime_state().events.subscribe();
    let file = crate::features::media::file_facts::read_file_metadata(
        &path,
        crate::features::enums::DataType::Audio,
    )
    .unwrap();
    catalog.record_files(&[file.clone(), file]).unwrap();
    let event = events.try_recv().unwrap();
    assert_eq!(
        event.event_type,
        crate::infrastructure::events::MEDIA_LIBRARY_UPDATED
    );
    assert_eq!(event.payload, r#"{"types":[1]}"#);
    assert!(events.try_recv().is_err());
    assert_eq!(catalog.forget_path(path.to_str().unwrap()).unwrap(), 1);
    assert_eq!(
        events.try_recv().unwrap().event_type,
        crate::infrastructure::events::MEDIA_LIBRARY_UPDATED
    );
    assert!(events.try_recv().is_err());
    assert_eq!(catalog.forget_path(path.to_str().unwrap()).unwrap(), 0);
    assert!(events.try_recv().is_err());
    core.stop_core().await;
}

fn directory_scan_fixture() -> tempfile::TempDir {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    tempfile::Builder::new()
        .prefix("media-scan-")
        .tempdir_in(home)
        .unwrap()
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn basic_catalog_is_searchable_before_a_blocked_media_probe() {
    let directory = directory_scan_fixture();
    let source = directory.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("document.txt"), b"document").unwrap();
    std::fs::write(source.join("source.rs"), b"fn main() {}").unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let mut catalog = NativeMediaCatalog::new(core.runtime_state());
    let gate = std::sync::Arc::new((tokio::sync::Notify::new(), tokio::sync::Semaphore::new(0)));
    catalog.read_gate = Some(gate.clone());
    let worker = tokio::spawn({
        let catalog = catalog.clone();
        let source = source.clone();
        async move { catalog.scan_directory(source.to_str().unwrap()).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), gate.0.notified())
        .await
        .unwrap();
    let rows = crate::features::services::files::indexed::native(
        &core.runtime_state().db,
        "text:source.rs",
        0,
        10,
        crate::features::prefs::settings::FileSortBy::NameAsc,
        false,
    )
    .unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(catalog.count(DataType::Doc).unwrap(), 1);
    gate.1.add_permits(1);
    assert_eq!(
        worker.await.unwrap().unwrap().status,
        crate::features::services::files::media_scans::Status::Completed
    );
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn exclusion_changes_remove_results_and_reinclude_without_touching_originals() {
    use crate::features::prefs::pref_keys::{FILE_INDEX_EXCLUDED_DIRS, MEDIA_SOURCE_DIRS};
    let directory = directory_scan_fixture();
    let source = directory.path().join("source");
    let private = source.join("private");
    std::fs::create_dir_all(&private).unwrap();
    let original = private.join("report.txt");
    std::fs::write(&original, b"original contents").unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    state
        .prefs
        .set(MEDIA_SOURCE_DIRS, vec![source.to_str().unwrap()])
        .unwrap();
    let catalog = NativeMediaCatalog::new(state.clone());
    catalog
        .set_source_directories(&[directory.path().to_string_lossy().into_owned()])
        .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    catalog
        .scan_directory(source.to_str().unwrap())
        .await
        .unwrap();
    let id = state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT file_id FROM file_catalog WHERE file_name='report.txt'",
                [],
                |row| row.get::<_, String>(0),
            )
        })
        .unwrap();
    state
        .db
        .with_conn(|conn| conn.execute("UPDATE files SET favorited=1 WHERE id=?1", [&id]))
        .unwrap();
    let preferences =
        crate::features::services::settings::preferences::Preferences::from_server(&state);
    preferences
        .set(
            FILE_INDEX_EXCLUDED_DIRS,
            serde_json::json!([private.to_str().unwrap()]),
        )
        .await
        .unwrap();
    assert_eq!(
        crate::features::services::files::indexed::native(
            &state.db,
            "text:report.txt",
            0,
            0,
            crate::features::prefs::settings::FileSortBy::NameAsc,
            true
        )
        .unwrap(),
        serde_json::json!(0)
    );
    catalog
        .scan_file(
            crate::features::enums::DataType::Doc,
            original.to_str().unwrap(),
        )
        .await
        .unwrap();
    catalog
        .scan_completed_files(serde_json::json!({"paths":[original.to_str().unwrap()]}))
        .await
        .unwrap();
    assert_eq!(
        crate::features::services::files::indexed::native(
            &state.db,
            "text:report.txt",
            0,
            0,
            crate::features::prefs::settings::FileSortBy::NameAsc,
            true
        )
        .unwrap(),
        serde_json::json!(0)
    );
    assert_eq!(std::fs::read(&original).unwrap(), b"original contents");
    preferences
        .set(FILE_INDEX_EXCLUDED_DIRS, serde_json::json!([]))
        .await
        .unwrap();
    state.media_scans.wait_background().await.unwrap();
    assert_eq!(
        crate::features::services::files::indexed::native(
            &state.db,
            "text:report.txt",
            0,
            0,
            crate::features::prefs::settings::FileSortBy::NameAsc,
            true
        )
        .unwrap(),
        serde_json::json!(1)
    );
    assert_eq!(
        state
            .db
            .with_conn(|conn| conn.query_row(
                "SELECT id,favorited,missing FROM files WHERE id=?1",
                [&id],
                |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, bool>(2)?
                ))
            ))
            .unwrap(),
        (id, true, false)
    );
    assert_eq!(std::fs::read(&original).unwrap(), b"original contents");
    core.stop_core().await;
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn an_unmounted_source_is_a_coverage_gap_and_does_not_retire_catalog_rows() {
    let directory = directory_scan_fixture();
    let source = directory.path().join("disk");
    std::fs::create_dir(&source).unwrap();
    let source = source.canonicalize().unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let id = state.db.with_conn(|connection| -> rusqlite::Result<String> {
        connection.execute("INSERT INTO volumes(id,identity,path,is_available) VALUES('offline-fixture','offline-fixture',?1,1)", [source.to_str().unwrap()])?;
        let volume_id: String = connection.query_row("SELECT id FROM volumes ORDER BY rowid DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        connection.execute("INSERT INTO files(id,volume_id,path,file_name,kind,updated_at,favorited) VALUES('file-'||'missing.rs',?1,'missing.rs','missing.rs','FILE','2026-01-01T00:00:00.000Z',1)", [volume_id])?;
        Ok("file-missing.rs".into())
    }).unwrap();
    let catalog = NativeMediaCatalog::new(state.clone());
    catalog
        .set_source_directories(&[source.to_string_lossy().into_owned()])
        .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let result = catalog.scan_source_directories("/").await.unwrap();
    assert_eq!(
        result.status,
        crate::features::services::files::media_scans::Status::Failed
    );
    assert_eq!(result.total_files, None);
    assert!(result.error.unwrap().contains("volume is offline"));
    let row = state.db.file(&id).unwrap().unwrap();
    assert!(!row.missing);
    assert!(row.favorited);
    assert!(!state.db.volumes().unwrap()[0].is_available);
    core.stop_core().await;
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn watcher_reconciliation_waits_for_the_active_scan() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let catalog = NativeMediaCatalog::new(state.clone());
    catalog.set_source_directories(&[]).unwrap();
    let active = state.media_scans.begin(0).unwrap();
    let reconcile = catalog.apply_watch_event(notify::Event::new(notify::EventKind::Other));
    tokio::pin!(reconcile);
    assert!(futures_util::poll!(reconcile.as_mut()).is_pending());
    active.finish(Ok(())).unwrap();
    reconcile.await.unwrap();
    core.stop_core().await;
}

#[cfg(all(unix, feature = "runtime-media"))]
#[tokio::test]
async fn scanning_skips_unreadable_directories_and_preserves_their_existing_rows() {
    use std::os::unix::fs::PermissionsExt;
    let directory = directory_scan_fixture();
    let source = directory.path().join("source");
    let denied = source.join("denied");
    std::fs::create_dir_all(&denied).unwrap();
    let private = denied.join("old.rs");
    std::fs::write(&private, b"private").unwrap();
    let public = source.join("readable.rs");
    std::fs::write(&public, b"public").unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let id = state
        .db
        .record_file_paths(&[private.to_str().unwrap().into()], None)
        .unwrap()[0]
        .id
        .clone();
    let catalog = NativeMediaCatalog::new(state.clone());
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0)).unwrap();
    let denied_by_os = std::fs::read_dir(&denied).is_err();
    let result = catalog.scan_directory(source.to_str().unwrap()).await;
    let denied_result = catalog.scan_directory(denied.to_str().unwrap()).await;
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o700)).unwrap();
    let denied_result = denied_result.unwrap();
    assert_eq!(
        denied_result.status,
        crate::features::services::files::media_scans::Status::Completed
    );
    if denied_by_os {
        assert!(matches!(
            denied_result.sources[0].state,
            crate::features::services::files::media_scans::CoverageState::Excluded
        ));
    }
    let result = result.unwrap();
    assert_eq!(
        result.status,
        crate::features::services::files::media_scans::Status::Completed
    );
    assert!(result.error.is_none());
    assert!(!state.db.file(&id).unwrap().unwrap().missing);
    assert_eq!(
        state
            .db
            .with_conn(|connection| connection.query_row(
                "SELECT COUNT(*) FROM files WHERE file_name='readable.rs' AND missing=0",
                [],
                |row| row.get::<_, i64>(0)
            ))
            .unwrap(),
        1
    );
    core.stop_core().await;
}

#[cfg(all(feature = "runtime-media", feature = "media"))]
#[tokio::test]
async fn indexing_does_not_generate_thumbnails_or_delete_disk_cache() {
    use std::sync::atomic::Ordering;
    let sources = directory_scan_fixture();
    let path = sources.path().join("photo.png");
    image::RgbaImage::from_pixel(768, 768, image::Rgba([40, 90, 150, 128]))
        .save(&path)
        .unwrap();
    let data = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(data.path()).unwrap();
    let state = core.runtime_state();
    let catalog = NativeMediaCatalog::new(state.clone());
    let cache = state.cache.root().join("thumbs");
    std::fs::create_dir_all(&cache).unwrap();
    let retained = cache.join("retained.jpg");
    std::fs::File::create(&retained)
        .unwrap()
        .set_len(257 * 1024 * 1024)
        .unwrap();
    catalog
        .scan_directory(sources.path().to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(catalog.count(DataType::Image).unwrap(), 1);
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 0);
    assert!(retained.exists());
    let request = crate::features::services::images::thumbnails::Request {
        path: path.to_string_lossy().into_owned(),
        width: 256,
        height: 256,
        center_crop: true,
        file_id: String::new(),
        access_order: None,
        file_name: "photo.png".into(),
    };
    let first = state
        .thumbnails
        .get(&state, request.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(first.starts_with(b"\x89PNG\r\n\x1a\n"));
    let second = state
        .thumbnails
        .get(&state, request)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 1);
    catalog
        .scan_directory(sources.path().to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::metadata(retained).unwrap().len(),
        257 * 1024 * 1024
    );
    core.stop_core().await;
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn sdk_mount_initializes_default_index_root_before_startup_scan() {
    use serde_json::json;
    let directory = directory_scan_fixture();
    let documents = directory.path().join("Documents");
    std::fs::create_dir(&documents).unwrap();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
    for file in ["audio-tagged.mp3", "video-h264-baseline.mp4"] {
        std::fs::copy(fixtures.join(file), documents.join(file)).unwrap();
    }
    std::fs::write(documents.join("document.pdf"), b"%PDF-1.4\n").unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("private")).unwrap();
    let state = core.runtime_state();
    state.file_platform.use_sdk_for_test();
    state
        .media_scans
        .set_defaults(crate::features::prefs::file_index::Defaults {
            scan_directories: vec![],
            excluded_directories: vec![],
            excluded_names: vec![],
        })
        .unwrap();
    let (generation, mut requests) = state.host.connect();
    let host = state.host.clone();
    let root = documents.to_string_lossy().into_owned();
    let expected = documents
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let replies = tokio::spawn(async move {
        let mut mount_calls = 0;
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["MEDIA_SCAN"]),
                "systemMountFacts" => {
                    mount_calls += 1;
                    json!([
                        {"path":root,"driveType":"INTERNAL_STORAGE"},
                        {"path":"/private-data","driveType":"APP"}
                    ])
                }
                "systemAccessFacts" => {
                    assert_eq!(request["params"]["features"], json!(["FILES"]));
                    json!({"granted":{"FILES":true}})
                }
                method => panic!("Unexpected host request: {method}"),
            };
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
        mount_calls
    });
    host_connected(state.clone(), generation).await.unwrap();
    assert_eq!(
        NativeMediaCatalog::new(state.clone())
            .source_directories()
            .unwrap(),
        vec![expected]
    );
    state.media_scans.wait_background().await.unwrap();
    let snapshot = state.media_scans.snapshot().unwrap().unwrap();
    assert_eq!(
        snapshot.status,
        crate::features::services::files::media_scans::Status::Completed,
        "{snapshot:?}"
    );
    let catalog = NativeMediaCatalog::new(state.clone());
    for kind in [DataType::Audio, DataType::Video, DataType::Doc] {
        assert_eq!(catalog.count(kind).unwrap(), 1, "{kind:?}");
    }
    state.host.disconnect(generation);
    assert_eq!(replies.await.unwrap(), 1);
    core.stop_core().await;
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn changed_directory_grants_refresh_all_shared_media_lists_without_copying() {
    use serde_json::json;
    let directory = directory_scan_fixture();
    let folders: Vec<_> = ["Documents", "Music"].into_iter().map(|name| directory.path().join(name)).collect();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
    for folder in &folders {
        std::fs::create_dir(folder).unwrap();
        for name in ["audio-tagged.mp3", "video-h264-baseline.mp4"] {
            std::fs::copy(fixtures.join(name), folder.join(name)).unwrap();
        }
        std::fs::write(folder.join("document.pdf"), b"%PDF-1.4\n").unwrap();
    }
    let core = crate::test_runtime::Core::open(&directory.path().join("private")).unwrap();
    let state = core.runtime_state();
    state.file_platform.use_sdk_for_test();
    state.media_scans.set_defaults(crate::features::prefs::file_index::Defaults {
        scan_directories: vec![], excluded_directories: vec![], excluded_names: vec![],
    }).unwrap();
    let grants = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let shared = grants.clone();
    let (generation, mut requests) = state.host.connect();
    let host = state.host.clone();
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["MEDIA_SCAN"]),
                "systemMountFacts" => json!(shared.lock().unwrap().iter().map(|path| json!({"path":path,"driveType":"INTERNAL_STORAGE"})).collect::<Vec<_>>()),
                "systemAccessFacts" => json!({"granted":{"FILES":true}}),
                method => panic!("Unexpected host request: {method}"),
            };
            host.reply(generation, json!({"id":request["id"],"result":result})).unwrap();
        }
    });
    let catalog = NativeMediaCatalog::new(state.clone());
    for indices in [vec![0], vec![0, 1], vec![1]] {
        *grants.lock().unwrap() = indices.iter().map(|index| folders[*index].to_string_lossy().into_owned()).collect();
        access_changed(state.clone(), generation).await.unwrap();
        state.media_scans.wait_background().await.unwrap();
        let expected = indices.len() as i64;
        for kind in [DataType::Audio, DataType::Video, DataType::Doc] {
            assert_eq!(catalog.count(kind).unwrap(), expected, "{kind:?}");
        }
    }
    assert_eq!(std::fs::read(folders[0].join("audio-tagged.mp3")).unwrap(), std::fs::read(fixtures.join("audio-tagged.mp3")).unwrap());
    assert!(folders[0].join("document.pdf").is_file());
    state.host.disconnect(generation);
    responder.await.unwrap();
    core.stop_core().await;
}
