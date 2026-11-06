use super::*;

#[test]
fn explicit_directories_are_persisted_on_the_containing_volume() {
    let directory = tempfile::tempdir().unwrap();
    let db = plain_rs::infrastructure::db::Db::open(&directory.path().join("index.db")).unwrap();
    configure_index(&db, &[directory.path().to_owned()]).unwrap();
    let volumes = db.volumes().unwrap();
    assert_eq!(volumes.len(), 1);
    assert_eq!(
        volumes[0].index_mode,
        plain_rs::infrastructure::db::IndexMode::Allowlist
    );
    let roots = plain_rs::features::files::index_scope::source_directories(&volumes);
    assert_eq!(
        roots,
        vec![plain_rs::features::files::index_scope::normalized(
            &directory.path().canonicalize().unwrap()
        )]
    );
    configure_index(&db, &[]).unwrap();
    assert_eq!(db.volumes().unwrap(), volumes);
}

#[test]
fn relative_scan_roots_are_rejected_before_registering_volumes() {
    let directory = tempfile::tempdir().unwrap();
    let db = plain_rs::infrastructure::db::Db::open(&directory.path().join("index.db")).unwrap();
    assert!(configure_index(&db, &[PathBuf::from("relative")]).is_err());
    assert!(db.volumes().unwrap().is_empty());
}

#[tokio::test]
async fn startup_scan_indexes_configured_directories_through_the_headless_host() {
    use plain_rs::features::services::files::media_scans::Status;
    use std::sync::Arc;
    let home = plain_rs::infrastructure::platform::system::consts::home_directory().unwrap();
    let directory = tempfile::Builder::new()
        .prefix("scan-check-")
        .tempdir_in(home)
        .unwrap();
    let source = directory.path().join("documents");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("document.txt"), b"document").unwrap();
    let core = Arc::new(
        plain_server::server::ContentServer::open(&directory.path().join("core")).unwrap(),
    );
    core.runtime()
        .media_scans
        .set_defaults(plain_rs::features::files::index_scope::Defaults {
            excluded_directories: Vec::new(),
            excluded_names: Vec::new(),
        })
        .unwrap();
    let catalog = core.native_media_catalog().unwrap();
    plain_rs::features::services::files::indexing::set_directories(
        &core.runtime().db,
        &[source.to_string_lossy().into_owned()],
    )
    .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let runtime = Arc::new(crate::host::Host::new(core.clone(), vec![]));
    let runner = tokio::spawn(runtime.run());
    let started = catalog.start_source_scan("/").unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let snapshot = catalog.file_scan_status().unwrap().unwrap();
            if matches!(
                snapshot.status,
                Status::Completed | Status::Failed | Status::Cancelled
            ) {
                break snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    let count = catalog
        .count(plain_rs::features::enums::DataType::Doc)
        .unwrap();
    core.stop_core().await;
    runner.await.unwrap().unwrap();
    let completed = result.unwrap();
    assert_eq!(completed.id, started.id);
    assert_eq!(completed.status, Status::Completed, "{:?}", completed.error);
    assert_eq!(completed.processed_files, 1);
    assert_eq!(count, 1);
}
