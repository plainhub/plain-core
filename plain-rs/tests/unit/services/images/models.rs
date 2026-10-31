use super::*;
use plain_rs::db::Db;

async fn settled(runtime: &Runtime) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let loading = runtime
                .task
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|task| !task.is_finished());
            if !loading && !runtime.index.status().is_running {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn failed_replacement_preserves_active_package_index_and_search() {
    let directory = tempfile::tempdir().unwrap();
    let incoming = directory.path().join("ai_models/incoming");
    let original = plain_rs::inference::models::test_fixture::fixture(&incoming);
    let photo = directory.path().join("photo.png");
    image::RgbImage::from_pixel(64, 64, image::Rgb([128, 64, 32]))
        .save(&photo)
        .unwrap();
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let peer = host.clone();
    let task = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemImageModelsObserve" | "imageCatalogVerify" => json!(true),
                "imageCatalogSnapshot" => json!({"revision":"one","total":1}),
                "imageCatalogPage" => {
                    json!({"revision":"one","items":[{"id":"one","path":photo}],"nextCursor":"one","done":true})
                }
                method => panic!("Unexpected {method}"),
            };
            peer.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let db = Arc::new(Db::open(&directory.path().join("data.db")).unwrap());
    let (events, _) = tokio::sync::broadcast::channel(16);
    let index = Arc::new(ImageIndex::new(
        db.clone(),
        host.clone(),
        events.clone(),
        Arc::new(plain_rs::host::resources::NativeResources::new(
            host.clone(),
        )),
        directory.path().join("cache"),
    ));
    index.use_system_catalog_for_test();
    let runtime = Runtime::new(
        directory.path().into(),
        host,
        prefs,
        Arc::new(plain_rs::kv::LazyDb::new(directory.path().join("plain.db"))),
        index,
        events,
    );
    runtime.import().await.unwrap();
    settled(&runtime).await;
    assert_eq!(
        runtime.snapshot().status.status,
        ImageSearchStatusType::Ready
    );
    assert_eq!(plain_rs::media::images::embeddings::count(&db).unwrap(), 1);
    assert_eq!(runtime.search("cat", 50).await.unwrap()[0].image_id, "one");
    let mut replacement = plain_rs::inference::models::test_fixture::fixture(&incoming);
    replacement.files[0].sha256 = "0".repeat(64);
    std::fs::write(
        incoming.join("manifest.json"),
        serde_json::to_vec(&replacement).unwrap(),
    )
    .unwrap();
    runtime.import().await.unwrap();
    settled(&runtime).await;
    assert_eq!(
        runtime.snapshot().status.status,
        ImageSearchStatusType::Ready
    );
    assert!(runtime.snapshot().status.error_message.contains("checksum"));
    assert_eq!(plain_rs::media::images::embeddings::count(&db).unwrap(), 1);
    let active = plain_rs::inference::models::manifest::Manifest::parse(
        &std::fs::read(directory.path().join("ai_models/imported/manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        active.fingerprint().unwrap(),
        original.fingerprint().unwrap()
    );
    assert_eq!(runtime.search("cat", 50).await.unwrap()[0].score, 1.0);
    runtime.shutdown().await;
    runtime.enable(true).await.unwrap();
    settled(&runtime).await;
    assert_eq!(
        runtime.snapshot().status.status,
        ImageSearchStatusType::Ready
    );
    assert!(runtime.snapshot().status.error_message.is_empty());
    assert_eq!(runtime.search("cat", 50).await.unwrap()[0].score, 1.0);
    replacement = plain_rs::inference::models::test_fixture::fixture(&incoming);
    replacement.id = "different-space".into();
    std::fs::write(
        incoming.join("manifest.json"),
        serde_json::to_vec(&replacement).unwrap(),
    )
    .unwrap();
    runtime.import().await.unwrap();
    settled(&runtime).await;
    assert_eq!(
        runtime.snapshot().status.status,
        ImageSearchStatusType::Ready
    );
    assert_eq!(plain_rs::media::images::embeddings::count(&db).unwrap(), 1);
    runtime.cancel(true).await.unwrap();
    assert_eq!(plain_rs::media::images::embeddings::count(&db).unwrap(), 0);
    assert!(!directory.path().join("ai_models").exists());
    task.abort();
}

#[tokio::test]
async fn catalog_changes_share_one_cancellable_timer_and_do_not_retain_the_runtime() {
    let directory = tempfile::tempdir().unwrap();
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let (events, _) = tokio::sync::broadcast::channel(16);
    let index = Arc::new(ImageIndex::new(
        db,
        host.clone(),
        events.clone(),
        Arc::new(plain_rs::host::resources::NativeResources::new(
            host.clone(),
        )),
        directory.path().join("cache"),
    ));
    index.use_system_catalog_for_test();
    let runtime = Runtime::new(
        directory.path().into(),
        host.clone(),
        prefs,
        Arc::new(plain_rs::kv::LazyDb::new(directory.path().join("plain.db"))),
        index,
        events,
    );
    runtime.catalog_changed();
    assert!(runtime.catalog_task.lock().unwrap().is_none());
    runtime.observing.store(true, Ordering::Release);
    runtime.catalog_changed();
    let old = runtime
        .catalog_task
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .abort_handle();
    runtime.catalog_changed();
    let current = runtime
        .catalog_task
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .abort_handle();
    assert_ne!(old.id(), current.id());
    assert_eq!(Arc::strong_count(&runtime), 1);
    runtime.cancel_catalog_changes();
    assert!(runtime.catalog_task.lock().unwrap().is_none());
    tokio::time::timeout(Duration::from_secs(5), async {
        while !old.is_finished() || !current.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!runtime.index.status().is_running);
    assert!(requests.try_recv().is_err());
    host.disconnect(generation);
}

#[tokio::test]
async fn core_start_restores_enabled_local_model_without_a_client_mutation_or_download() {
    let directory = tempfile::tempdir().unwrap();
    plain_rs::inference::models::test_fixture::fixture(
        &directory.path().join("ai_models/imported"),
    );
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let responder = host.clone();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let task = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            observed.fetch_add(1, Ordering::SeqCst);
            let result = match request["method"].as_str().unwrap() {
                "systemImageModelsObserve" | "imageCatalogVerify" => json!(true),
                "imageCatalogSnapshot" => json!({"revision":"empty","total":0}),
                "imageCatalogPage" => {
                    json!({"revision":"empty","items":[],"nextCursor":"","done":true})
                }
                method => panic!("Unexpected startup request: {method}"),
            };
            responder
                .reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    prefs.set("ai_image_search_enabled", true).unwrap();
    prefs.set("ai_image_search_model", "imported").unwrap();
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let (events, mut updates) = tokio::sync::broadcast::channel(32);
    let index = Arc::new(ImageIndex::new(
        db,
        host.clone(),
        events.clone(),
        Arc::new(plain_rs::host::resources::NativeResources::new(
            host.clone(),
        )),
        directory.path().join("cache"),
    ));
    index.use_system_catalog_for_test();
    let runtime = Runtime::new(
        directory.path().into(),
        host,
        prefs,
        Arc::new(plain_rs::kv::LazyDb::new(directory.path().join("plain.db"))),
        index,
        events,
    );
    let (stop, stopped) = tokio::sync::watch::channel(false);
    runtime.follow(stopped);
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = updates.recv().await.unwrap();
            if event.event_type == "IMAGE_MODELS_UPDATED" {
                let value: serde_json::Value = serde_json::from_str(&event.payload).unwrap();
                if value["status"] == "READY" {
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
    settled(&runtime).await;
    assert!(runtime.engine.lock().unwrap().is_some());
    assert!(runtime.observing.load(Ordering::Acquire));
    assert!(calls.load(Ordering::SeqCst) > 0);
    assert!(!directory.path().join("ai_models/download").exists());
    stop.send(true).unwrap();
    runtime.shutdown().await;
    task.abort();
}

#[tokio::test]
async fn native_model_restore_and_shutdown_do_not_request_sdk_observers_or_catalogs() {
    let directory = tempfile::tempdir().unwrap();
    plain_rs::inference::models::test_fixture::fixture(
        &directory.path().join("ai_models/imported"),
    );
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    prefs.set("ai_image_search_enabled", true).unwrap();
    prefs.set("ai_image_search_model", "imported").unwrap();
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let (events, _) = tokio::sync::broadcast::channel(32);
    let index = Arc::new(ImageIndex::new(
        db,
        host.clone(),
        events.clone(),
        Arc::new(plain_rs::host::resources::NativeResources::new(
            host.clone(),
        )),
        directory.path().join("cache"),
    ));
    index.use_native_catalog_for_test();
    let runtime = Runtime::new(
        directory.path().into(),
        host,
        prefs,
        Arc::new(plain_rs::kv::LazyDb::new(directory.path().join("plain.db"))),
        index,
        events,
    );
    runtime.enable(true).await.unwrap();
    settled(&runtime).await;
    assert_eq!(
        runtime.snapshot().status.status,
        ImageSearchStatusType::Ready
    );
    assert!(runtime.snapshot().status.error_message.is_empty());
    assert!(requests.try_recv().is_err());
    runtime.shutdown().await;
    assert!(requests.try_recv().is_err());
}

fn projection_runtime() -> (
    tempfile::TempDir,
    Arc<Runtime>,
    tokio::sync::broadcast::Receiver<WsEvent>,
) {
    let directory = tempfile::tempdir().unwrap();
    let host = Arc::new(Host::default());
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let (events, updates) = tokio::sync::broadcast::channel(32);
    let index = Arc::new(ImageIndex::new(
        db,
        host.clone(),
        events.clone(),
        Arc::new(plain_rs::host::resources::NativeResources::new(
            host.clone(),
        )),
        directory.path().join("cache"),
    ));
    index.use_native_catalog_for_test();
    let runtime = Runtime::new(
        directory.path().into(),
        host,
        prefs,
        Arc::new(plain_rs::kv::LazyDb::new(directory.path().join("plain.db"))),
        index,
        events,
    );
    (directory, runtime, updates)
}

#[tokio::test]
async fn repeated_model_projections_emit_once_per_changed_snapshot() {
    let (_directory, runtime, mut updates) = projection_runtime();
    runtime.publish();
    runtime.publish();
    let first = updates.try_recv().unwrap();
    assert_eq!(first.event_type, "IMAGE_MODELS_UPDATED");
    let first: serde_json::Value = serde_json::from_str(&first.payload).unwrap();
    assert!(updates.try_recv().is_err());
    runtime.report_error("Index failed".into());
    runtime.report_error("Index failed".into());
    let changed = updates.try_recv().unwrap();
    let changed: serde_json::Value = serde_json::from_str(&changed.payload).unwrap();
    assert!(changed["version"].as_u64().unwrap() > first["version"].as_u64().unwrap());
    assert_eq!(changed["errorMessage"], "Index failed");
    assert!(updates.try_recv().is_err());
    runtime.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn idle_model_runtime_does_not_poll_snapshots_and_index_events_project_without_a_timer() {
    let (_directory, runtime, mut updates) = projection_runtime();
    let (stop, stopped) = tokio::sync::watch::channel(false);
    runtime.follow(stopped);
    assert_eq!(
        updates.recv().await.unwrap().event_type,
        "IMAGE_MODELS_UPDATED"
    );
    tokio::task::yield_now().await;
    runtime.snapshot_reads.store(0, Ordering::Relaxed);
    tokio::time::advance(Duration::from_secs(30)).await;
    tokio::task::yield_now().await;
    assert_eq!(runtime.snapshot_reads.load(Ordering::Relaxed), 0);
    assert!(updates.try_recv().is_err());
    tokio::time::resume();
    runtime.index.use_system_catalog_for_test();
    let host = runtime.host.clone();
    let (generation, mut requests) = host.connect();
    runtime.index.start(false).unwrap();
    loop {
        let event = updates.recv().await.unwrap();
        if event.event_type == "IMAGE_MODELS_UPDATED" {
            let snapshot: serde_json::Value = serde_json::from_str(&event.payload).unwrap();
            if snapshot["isIndexing"] == true {
                break;
            }
        }
    }
    assert!(runtime.snapshot_reads.load(Ordering::Relaxed) > 0);
    let pending = requests.recv().await.unwrap();
    assert_eq!(pending["method"], "imageCatalogSnapshot");
    runtime.index.cancel().unwrap();
    host.reply(
        generation,
        json!({"id":pending["id"],"result":{"revision":"empty","total":0}}),
    )
    .unwrap();
    runtime.index.wait_until_idle().await;
    stop.send(true).unwrap();
    runtime.shutdown().await;
}

#[tokio::test]
async fn initial_snapshot_is_published_before_the_core_is_returned_to_consumers() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let initial = state.image_models.snapshot();
    assert_eq!(
        state.image_models.state.lock().unwrap().published_version,
        Some(initial.version)
    );
    let mut events = state.events.subscribe();
    tokio::task::yield_now().await;
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(event.event_type, "IMAGE_MODELS_UPDATED"));
    }
    core.stop_core().await;
}
