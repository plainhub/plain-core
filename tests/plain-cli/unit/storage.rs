use super::*;

#[test]
fn explicit_directories_are_added_to_platform_defaults_and_persisted() {
    let prefs = plain_rs::features::prefs::Prefs::in_memory();
    let directory = tempfile::tempdir().unwrap();
    let roots = source_directories(&prefs, &[directory.path().to_owned()]).unwrap();
    assert!(
        roots.contains(
            &directory
                .path()
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        )
    );
    for root in plain_rs::features::prefs::file_index::defaults().scan_directories {
        assert!(roots.contains(&root));
    }
    assert_eq!(source_directories(&prefs, &[]).unwrap(), roots);
}

#[test]
fn relative_scan_roots_are_rejected_before_saving_preferences() {
    let prefs = plain_rs::features::prefs::Prefs::in_memory();
    assert!(source_directories(&prefs, &[PathBuf::from("relative")]).is_err());
    assert!(
        prefs
            .get::<Vec<String>>(plain_rs::features::prefs::pref_keys::MEDIA_SOURCE_DIRS)
            .unwrap()
            .is_none()
    );
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
        .set_defaults(plain_rs::features::prefs::file_index::Defaults {
            scan_directories: Vec::new(),
            excluded_directories: Vec::new(),
            excluded_names: Vec::new(),
        })
        .unwrap();
    let catalog = core.native_media_catalog().unwrap();
    catalog
        .set_source_directories(&[source.to_string_lossy().into_owned()])
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
