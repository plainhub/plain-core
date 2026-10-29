#[tokio::test]
async fn catalog_is_created_from_core_and_stopped_handles_reject_access() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    assert_eq!(catalog.count(plain_rs::enums::DataType::Image).unwrap(), 0);
    assert!(catalog.record_files(&[]).unwrap().is_empty());
    assert!(
        catalog
            .list(plain_rs::enums::DataType::Image, 10, 0)
            .unwrap()
            .is_empty()
    );
    core.stop_core().await;
    assert!(core.native_media_catalog().is_err());
    assert!(catalog.count(plain_rs::enums::DataType::Image).is_err());
    assert!(
        catalog
            .by_path(plain_rs::enums::DataType::Image, "/photo.jpg")
            .is_err()
    );
    assert!(
        catalog
            .list(plain_rs::enums::DataType::Image, 10, 0)
            .is_err()
    );
    assert!(catalog.record_files(&[]).is_err());
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
#[tokio::test]
async fn native_file_metadata_reaches_catalog_without_any_host_request() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.txt");
    std::fs::write(&path, "document").unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let host = core.runtime_state().host.clone();
    let (_generation, mut requests) = crate::test_support::HostRequests::connect(&host);
    let catalog = core.native_media_catalog().unwrap();
    let indexed = catalog
        .scan_file(plain_rs::enums::DataType::Doc, path.to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(indexed.size_bytes, 8);
    assert_eq!(
        catalog
            .count_matching(plain_rs::enums::DataType::Doc, "ext:txt")
            .unwrap(),
        1
    );
    let rows = catalog
        .search(
            plain_rs::enums::DataType::Doc,
            "ext:txt",
            crate::graphql::resolvers::files::types::FileSortBy::NameAsc,
            10,
            0,
        )
        .unwrap()
        .iter()
        .map(crate::services::files::media_catalog_facts::row_facts)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(rows[0]["id"], indexed.id);
    assert_eq!(rows[0]["size"], 8);
    assert!(!rows[0].as_object().unwrap().contains_key("originalPath"));
    std::fs::remove_file(&path).unwrap();
    assert!(
        catalog
            .scan_file(plain_rs::enums::DataType::Doc, path.to_str().unwrap())
            .await
            .is_err()
    );
    assert_eq!(catalog.count(plain_rs::enums::DataType::Doc).unwrap(), 1);
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
    assert!(
        catalog
            .scan_file(plain_rs::enums::DataType::Doc, path.to_str().unwrap())
            .await
            .is_err()
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn rescan_with_failed_probe_retains_current_known_metadata_without_host_round_trips() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("damaged.mp3");
    std::fs::write(&path, b"unrecognized audio bytes").unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    let mut known = plain_media::metadata::file_facts::read_file_metadata(
        &path,
        plain_rs::enums::DataType::Audio,
    )
    .unwrap();
    known.title = "Known title".into();
    known.artist = "Known artist".into();
    known.duration_ms = 1500;
    known.cover_image_id = Some("known-album".into());
    known.size_bytes = 999;
    let original = catalog.record_files(&[known]).unwrap().remove(0);
    let (_generation, mut requests) =
        crate::test_support::HostRequests::connect(&core.runtime_state().host);
    let updated = catalog
        .scan_file(plain_rs::enums::DataType::Audio, path.to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(updated.id, original.id);
    assert_eq!(updated.title, original.title);
    assert_eq!(updated.artist, original.artist);
    assert_eq!(updated.duration_ms, original.duration_ms);
    assert_eq!(updated.cover_image_id, original.cover_image_id);
    assert_eq!(
        updated.size_bytes,
        std::fs::metadata(path).unwrap().len() as i64
    );
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
    assert!(catalog.record_file_reads(&[]).is_err());
}

#[cfg(feature = "media")]
#[tokio::test]
async fn file_scan_cancel_aborts_pending_reads_and_keeps_catalog_unchanged() {
    use crate::services::files::media_scans::Status;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.txt");
    std::fs::write(&path, "document").unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let mut catalog = core.native_media_catalog().unwrap();
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
                    plain_rs::enums::DataType::Doc,
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
            .scan_files(plain_rs::enums::DataType::Doc, vec![])
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
    assert_eq!(catalog.count(plain_rs::enums::DataType::Doc).unwrap(), 0);
    assert_eq!(
        catalog
            .scan_files(plain_rs::enums::DataType::Doc, vec![])
            .await
            .unwrap()
            .status,
        Status::Completed
    );
    core.stop_core().await;
    assert!(catalog.file_scan_status().is_err());
    assert!(core.runtime_state().media_scans.begin(1).is_err());
}

#[cfg(feature = "media")]
#[tokio::test]
async fn aborted_scan_future_releases_the_shared_job_and_failed_scans_report_partial_progress() {
    use crate::services::files::media_scans::Status;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.txt");
    std::fs::write(&path, "document").unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let mut catalog = core.native_media_catalog().unwrap();
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
                    plain_rs::enums::DataType::Doc,
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
            plain_rs::enums::DataType::Doc,
            vec![path.to_str().unwrap().into(), "invalid-reference".into()],
        )
        .await
        .unwrap();
    assert_eq!(result.status, Status::Failed);
    assert_eq!(result.total_files, Some(2));
    assert_eq!(result.processed_files, 1);
    assert!(result.error.is_some());
    assert_eq!(catalog.count(plain_rs::enums::DataType::Doc).unwrap(), 1);
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(feature = "media")]
fn directory_scan_fixture() -> tempfile::TempDir {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    tempfile::Builder::new()
        .prefix("media-scan-")
        .tempdir_in(home)
        .unwrap()
}

#[cfg(feature = "media")]
#[tokio::test]
async fn directory_scan_uses_core_policy_real_walker_and_existing_catalog() {
    use crate::services::files::media_scans::Status;
    let directory = directory_scan_fixture();
    let root = directory.path().join("library");
    let data = root.join("core");
    let cache = root.join("system-cache");
    let excluded = root.join("excluded");
    for folder in [
        &root,
        &cache,
        &excluded,
        &root.join("nested"),
        &root.join(".hidden"),
        &root.join("marked"),
    ] {
        std::fs::create_dir_all(folder).unwrap();
    }
    for name in [
        "one.txt",
        "nested/two.txt",
        "photo.heic",
        "excluded/excluded.txt",
        ".hidden/hidden.txt",
        "marked/marked.txt",
    ] {
        std::fs::write(root.join(name), b"media facts").unwrap();
    }
    std::fs::write(cache.join("cache.txt"), b"cached facts").unwrap();
    std::fs::write(root.join("empty.txt"), b"").unwrap();
    std::fs::write(root.join("marked/.nomedia"), b"").unwrap();
    let config = directory.path().join("config.toml");
    std::fs::write(
        &config,
        format!("[media_scan]\nexcluded_dirs = {}\n", excluded.display()),
    )
    .unwrap();
    let core = crate::assembly::server::ContentServer::open_with_cache(&data, &cache).unwrap();
    std::fs::write(data.join("internal.txt"), b"internal").unwrap();
    let catalog = core.native_media_catalog().unwrap();
    assert_eq!(
        core.runtime_state()
            .media_scans
            .policy()
            .unwrap()
            .cache_directory,
        cache.canonicalize().unwrap()
    );
    catalog.configure_scan_directories(Some(&config)).unwrap();
    let host = core.runtime_state().host.clone();
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&host);
    let (done, mut finish) = tokio::sync::oneshot::channel::<()>();
    let authorize = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut finish => break,
                request=requests.recv() => {
                    let request=request.unwrap();
                    assert_eq!(request["method"],"systemPermissionFacts");
                    assert_eq!(request["params"]["permissions"],serde_json::json!(["WRITE_EXTERNAL_STORAGE"]));
                    host.reply(generation,serde_json::json!({"id":request["id"],"result":{"granted":{"WRITE_EXTERNAL_STORAGE":true}}})).unwrap();
                },
            }
        }
    });
    let result = catalog
        .scan_directory(root.to_str().unwrap())
        .await
        .unwrap();
    done.send(()).unwrap();
    authorize.await.unwrap();
    assert_eq!(result.status, Status::Completed);
    assert_eq!(result.total_files, Some(7));
    assert_eq!(result.processed_files, 7);
    assert_eq!(catalog.count(plain_rs::enums::DataType::Doc).unwrap(), 3);
    assert_eq!(catalog.count(plain_rs::enums::DataType::Image).unwrap(), 1);
    assert!(
        catalog
            .by_path(
                plain_rs::enums::DataType::Doc,
                cache.join("cache.txt").to_str().unwrap()
            )
            .unwrap()
            .is_none()
    );
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn directory_scan_cancels_during_the_first_read_without_catalog_writes() {
    use crate::services::files::media_scans::Status;
    let directory = directory_scan_fixture();
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let mut catalog = core.native_media_catalog().unwrap();
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
        None
    );
    assert!(catalog.configure_scan_directories(None).is_err());
    catalog.cancel_file_scan().unwrap();
    let result = worker.await.unwrap().unwrap();
    assert_eq!(result.status, Status::Cancelled);
    assert_eq!(result.processed_files, 0);
    assert_eq!(catalog.count(plain_rs::enums::DataType::Doc).unwrap(), 0);
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn directory_scan_cancellation_releases_a_bounded_producer_without_committing_pending_files()
{
    use crate::services::files::media_scans::Status;
    let directory = directory_scan_fixture();
    let root = directory.path().join("files");
    std::fs::create_dir(&root).unwrap();
    for index in 0..96 {
        std::fs::write(root.join(format!("{index}.txt")), b"data").unwrap();
    }
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let mut catalog = core.native_media_catalog().unwrap();
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
    assert_eq!(result.processed_files, 0);
    assert_eq!(result.total_files, None);
    assert!(requests.try_recv().is_err());
    assert_eq!(catalog.count(plain_rs::enums::DataType::Doc).unwrap(), 0);
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn completion_scan_uses_the_object_protocol_and_os_reads_without_host_reentry() {
    let directory = directory_scan_fixture();
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let path = directory.path().join("completed.txt");
    std::fs::write(&path, b"completed file").unwrap();
    assert!(!core.runtime_state().host.connected());
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[path]}))
            .unwrap()
    );
    assert_eq!(catalog.count(plain_rs::enums::DataType::Doc).unwrap(), 1);
    assert!(
        catalog
            .scan_completed_files(serde_json::json!([path]))
            .is_err()
    );
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[path],"extra":true}))
            .is_err()
    );
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[]}))
            .unwrap()
    );
    let opaque = directory.path().join("file.bin");
    std::fs::write(&opaque, b"opaque").unwrap();
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[opaque]}))
            .unwrap()
    );
    assert_eq!(catalog.count(plain_rs::enums::DataType::Doc).unwrap(), 1);
    core.stop_core().await;
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[]}))
            .is_err()
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn completion_scan_reports_read_failures_instead_of_partial_success() {
    let directory = directory_scan_fixture();
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let path = directory.path().join("completed.txt");
    std::fs::write(&path, b"completed file").unwrap();
    let missing = directory.path().join("missing.txt");
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[path,missing]}))
            .is_err()
    );
    assert_eq!(catalog.count(plain_rs::enums::DataType::Doc).unwrap(), 0);
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":["relative.txt"]}))
            .is_err()
    );
    core.stop_core().await;
}

