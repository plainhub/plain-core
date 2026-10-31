use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
async fn wait(service: &ImageIndex) {
    tokio::time::timeout(std::time::Duration::from_secs(5), service.wait_until_idle())
        .await
        .unwrap();
}
fn setup(
    fail_page: bool,
) -> (
    Arc<ImageIndex>,
    tokio::task::JoinHandle<()>,
    Arc<AtomicUsize>,
) {
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let ended = Arc::new(AtomicUsize::new(0));
    let peer = host.clone();
    let task = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let id = &request["id"];
            let p = &request["params"];
            let result = match request["method"].as_str().unwrap() {
                "imageCatalogSnapshot" => json!({"revision":"one","total":300}),
                "imageCatalogPage" => {
                    if fail_page {
                        peer.reply(generation, json!({"id":id,"error":"permission denied"}))
                            .unwrap();
                        continue;
                    }
                    let offset = p["cursor"].as_str().unwrap().parse::<usize>().unwrap_or(0);
                    let end = (offset + 128).min(300);
                    json!({"revision":"one","items":(offset..end).map(|n|json!({"id":n.to_string(),"path":format!("/synthetic/{n}")})).collect::<Vec<_>>(),"nextCursor":end.to_string(),"done":end==300})
                }
                "imageCatalogResolve" => json!(
                    p["ids"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(
                            |i| json!({"id":i,"path":format!("/synthetic/{}",i.as_str().unwrap())})
                        )
                        .collect::<Vec<_>>()
                ),
                "imageCatalogVerify" => json!(true),
                method => panic!("unexpected {method}"),
            };
            peer.reply(generation, json!({"id":id,"result":result}))
                .unwrap();
        }
    });
    let service = Arc::new(ImageIndex::new(
        db,
        host.clone(),
        tokio::sync::broadcast::channel(16).0,
        Arc::new(plain_rs::host::resources::NativeResources::new(host)),
        std::env::temp_dir(),
    ));
    service.use_system_catalog_for_test();
    struct Encoder(Arc<AtomicUsize>);
    impl plain_inference::embedding::ImageEncoder for Encoder {
        fn release(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn image_reader(
            &self,
            _: &mut dyn plain_inference::embedding::ImageSource,
        ) -> Result<Option<Vec<f32>>, String> {
            panic!("Synthetic local images must use the direct file path")
        }
        fn image(&self, _: &std::path::Path) -> Result<Option<Vec<f32>>, String> {
            Ok(Some(vec![1.0]))
        }
    }
    service.set_encoder(Some(Arc::new(Encoder(ended.clone()))));
    (service, task, ended)
}
#[tokio::test]
async fn host_job_runs_bounded_scan_and_releases_engine_before_completion() {
    let (service, task, ended) = setup(false);
    service.start(false).unwrap();
    wait(&service).await;
    assert_eq!(service.status().indexed_images, 300);
    assert!(service.status().error_message.is_empty());
    assert_eq!(image_embeddings::count(&service.db).unwrap(), 300);
    assert_eq!(ended.load(Ordering::SeqCst), 1);
    task.abort();
}
#[tokio::test]
async fn host_errors_are_visible_and_release_the_engine() {
    let (service, task, ended) = setup(true);
    service.start(false).unwrap();
    wait(&service).await;
    assert!(service.status().error_message.contains("permission denied"));
    assert_eq!(ended.load(Ordering::SeqCst), 1);
    task.abort();
}
#[tokio::test]
async fn cancelled_generation_cannot_resurrect_deleted_embeddings() {
    let (service, task, _) = setup(false);
    let control = IndexControl {
        generation: 0,
        state: service.state.clone(),
        events: service.events.clone(),
    };
    let input = EmbeddingInput {
        id: "one".into(),
        path: "/synthetic/one".into(),
        embedding_base64: plain_rs::utils::base64::base64_encode(&1.0_f32.to_be_bytes()),
    };
    image_embeddings::save(&service.db, std::slice::from_ref(&input)).unwrap();
    assert_eq!(service.remove(&["one".into()]).unwrap(), 1);
    assert!(control.save(&service.db, &[input]).is_err());
    assert_eq!(image_embeddings::count(&service.db).unwrap(), 0);
    task.abort();
}

