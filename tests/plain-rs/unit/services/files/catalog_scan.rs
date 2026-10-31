use super::*;

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn file_scan_cancel_aborts_pending_reads_and_keeps_catalog_unchanged() {
    use plain_rs::services::files::media_scans::Status;
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

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn aborted_scan_future_releases_the_shared_job_and_failed_scans_report_partial_progress() {
    use plain_rs::services::files::media_scans::Status;
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

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn directory_scan_cancels_during_the_first_read_without_catalog_writes() {
    use plain_rs::services::files::media_scans::Status;
    let directory = directory_scan_fixture();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let mut catalog = NativeMediaCatalog::new(core.runtime_state());
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

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn directory_scan_cancellation_releases_a_bounded_producer_without_committing_pending_files()
{
    use plain_rs::services::files::media_scans::Status;
    let directory = directory_scan_fixture();
    let root = directory.path().join("files");
    std::fs::create_dir(&root).unwrap();
    for index in 0..96 {
        std::fs::write(root.join(format!("{index}.txt")), b"data").unwrap();
    }
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let mut catalog = NativeMediaCatalog::new(core.runtime_state());
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

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn source_directory_scan_keeps_one_session_across_roots_and_partial_failure() {
    use plain_rs::services::files::media_scans::Status;
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

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn background_source_scan_is_busy_before_the_first_read_and_core_stop_joins_it() {
    let directory = directory_scan_fixture();
    let root = directory.path().join("library");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("document.txt"), b"document").unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let mut catalog = NativeMediaCatalog::new(core.runtime_state());
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
        plain_rs::services::files::media_scans::Status::Cancelled
    );
    assert!(requests.try_recv().is_err());
    assert_eq!(
        plain_rs::services::files::media_catalog::count(
            &core.runtime_state().db,
            plain_rs::enums::DataType::Doc,
            ""
        )
        .unwrap(),
        0
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
    let file =
        plain_rs::media::file_facts::read_file_metadata(&path, plain_rs::enums::DataType::Audio)
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

fn directory_scan_fixture() -> tempfile::TempDir {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    tempfile::Builder::new()
        .prefix("media-scan-")
        .tempdir_in(home)
        .unwrap()
}
