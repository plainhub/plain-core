use super::*;

#[cfg(feature = "http_transport")]
#[path = "runtime/multi_device.rs"]
mod multi_device;

struct Directory(std::path::PathBuf);

impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(plain_rs::utils::short_uuid::short_uuid());
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn access_primitives_report_capabilities_and_grants_without_an_app_business_row() {
    let directory = Directory::new();
    let core = Arc::new(ContentServer::open(&directory.0).unwrap());
    let runtime = Runtime::new(core.clone(), vec!["MEDIA_SCAN".into()]);
    assert_eq!(
        runtime
            .dispatch("peerTransportCapabilities", serde_json::json!({}))
            .await
            .unwrap(),
        serde_json::json!([])
    );
    assert_eq!(
        runtime
            .dispatch("systemCapabilities", serde_json::json!({}))
            .await
            .unwrap(),
        serde_json::json!(["MEDIA_SCAN"])
    );
    let permissions = runtime
        .dispatch(
            "systemPermissionFacts",
            serde_json::json!({"permissions":["WRITE_EXTERNAL_STORAGE","READ_SMS"]}),
        )
        .await
        .unwrap();
    assert_eq!(
        permissions,
        serde_json::json!({"granted":{"WRITE_EXTERNAL_STORAGE":true,"READ_SMS":false}})
    );
    assert!(
        runtime
            .dispatch("unregisteredMethod", serde_json::json!({}))
            .await
            .is_err()
    );
    assert!(
        runtime
            .dispatch("systemMountFacts", serde_json::json!({}))
            .await
            .is_err()
    );
    assert!(
        runtime
            .dispatch("systemDisksFacts", serde_json::json!({}))
            .await
            .is_err()
    );
    core.stop_core().await;
}