#[tokio::test]
async fn native_full_and_selected_indexing_use_local_catalog_and_encoder_without_host_requests() {
    struct Encoder;
    impl plain_inference::embedding::ImageEncoder for Encoder {
        fn release(&self) {}
        fn image_reader(
            &self,
            _: &mut dyn plain_inference::embedding::ImageSource,
        ) -> Result<Option<Vec<f32>>, String> {
            panic!("Native catalog must provide local file paths")
        }
        fn image(&self, path: &std::path::Path) -> Result<Option<Vec<f32>>, String> {
            Ok(
                (std::fs::read(path).map_err(|error| error.to_string())? == b"supported")
                    .then_some(vec![1.0]),
            )
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let supported = directory.path().join("supported.png");
    let unsupported = directory.path().join("unsupported.image");
    std::fs::write(&supported, b"supported").unwrap();
    std::fs::write(&unsupported, b"unsupported").unwrap();
    for path in [&supported, &unsupported] {
        db.record_media_files(&[plain_rs::db::MediaFileMetadata {
            data_type: plain_rs::enums::DataType::Image,
            path: path.to_str().unwrap().into(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            album_artist: String::new(),
            bucket_id: String::new(),
            size_bytes: 1,
            duration_ms: 0,
            cover_image_id: None,
            width: 1,
            height: 1,
            rotation: 0,
            taken_at: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }])
        .unwrap();
    }
    image_embeddings::save(
        &db,
        &[EmbeddingInput {
            id: "obsolete".into(),
            path: "/obsolete.png".into(),
            embedding_base64: plain_rs::utils::base64::base64_encode(&1.0f32.to_be_bytes()),
        }],
    )
    .unwrap();
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let service = Arc::new(ImageIndex::new(
        db.clone(),
        host.clone(),
        tokio::sync::broadcast::channel(16).0,
        Arc::new(plain_rs::host::resources::NativeResources::new(host)),
        directory.path().join("cache"),
    ));
    service.use_native_catalog_for_test();
    assert!(!service.uses_system_catalog());
    service.set_encoder(Some(Arc::new(Encoder)));
    service.start(true).unwrap();
    wait(&service).await;
    assert_eq!(image_embeddings::ids(&db).unwrap(), ["1"]);
    assert_eq!(service.status().total_images, 2);
    assert_eq!(service.status().skipped_images, 1);
    assert!(requests.try_recv().is_err());
    image_embeddings::delete(&db, &["first".into()]).unwrap();
    service
        .selected(vec!["first".into(), "missing".into()])
        .unwrap();
    wait(&service).await;
    assert_eq!(image_embeddings::ids(&db).unwrap(), ["1"]);
    assert!(requests.try_recv().is_err());
    assert!(!directory.path().join("cache").exists());
}

#[tokio::test]
async fn cancellation_waits_for_actual_worker_exit_and_notifies_all_idle_waiters() {
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let service = Arc::new(ImageIndex::new(
        db,
        host.clone(),
        tokio::sync::broadcast::channel(16).0,
        Arc::new(plain_rs::host::resources::NativeResources::new(
            host.clone(),
        )),
        std::env::temp_dir(),
    ));
    service.use_system_catalog_for_test();
    service.start(false).unwrap();
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "imageCatalogSnapshot");
    service.cancel().unwrap();
    let mut waiting = Vec::new();
    for _ in 0..2 {
        let index = service.clone();
        let (started, started_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            started.send(()).unwrap();
            index.wait_until_idle().await;
        });
        started_rx.await.unwrap();
        assert!(!task.is_finished());
        waiting.push(task);
    }
    host.reply(
        generation,
        json!({"id":request["id"],"result":{"revision":"empty","total":0}}),
    )
    .unwrap();
    for task in waiting {
        tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
    }
    assert!(!service.status().is_running);
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn generation_failure_finishes_the_worker_and_idle_is_immediately_ready() {
    use futures_util::FutureExt;
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let host = Arc::new(Host::default());
    let service = Arc::new(ImageIndex::new(
        db,
        host.clone(),
        tokio::sync::broadcast::channel(16).0,
        Arc::new(plain_rs::host::resources::NativeResources::new(host)),
        std::env::temp_dir(),
    ));
    assert!(service.wait_until_idle().now_or_never().is_some());
    service.state.lock().unwrap().generation = u64::MAX;
    service.start(false).unwrap();
    wait(&service).await;
    assert_eq!(
        service.status().error_message,
        "image index generation overflow"
    );
    assert!(!service.status().is_running);
    assert!(service.wait_until_idle().now_or_never().is_some());
}
