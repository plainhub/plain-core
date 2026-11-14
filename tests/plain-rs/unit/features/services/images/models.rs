use super::*;
use crate::infrastructure::db::Db;

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
    let directory = plain_libs::temporary::tempdir().unwrap();
    let incoming = directory.path().join("ai_models/incoming");
    let original = plain_libs::ai_image_search::models::test_fixture::fixture(&incoming);
    let photo = directory.path().join("photo.png");
    plain_libs::image::RgbImage::from_pixel(64, 64, plain_libs::image::Rgb([128, 64, 32]))
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
        Arc::new(crate::infrastructure::host::resources::NativeResources::new(host.clone())),
        directory.path().join("cache"),
    ));
    index.use_system_catalog_for_test();
    let runtime = Runtime::new(
        directory.path().into(),
        host,
        prefs,
        Arc::new(plain_libs::kv::LazyDb::new(
            directory.path().join("plain.db"),
        )),
        index,
        events,
    );
    runtime.import().await.unwrap();
    settled(&runtime).await;
    assert_eq!(
        runtime.snapshot().status.status,
        ImageSearchStatusType::Ready
    );
    assert_eq!(
        crate::features::media::images::embeddings::count(&db).unwrap(),
        1
    );
    assert_eq!(runtime.search("cat").await.unwrap()[0].image_id, "one");
    let mut replacement = plain_libs::ai_image_search::models::test_fixture::fixture(&incoming);
    replacement.files[0].sha256 = "0".repeat(64);
    std::fs::write(
        incoming.join("manifest.json"),
        plain_libs::serde_json::to_vec(&replacement).unwrap(),
    )
    .unwrap();
    runtime.import().await.unwrap();
    settled(&runtime).await;
    assert_eq!(
        runtime.snapshot().status.status,
        ImageSearchStatusType::Ready
    );
    assert!(runtime.snapshot().status.error_message.contains("checksum"));
    assert_eq!(
        crate::features::media::images::embeddings::count(&db).unwrap(),
        1
    );
    let active = plain_libs::ai_image_search::models::manifest::Manifest::parse(
        &std::fs::read(directory.path().join("ai_models/imported/manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        active.fingerprint().unwrap(),
        original.fingerprint().unwrap()
    );
    assert_eq!(runtime.search("cat").await.unwrap()[0].score, 1.0);
    runtime.shutdown().await;
    runtime.enable(true).await.unwrap();
    settled(&runtime).await;
    assert_eq!(
        runtime.snapshot().status.status,
        ImageSearchStatusType::Ready
    );
    assert!(runtime.snapshot().status.error_message.is_empty());
    assert_eq!(runtime.search("cat").await.unwrap()[0].score, 1.0);
    replacement = plain_libs::ai_image_search::models::test_fixture::fixture(&incoming);
    replacement.id = "different-space".into();
    std::fs::write(
        incoming.join("manifest.json"),
        plain_libs::serde_json::to_vec(&replacement).unwrap(),
    )
    .unwrap();
    runtime.import().await.unwrap();
    settled(&runtime).await;
    assert_eq!(
        runtime.snapshot().status.status,
        ImageSearchStatusType::Ready
    );
    assert_eq!(
        crate::features::media::images::embeddings::count(&db).unwrap(),
        1
    );
    runtime.cancel(true).await.unwrap();
    assert_eq!(
        crate::features::media::images::embeddings::count(&db).unwrap(),
        0
    );
    assert!(!directory.path().join("ai_models").exists());
    task.abort();
}

#[tokio::test]
async fn catalog_changes_share_one_cancellable_timer_and_do_not_retain_the_runtime() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let (events, _) = tokio::sync::broadcast::channel(16);
    let index = Arc::new(ImageIndex::new(
        db,
        host.clone(),
        events.clone(),
        Arc::new(crate::infrastructure::host::resources::NativeResources::new(host.clone())),
        directory.path().join("cache"),
    ));
    index.use_system_catalog_for_test();
    let runtime = Runtime::new(
        directory.path().into(),
        host.clone(),
        prefs,
        Arc::new(plain_libs::kv::LazyDb::new(
            directory.path().join("plain.db"),
        )),
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
    let directory = plain_libs::temporary::tempdir().unwrap();
    plain_libs::ai_image_search::models::test_fixture::fixture(
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
        Arc::new(crate::infrastructure::host::resources::NativeResources::new(host.clone())),
        directory.path().join("cache"),
    ));
    index.use_system_catalog_for_test();
    let runtime = Runtime::new(
        directory.path().into(),
        host,
        prefs,
        Arc::new(plain_libs::kv::LazyDb::new(
            directory.path().join("plain.db"),
        )),
        index,
        events,
    );
    let (stop, stopped) = tokio::sync::watch::channel(false);
    runtime.follow(stopped);
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = updates.recv().await.unwrap();
            if event.event_type == "IMAGE_MODELS_UPDATED" {
                let value: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&event.payload).unwrap();
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
    let directory = plain_libs::temporary::tempdir().unwrap();
    plain_libs::ai_image_search::models::test_fixture::fixture(
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
        Arc::new(crate::infrastructure::host::resources::NativeResources::new(host.clone())),
        directory.path().join("cache"),
    ));
    index.use_native_catalog_for_test();
    let runtime = Runtime::new(
        directory.path().into(),
        host,
        prefs,
        Arc::new(plain_libs::kv::LazyDb::new(
            directory.path().join("plain.db"),
        )),
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
    plain_libs::temporary::TempDir,
    Arc<Runtime>,
    tokio::sync::broadcast::Receiver<WsEvent>,
) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let host = Arc::new(Host::default());
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let (events, updates) = tokio::sync::broadcast::channel(32);
    let index = Arc::new(ImageIndex::new(
        db,
        host.clone(),
        events.clone(),
        Arc::new(crate::infrastructure::host::resources::NativeResources::new(host.clone())),
        directory.path().join("cache"),
    ));
    index.use_native_catalog_for_test();
    let runtime = Runtime::new(
        directory.path().into(),
        host,
        prefs,
        Arc::new(plain_libs::kv::LazyDb::new(
            directory.path().join("plain.db"),
        )),
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
    let first: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&first.payload).unwrap();
    assert!(updates.try_recv().is_err());
    runtime.report_error("Index failed".into());
    runtime.report_error("Index failed".into());
    let changed = updates.try_recv().unwrap();
    let changed: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&changed.payload).unwrap();
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
            let snapshot: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&event.payload).unwrap();
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
    let directory = plain_libs::temporary::tempdir().unwrap();
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