#[tokio::test]
async fn native_app_query_uses_startup_data_and_live_capabilities_without_public_listener() {
    let directory = Directory::new();
    let core = Arc::new(ContentServer::open(&directory.0).unwrap());
    let runtime = Arc::new(Runtime::new(core.clone(), vec!["MEDIA_SCAN".into()]));
    let runner = tokio::spawn(runtime.run());
    let response = core
        .native_graphql()
        .unwrap()
        .execute("query { app { appDir deviceType capabilities httpPort httpsPort } }".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(
        data["app"]["appDir"],
        directory.0.to_string_lossy().as_ref()
    );
    assert_eq!(data["app"]["deviceType"], "COMPUTER");
    let mut capabilities = vec!["MEDIA_SCAN"];
    capabilities.extend(crate::services::files::browser::capabilities());
    assert_eq!(data["app"]["capabilities"], serde_json::json!(capabilities));
    assert_eq!(data["app"]["httpPort"], 0);
    assert_eq!(data["app"]["httpsPort"], 0);
    core.stop_core().await;
    runner.await.unwrap().unwrap();
}

#[tokio::test]
async fn native_file_resolution_uses_actual_os_read_authorization() {
    let directory = Directory::new();
    let path = directory.0.join("readable.txt");
    std::fs::write(&path, "actual bytes").unwrap();
    let core = Arc::new(ContentServer::open(&directory.0).unwrap());
    let runtime = Arc::new(Runtime::new(core.clone(), vec![]));
    assert!(
        runtime
            .dispatch("fileTaskAuthorize", serde_json::json!({"paths":[path]}))
            .await
            .is_err()
    );
    assert!(
        runtime
            .dispatch(
                "fileTaskAuthorize",
                serde_json::json!({"operation":"write","paths":[path]})
            )
            .await
            .is_err()
    );
    let runner = tokio::spawn(runtime.run());
    assert_eq!(
        core.resolve_native_file(path.to_str().unwrap())
            .await
            .unwrap(),
        path
    );
    let missing = directory.0.join("missing.txt");
    assert!(
        core.resolve_native_file(missing.to_str().unwrap())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"actual bytes");
    core.stop_core().await;
    runner.await.unwrap().unwrap();
    assert!(
        core.resolve_native_file(path.to_str().unwrap())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn media_buckets_are_served_by_the_core_catalog() {
    let directory = Directory::new();
    let core = Arc::new(ContentServer::open(&directory.0).unwrap());
    let path = directory.0.join("report.txt");
    core.native_media_catalog()
        .unwrap()
        .record_files(&[plain_rs::db::MediaFileMetadata {
            album: String::new(),
            album_artist: String::new(),

            data_type: plain_rs::enums::DataType::Doc,
            path: path.to_str().unwrap().into(),
            title: "Document title".into(),
            artist: String::new(),
            bucket_id: directory.0.to_str().unwrap().into(),
            size_bytes: 12,
            duration_ms: 0,
            cover_image_id: None,
            width: 0,
            height: 0,
            rotation: 0,
            taken_at: None,
            created_at: "2026-10-13T00:00:00.000Z".into(),
            updated_at: "2026-10-13T00:00:00.000Z".into(),
        }])
        .unwrap();
    let runtime = Runtime::new(core.clone(), vec![]);
    let runtime = Arc::new(runtime);
    let runner = tokio::spawn(runtime.clone().run());
    let response = core
        .native_graphql()
        .unwrap()
        .execute("query { mediaBuckets(type: DOC) { id name itemCount topItemPaths } }".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(
        data["mediaBuckets"][0]["id"],
        directory.0.canonicalize().unwrap().to_str().unwrap()
    );
    assert_eq!(data["mediaBuckets"][0]["itemCount"], 1);
    assert_eq!(
        data["mediaBuckets"][0]["topItemPaths"],
        serde_json::json!([directory
            .0
            .canonicalize()
            .unwrap()
            .join(path.file_name().unwrap())])
    );
    core.stop_core().await;
    runner.await.unwrap().unwrap();
}

#[tokio::test]
async fn completion_scan_runs_directly_without_another_host_request() {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let directory = Directory(home.join(plain_rs::utils::short_uuid::short_uuid()));
    std::fs::create_dir(&directory.0).unwrap();
    let core = Arc::new(ContentServer::open(&directory.0.as_path().join("core")).unwrap());
    core.native_media_catalog()
        .unwrap()
        .configure_scan_directories(None)
        .unwrap();
    let path = directory.0.as_path().join("completed.txt");
    std::fs::write(&path, b"completed file").unwrap();
    let runtime = Arc::new(Runtime::new(core.clone(), vec![]));
    let mut calls = tokio::task::JoinSet::new();
    for _ in 0..32 {
        let runtime = runtime.clone();
        let path = path.clone();
        calls.spawn(async move {
            runtime
                .core
                .native_media_catalog()
                .unwrap()
                .scan_completed_files(serde_json::json!({"paths":[path]}))
        });
    }
    while let Some(result) = calls.join_next().await {
        assert!(result.unwrap().unwrap());
    }
    assert_eq!(
        core.native_media_catalog()
            .unwrap()
            .count(plain_rs::enums::DataType::Doc)
            .unwrap(),
        1
    );
    let missing = directory.0.as_path().join("missing.txt");
    assert!(
        core.native_media_catalog()
            .unwrap()
            .scan_completed_files(serde_json::json!({"paths":[missing]}))
            .is_err()
    );
    core.stop_core().await;
}

#[tokio::test]
async fn device_and_log_queries_use_the_headless_server_provider_without_a_legacy_context() {
    let directory = Directory::new();
    let logs = directory.0.join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::write(logs.join("latest.log"), "first\nsecond\n").unwrap();
    let core = Arc::new(ContentServer::open(&directory.0).unwrap());
    core.set_log_file(&logs.join("latest.log")).unwrap();
    let runtime = Arc::new(Runtime::new(core.clone(), Vec::new()));
    let runner = tokio::spawn(runtime.run());
    let response = core.native_graphql().unwrap().execute("{ deviceInfo { platform cpuArch appVersion } deviceStatus { uptimeSec cpuUsage } appLogs(query:\"\",offset:0,limit:2) }".into()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["deviceInfo"]["cpuArch"], std::env::consts::ARCH);
    assert_eq!(data["appLogs"], serde_json::json!(["second", "first"]));
    let response = core
        .native_graphql()
        .unwrap()
        .execute("mutation { clearAppLogs }".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        std::fs::read(logs.join("latest.log")).unwrap(),
        Vec::<u8>::new()
    );
    core.stop_core().await;
    runner.await.unwrap().unwrap();
}

#[tokio::test]
async fn thumbnails_and_regular_file_probes_use_the_shared_media_engine() {
    let directory = Directory::new();
    let image_path = directory.0.join("photo.png");
    image::RgbImage::new(8, 4).save(&image_path).unwrap();
    let core = Arc::new(ContentServer::open(&directory.0).unwrap());
    core.native_media_catalog()
        .unwrap()
        .configure_scan_directories(None)
        .unwrap();
    let runtime = Arc::new(Runtime::new(core.clone(), Vec::new()));
    let runner = tokio::spawn(runtime.clone().run());
    let state = core.headless_state();
    let bytes = state
        .thumbnails
        .get(
            state,
            crate::services::images::thumbnails::Request {
                path: image_path.to_string_lossy().into_owned(),
                width: 4,
                height: 4,
                center_crop: true,
                file_id: String::new(),
                file_name: "photo.png".into(),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 4);
    assert_eq!(image::load_from_memory(&bytes).unwrap().height(), 4);
    assert!(!plain_rs::files::filesystem::image_format::is_animated_image_or_svg(&image_path));
    core.stop_core().await;
    runner.await.unwrap().unwrap();
}

#[tokio::test]
async fn headless_dispatch_rejects_the_retired_image_metadata_operation() {
    let directory = Directory::new();
    let core = Arc::new(ContentServer::open(&directory.0).unwrap());
    let runtime = Runtime::new(core.clone(), Vec::new());
    assert!(
        runtime
            .dispatch(
                "systemImageFileInfo",
                serde_json::json!({"path":"photo.png"})
            )
            .await
            .is_err()
    );
    core.stop_core().await;
}

#[tokio::test]
async fn audio_queue_uses_the_registered_host_and_keeps_unindexed_playback_temporary() {
    let directory = tempfile::tempdir().unwrap();
    let indexed = directory.path().join("indexed.mp3");
    let raw = directory.path().join("raw.mp3");
    let fixture = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/audio-untagged.mp3"
    ));
    std::fs::copy(fixture, &indexed).unwrap();
    std::fs::copy(fixture, &raw).unwrap();
    let core = Arc::new(ContentServer::open(directory.path()).unwrap());
    let db = core.runtime_state().db.clone();
    let rows = db
        .record_media_files(&[plain_rs::db::MediaFileMetadata {
            album: String::new(),
            album_artist: String::new(),

            data_type: plain_rs::enums::DataType::Audio,
            path: indexed.to_string_lossy().into_owned(),
            title: "Indexed".into(),
            artist: "Artist".into(),
            bucket_id: String::new(),
            size_bytes: 1,
            duration_ms: 123,
            cover_image_id: None,
            width: 0,
            height: 0,
            rotation: 0,
            taken_at: None,
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
        }])
        .unwrap();
    let id = rows[0].id.clone();
    let runtime = Arc::new(Runtime::new(core.clone(), Vec::new()));
    let runner = tokio::spawn(runtime.run());
    let executor = core.native_graphql().unwrap();
    let response=executor.execute(format!(r#"mutation {{ addAudioToQueue(query:"ids:{id}") playAudio(mediaId:"{id}") {{ mediaId path }} }}"#).into()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["playAudio"]["mediaId"],
        id
    );
    let response=executor.execute("{ audioQueueItemCount audioQueueItems(query:\"\",offset:0,limit:200) { mediaId } audioPlayback { mediaId isPlaying } }".into()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let page = response.data.into_json().unwrap();
    assert_eq!(page["audioQueueItemCount"], 1);
    assert_eq!(page["audioQueueItems"][0]["mediaId"], id);
    assert_eq!(page["audioPlayback"]["mediaId"], id);
    assert_eq!(page["audioPlayback"]["isPlaying"], false);
    let response = executor
        .execute(format!(r#"mutation {{ removeAudioFromQueue(mediaId:"{id}") }}"#).into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let response = executor
        .execute(
            format!(
                r#"mutation {{ audioPlayPath(path:{}) {{ path }} }}"#,
                serde_json::to_string(&raw.to_string_lossy()).unwrap()
            )
            .into(),
        )
        .await;
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    assert_eq!(
        response.errors[0].message,
        "Native audio playback is unavailable"
    );
    let response = executor
        .execute("{ audioPlayback {mediaId path isPlaying} }".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let result = response.data.into_json().unwrap();
    assert!(result["audioPlayback"]["mediaId"].is_null());
    assert_eq!(result["audioPlayback"]["isPlaying"], false);
    assert_eq!(
        result["audioPlayback"]["path"],
        raw.to_string_lossy().as_ref()
    );
    assert_eq!(
        db.media_file_count(plain_rs::enums::DataType::Audio)
            .unwrap(),
        1
    );
    assert_eq!(plain_rs::media::audio::queue::queue_total(&db).unwrap(), 0);
    let response = executor
        .execute("mutation { clearAudioQueue }".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        plain_rs::media::audio::queue::get_audio_current(&db).unwrap(),
        ""
    );
    core.stop_core().await;
    runner.await.unwrap().unwrap();
}