#[tokio::test]
async fn source_directories_share_core_preferences_and_reject_stopped_handles() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    let clone = catalog.clone();
    let sources = vec!["/media/a".to_string(), "/media/a".to_string()];
    catalog.set_source_directories(&sources).unwrap();
    assert_eq!(clone.source_directories().unwrap(), sources);
    assert_eq!(
        plain_rs::prefs::media::source_directories(&core.runtime_state().prefs).unwrap(),
        sources
    );
    core.stop_core().await;
    assert!(catalog.source_directories().is_err());
    assert!(catalog.set_source_directories(&[]).is_err());
}

#[cfg(feature = "media")]
#[tokio::test]
async fn source_directory_scan_keeps_one_session_across_roots_and_partial_failure() {
    use crate::services::files::media_scans::Status;
    let directory = directory_scan_fixture();
    let roots = [
        directory.path().join("first"),
        directory.path().join("second"),
    ];
    for root in &roots {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(root.join("document.txt"), b"document").unwrap();
    }
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let mut catalog = core.native_media_catalog().unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let sources = roots
        .iter()
        .map(|root| root.to_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    catalog.set_source_directories(&sources).unwrap();
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
        assert_eq!(snapshot.total_files, None);
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
    assert_eq!(catalog.count(plain_rs::enums::DataType::Doc).unwrap(), 2);

    catalog.read_gate = None;
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

#[cfg(feature = "media")]
#[tokio::test]
async fn background_source_scan_is_busy_before_the_first_read_and_core_stop_joins_it() {
    let directory = directory_scan_fixture();
    let root = directory.path().join("library");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("document.txt"), b"document").unwrap();
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let mut catalog = core.native_media_catalog().unwrap();
    catalog.configure_scan_directories(None).unwrap();
    catalog
        .set_source_directories(&[root.to_str().unwrap().to_owned()])
        .unwrap();
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
        crate::services::files::media_scans::Status::Cancelled
    );
    assert!(requests.try_recv().is_err());
    assert_eq!(
        crate::services::files::media_catalog::count(
            &core.runtime_state().db,
            plain_rs::enums::DataType::Doc,
            ""
        )
        .unwrap(),
        0
    );
}