#[tokio::test]
async fn semantic_search_pages_all_matches_and_filters_before_pagination() {
    use crate::features::enums::DataType;
    use crate::features::media::images::embeddings::{self, EmbeddingInput};
    use crate::features::services::files::catalog::MediaCatalog;
    use crate::features::services::images::search;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let model_dir = directory.path().join("model");
    let mut manifest = plain_libs::ai_image_search::models::test_fixture::fixture(&model_dir);
    manifest.minimum_score = 0.15;
    let engine = Arc::new(
        plain_libs::ai_image_search::embedding::Engine::open(model_dir, manifest).unwrap(),
    );
    let vector = engine.text("cat").unwrap();
    let host = Arc::new(Host::default());
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let db = Arc::new(Db::open(&directory.path().join("data.db")).unwrap());
    let (events, _) = tokio::sync::broadcast::channel(16);
    let index = Arc::new(ImageIndex::new(
        db.clone(),
        host.clone(),
        events.clone(),
        Arc::new(crate::infrastructure::host::resources::NativeResources::new(host.clone())),
        directory.path().join("cache"),
    ));
    let runtime = Runtime::new(
        directory.path().into(),
        host.clone(),
        prefs,
        Arc::new(plain_libs::kv::LazyDb::new(
            directory.path().join("plain.db"),
        )),
        index,
        events,
    );
    *runtime.engine.lock().unwrap() = Some(engine);
    runtime.state.lock().unwrap().status = ImageSearchStatusType::Ready;
    let mut ranked = Vec::new();
    let mut selected = Vec::new();
    let mut filename = Vec::new();
    for i in 0..620 {
        let folder = if i >= 550 { "selected" } else { "other" };
        let title = if i < 7 || (550..553).contains(&i) {
            format!("cat-{i:04}")
        } else {
            format!("photo-{i:04}")
        };
        let rows = db
            .record_media_files(&[crate::infrastructure::db::MediaFileMetadata {
                data_type: DataType::Image,
                path: format!("/synthetic/{folder}/{title}.png"),
                title: title.clone(),
                album: String::new(),
                album_artist: String::new(),
                artist: String::new(),
                bucket_id: folder.into(),
                size_bytes: 2048,
                duration_ms: 0,
                cover_image_id: None,
                width: 64,
                height: 64,
                rotation: 0,
                taken_at: None,
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            }])
            .unwrap();
        let row = &rows[0];
        let score = if i == 619 {
            0.1
        } else {
            1.0 - i as f32 / 1000.0
        };
        let bytes = vector
            .iter()
            .flat_map(|v| (v * score).to_be_bytes())
            .collect::<Vec<_>>();
        embeddings::save(
            &db,
            &[EmbeddingInput {
                id: row.id.clone(),
                path: row.path.clone(),
                embedding_base64: plain_libs::utils::base64::base64_encode(&bytes),
            }],
        )
        .unwrap();
        if title.starts_with("cat") {
            filename.push(row.id.clone());
        }
        if i < 619 && !title.starts_with("cat") {
            ranked.push(row.id.clone());
        }
        if i >= 550 && i < 619 {
            selected.push(row.id.clone());
        }
    }
    let orphan_bytes = vector
        .iter()
        .flat_map(|v| v.to_be_bytes())
        .collect::<Vec<_>>();
    embeddings::save(
        &db,
        &[EmbeddingInput {
            id: "orphan".into(),
            path: "/synthetic/missing.png".into(),
            embedding_base64: plain_libs::utils::base64::base64_encode(&orphan_bytes),
        }],
    )
    .unwrap();
    let catalog = MediaCatalog::Native(db.clone());
    let mut expected = filename;
    expected.extend(ranked);
    assert_eq!(
        search::count(&catalog, &runtime, "cat", "text:cat")
            .await
            .unwrap(),
        619
    );
    let mut actual = Vec::new();
    for offset in (0..650).step_by(37) {
        let page = search::rows(
            &catalog, &runtime, "cat", "text:cat", offset, 37, "NAME_ASC",
        )
        .await
        .unwrap();
        actual.extend(
            page.into_iter()
                .map(|r| r["id"].as_str().unwrap().to_owned()),
        );
    }
    assert_eq!(actual, expected);
    let query = "text:cat bucket_id:/synthetic/selected";
    assert_eq!(
        search::count(&catalog, &runtime, "cat", query)
            .await
            .unwrap(),
        69
    );
    let page = search::rows(&catalog, &runtime, "cat", query, 50, 50, "NAME_ASC")
        .await
        .unwrap();
    assert_eq!(
        page.iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        selected[50..]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    assert!(
        search::rows(&catalog, &runtime, "cat", query, 69, 10, "NAME_ASC")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        search::rows(&catalog, &runtime, "cat", query, 0, 0, "NAME_ASC")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        search::rows(&catalog, &runtime, "cat", query, -1, 10, "NAME_ASC")
            .await
            .is_err()
    );
    assert!(
        search::rows(&catalog, &runtime, "cat", query, 0, -1, "NAME_ASC")
            .await
            .is_err()
    );
    let selected_id = selected.last().unwrap();
    let id_query = format!("text:cat ids:{selected_id}");
    assert_eq!(
        search::count(&catalog, &runtime, "cat", &id_query)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        search::rows(&catalog, &runtime, "cat", &id_query, 0, 10, "NAME_ASC")
            .await
            .unwrap()[0]["id"],
        *selected_id
    );
    assert_eq!(
        search::count(&catalog, &runtime, "cat", "text:cat ids:missing")
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        search::count(&catalog, &runtime, "", "").await.unwrap(),
        620
    );
    for volume in db.volumes().unwrap() {
        db.set_volume_index(
            &volume.id,
            crate::infrastructure::db::FileIndexMode::Blocklist,
            &[],
            &[],
        )
        .unwrap();
    }
    let (generation, mut requests) = host.connect();
    let replies = host.clone();
    let provider_db = db.clone();
    let worker = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let params = &request["params"];
            let value = &params["plan"];
            let plan = crate::features::services::files::provider_query::Plan {
                clauses: plain_libs::serde_json::from_value(value["clauses"].clone()).unwrap(),
                args: plain_libs::serde_json::from_value(value["args"].clone()).unwrap(),
                ids_column: value["idsColumn"].as_str().map(str::to_owned),
                ids: value
                    .get("ids")
                    .map(|ids| plain_libs::serde_json::from_value(ids.clone()).unwrap())
                    .unwrap_or_default(),
                trash: value["trash"].as_bool(),
            };
            if plan.ids_column.is_some() {
                assert!(plan.ids.len() <= 256);
            }
            let count = request["method"] == "systemMediaCount";
            assert!(count || request["method"] == "systemMediaRows");
            let sort = if count {
                crate::features::prefs::settings::MediaSortBy::NameAsc
            } else {
                plain_libs::serde_json::from_value(params["sortBy"].clone()).unwrap()
            };
            let rows = crate::features::services::files::media_catalog::rows_with_plan(
                &provider_db,
                DataType::Image,
                plan,
                sort,
                if count {
                    u32::MAX
                } else {
                    params["limit"].as_u64().unwrap() as u32
                },
                if count {
                    0
                } else {
                    params["offset"].as_u64().unwrap() as u32
                },
            )
            .unwrap();
            let result = if count {
                json!(rows.len())
            } else {
                json!(
                    rows.iter()
                        .map(crate::features::services::files::media_catalog_facts::row_facts)
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap()
                )
            };
            replies
                .reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    let platform = MediaCatalog::Platform {
        db: db.clone(),
        host,
    };
    assert_eq!(
        search::count(&platform, &runtime, "cat", "text:cat")
            .await
            .unwrap(),
        619
    );
    assert_eq!(
        search::count(&platform, &runtime, "cat", query)
            .await
            .unwrap(),
        69
    );
    let platform_page = search::rows(&platform, &runtime, "cat", query, 50, 50, "NAME_ASC")
        .await
        .unwrap();
    assert_eq!(platform_page, page);
    for volume in db.volumes().unwrap() {
        db.set_volume_index(
            &volume.id,
            crate::infrastructure::db::FileIndexMode::Allowlist,
            &[std::path::Path::new("/synthetic/selected")
                .strip_prefix(&volume.path)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()],
            &[],
        )
        .unwrap();
    }
    assert_eq!(
        search::count(&platform, &runtime, "cat", "text:cat")
            .await
            .unwrap(),
        69
    );
    worker.abort();
    runtime.state.lock().unwrap().status = ImageSearchStatusType::Unavailable;
    assert_eq!(
        search::count(&catalog, &runtime, "cat", "text:cat")
            .await
            .unwrap(),
        10
    );
}