/// The watcher writes into the catalog, so it must not outlive the core
/// that owns it: after `stop_core` no filesystem event can reach the store.
#[cfg(feature = "media")]
#[tokio::test]
async fn the_watcher_stops_with_the_core_and_rejects_after_it() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open(&directory.path()).unwrap();
    let source = directory.path().join("media");
    std::fs::create_dir_all(&source).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    catalog
        .set_source_directories(&[source.to_string_lossy().into_owned()])
        .unwrap();
    assert!(catalog.watch_source_directories().unwrap());
    catalog.configure_scan_directories(None).unwrap();
    assert!(catalog.watch_source_directories().unwrap());
    assert!(
        core.runtime_state().media_watch.lock().unwrap().is_some(),
        "a configured source directory must be watched"
    );
    // Re-watching replaces rather than stacking a second one.
    assert!(catalog.watch_source_directories().unwrap());

    core.stop_core().await;
    assert!(
        core.runtime_state().media_watch.lock().unwrap().is_none(),
        "the watcher has to be released with the core"
    );
    assert!(catalog.watch_source_directories().is_err());
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[]}))
            .is_err(),
        "a stopped core must refuse catalog writes"
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn filesystem_watch_events_from_an_os_thread_update_the_shared_catalog() {
    let directory = tempfile::tempdir().unwrap();
    let source = tempfile::Builder::new()
        .prefix("watch-source-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    catalog.configure_scan_directories(None).unwrap();
    catalog
        .set_source_directories(&[source.path().to_string_lossy().into_owned()])
        .unwrap();
    catalog.watch_source_directories().unwrap();
    let path = source.path().join("photo.png");
    image::RgbImage::new(1, 1).save(&path).unwrap();
    let path = path.to_string_lossy().into_owned();
    let observed = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if catalog
                .by_path(plain_rs::enums::DataType::Image, &path)
                .unwrap()
                .is_some()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await;
    core.stop_core().await;
    assert!(
        observed.is_ok(),
        "OS watcher events must reach the owning Tokio runtime"
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn native_catalog_records_and_removal_publish_one_typed_change_per_batch() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("song.mp3");
    std::fs::write(&path, b"unrecognized audio bytes").unwrap();
    let path = path.canonicalize().unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    let mut events = core.runtime_state().events.subscribe();
    let file = plain_media::metadata::file_facts::read_file_metadata(
        &path,
        plain_rs::enums::DataType::Audio,
    )
    .unwrap();
    catalog.record_files(&[file.clone(), file]).unwrap();
    let event = events.try_recv().unwrap();
    assert_eq!(event.event_type, plain_rs::events::MEDIA_LIBRARY_UPDATED);
    assert_eq!(event.payload, r#"{"types":[1]}"#);
    assert!(events.try_recv().is_err());
    assert_eq!(catalog.forget_path(path.to_str().unwrap()).unwrap(), 1);
    assert_eq!(
        events.try_recv().unwrap().event_type,
        plain_rs::events::MEDIA_LIBRARY_UPDATED
    );
    assert!(events.try_recv().is_err());
    assert_eq!(catalog.forget_path(path.to_str().unwrap()).unwrap(), 0);
    assert!(events.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn nomedia_reclassification_preserves_file_identity_and_user_state() {
    use plain_rs::{db::FileKind, enums::DataType};
    let directory = directory_scan_fixture();
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let path = directory.path().join("song.wav");
    std::fs::write(&path, b"unrecognized audio bytes").unwrap();
    let scan = || {
        catalog
            .scan_completed_files(serde_json::json!({"paths":[path]}))
            .unwrap()
    };
    scan();
    let original = catalog
        .by_path(DataType::Audio, path.to_str().unwrap())
        .unwrap()
        .unwrap();
    let id: i64 = original.id.parse().unwrap();
    let db = core.runtime_state().db.clone();
    db.with_conn(|conn| -> rusqlite::Result<()> {
        conn.execute("UPDATE files SET favorited=1 WHERE id=?1", [id])?;
        conn.execute("INSERT INTO tags(id,name,type,created_at,updated_at) VALUES('nomedia-tag','Tag',1,'2026-01-01','2026-01-01')", [])?;
        conn.execute("INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('nomedia-tag',?1,?2,'2026-01-01')", rusqlite::params![original.id, DataType::Audio.kind()])?;
        Ok(())
    }).unwrap();
    let marker = directory.path().join(".nomedia");
    std::fs::write(&marker, b"").unwrap();
    let mut events = core.runtime_state().events.subscribe();
    scan();
    let event = events.try_recv().unwrap();
    assert_eq!(event.event_type, plain_rs::events::MEDIA_LIBRARY_UPDATED);
    assert!(crate::services::files::catalog_events::includes(
        &event.payload,
        DataType::Audio
    ));
    assert!(crate::services::files::catalog_events::includes(
        &event.payload,
        DataType::File
    ));
    assert_eq!(catalog.count(DataType::Audio).unwrap(), 0);
    let file = db.file(id).unwrap().unwrap();
    assert_eq!(file.kind, FileKind::File);
    assert!(file.favorited);
    std::fs::remove_file(marker).unwrap();
    scan();
    assert_eq!(catalog.count(DataType::Audio).unwrap(), 1);
    assert_eq!(
        catalog
            .by_path(DataType::Audio, path.to_str().unwrap())
            .unwrap()
            .unwrap()
            .id,
        original.id
    );
    assert!(db.file(id).unwrap().unwrap().favorited);
    assert_eq!(
        db.with_conn(|conn| conn.query_row(
            "SELECT count(*) FROM tag_relations WHERE tag_id='nomedia-tag' AND target_key=?1",
            [&original.id],
            |row| row.get::<_, i64>(0)
        ))
        .unwrap(),
        1
    );
    core.stop_core().await;
}
