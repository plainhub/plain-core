use crate::features::services::images::thumbnails::Request;
use crate::test_runtime::Core as ContentServer;
use sha2::Digest;
use std::sync::Arc;

#[tokio::test]
async fn native_thumbnail_output_is_one_shot_and_validates_before_consuming() {
    let thumbnails = super::Thumbnails::default();
    let (sender, mut receiver) = tokio::sync::oneshot::channel();
    thumbnails
        .outputs
        .lock()
        .unwrap()
        .insert("fixture".into(), sender);
    assert!(thumbnails.submit("fixture", Vec::new()).is_err());
    assert!(
        thumbnails
            .submit("fixture", vec![0; 32 * 1024 * 1024 + 1])
            .is_err()
    );
    thumbnails.submit("fixture", vec![0, 255, 1]).unwrap();
    assert_eq!(receiver.try_recv().unwrap(), vec![0, 255, 1]);
    assert!(thumbnails.submit("fixture", vec![1]).is_err());
    assert!(thumbnails.submit("expired", vec![1]).is_err());
}

#[test]
fn cancelled_thumbnail_output_discards_its_token() {
    let thumbnails = super::Thumbnails::default();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    thumbnails
        .outputs
        .lock()
        .unwrap()
        .insert("cancelled".into(), sender);
    let output = super::Output {
        service: &thumbnails,
        token: "cancelled".into(),
    };
    drop(output);
    assert!(thumbnails.submit("cancelled", vec![1]).is_err());
    assert!(receiver.blocking_recv().is_err());
}

fn request_for(path: &std::path::Path) -> Request {
    Request {
        path: path.to_str().unwrap().to_owned(),
        width: 80,
        height: 60,
        center_crop: true,
        file_id: String::new(),
        file_name: String::new(),
    }
}

#[tokio::test]
async fn overload_is_rejected_before_resolving_or_decoding_a_source() {
    let (_directory, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let full = state.thumbnails.requests.acquire_many(320).await.unwrap();
    let request = request_for(std::path::Path::new("/missing-thumbnail-fixture.png"));
    assert!(matches!(
        state.thumbnails.get(&state, request).await,
        Err(super::Error::Busy)
    ));
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    drop(full);
    core.stop_core().await;
}

fn server_with_cache() -> (tempfile::TempDir, ContentServer, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(crate::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "thumb-fixture").unwrap();
    let cache = dir.path().join("cache");
    let core = ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[4; 32]),
        prefs,
    )
    .unwrap();
    (dir, core, cache)
}

#[tokio::test]
async fn sdk_decode_validates_receipts_and_cleans_every_output_token() {
    for (receipt, output, expected) in [
        (
            serde_json::json!(true),
            Some(vec![0, 255, 1]),
            Ok(Some(vec![0, 255, 1])),
        ),
        (serde_json::json!(null), None, Ok(None)),
        (
            serde_json::json!(false),
            None,
            Err("invalid thumbnail receipt"),
        ),
        (
            serde_json::json!(true),
            None,
            Err("missing thumbnail output"),
        ),
    ] {
        let (_dir, core, _cache) = server_with_cache();
        let state = core.runtime_state();
        let (generation, mut requests) = state.host.connect();
        let owned = state.clone();
        let worker = tokio::spawn(async move {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "thumbnailDecode");
            assert_eq!(request["params"]["mediaId"], "sdk-id");
            assert_eq!(request["params"]["width"], 80);
            let token = request["params"]["outputToken"].as_str().unwrap();
            if let Some(bytes) = output {
                owned.thumbnails.submit(token, bytes).unwrap();
            }
            owned
                .host
                .reply(
                    generation,
                    serde_json::json!({"id":request["id"],"result":receipt}),
                )
                .unwrap();
        });
        let mut request = request_for(std::path::Path::new("/image.jpg"));
        request.file_id = "sdk-id".into();
        let actual = super::decode_sdk(&state, &request, true).await;
        assert_eq!(
            actual
                .as_ref()
                .map(|value| value.clone())
                .map_err(String::as_str),
            expected
        );
        worker.await.unwrap();
        assert!(state.thumbnails.outputs.lock().unwrap().is_empty());
        core.stop_core().await;
    }
}

#[tokio::test]
async fn cancelling_a_pending_sdk_decode_removes_its_token() {
    let (_dir, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let owned = state.clone();
    let call = tokio::spawn(async move {
        super::decode_sdk(
            &owned,
            &request_for(std::path::Path::new("/image.jpg")),
            false,
        )
        .await
    });
    let request = requests.recv().await.unwrap();
    let token = request["params"]["outputToken"].as_str().unwrap();
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    assert!(state.thumbnails.outputs.lock().unwrap().is_empty());
    assert!(state.thumbnails.submit(token, vec![1]).is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn shutdown_releases_a_pending_sdk_decode_without_waiting_for_the_host_reply() {
    let (_dir, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let owned = state.clone();
    let call = tokio::spawn(async move {
        super::decode_sdk(
            &owned,
            &request_for(std::path::Path::new("/image.jpg")),
            false,
        )
        .await
    });
    requests.recv().await.unwrap();
    core.stop_core().await;
    assert_eq!(call.await.unwrap().unwrap_err(), "Rust core stopped");
    assert!(state.thumbnails.outputs.lock().unwrap().is_empty());
}

#[tokio::test]
async fn cancelling_a_started_generation_keeps_its_slot_and_single_flight_until_it_finishes() {
    let (_directory, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("image.jpg");
    image::RgbImage::from_pixel(800, 400, image::Rgb([200, 10, 20]))
        .save(&path)
        .unwrap();
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *state.thumbnails.decode_gate.lock().unwrap() = Some(gate.clone());
    let request = request_for(&path);
    let first = tokio::spawn({
        let state = state.clone();
        let request = request.clone();
        async move { state.thumbnails.get(&state, request).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst)
            == 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    assert_eq!(state.thumbnails.capacity.available_permits(), 3);
    let next = state.thumbnails.get(&state, request);
    tokio::pin!(next);
    assert!(futures_util::poll!(next.as_mut()).is_pending());
    gate.add_permits(1);
    assert!(next.await.unwrap().is_some());
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(state.thumbnails.capacity.available_permits(), 4);
    core.stop_core().await;
}

#[tokio::test]
async fn cancelling_300_queued_requests_releases_admission_without_starting_a_decode() {
    let (_directory, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("image.jpg");
    image::RgbImage::from_pixel(800, 400, image::Rgb([10, 200, 20]))
        .save(&path)
        .unwrap();
    let held = state.thumbnails.capacity.acquire_many(4).await.unwrap();
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..300 {
        let state = state.clone();
        let request = request_for(&path);
        requests.spawn(async move { state.thumbnails.get(&state, request).await });
    }
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while state.thumbnails.requests.available_permits() != 20 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    requests.abort_all();
    while requests.join_next().await.is_some() {}
    assert_eq!(state.thumbnails.requests.available_permits(), 320);
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    drop(held);
    assert!(
        state
            .thumbnails
            .get(&state, request_for(&path))
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    core.stop_core().await;
}

#[tokio::test]
async fn changed_source_during_generation_cannot_publish_an_old_version() {
    let (_directory, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("image.jpg");
    image::RgbImage::from_pixel(800, 400, image::Rgb([240, 10, 10]))
        .save(&path)
        .unwrap();
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *state.thumbnails.decode_gate.lock().unwrap() = Some(gate.clone());
    let old = tokio::spawn({
        let state = state.clone();
        let request = request_for(&path);
        async move { state.thumbnails.get(&state, request).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst)
            == 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    image::RgbImage::from_pixel(800, 400, image::Rgb([10, 240, 10]))
        .save(&path)
        .unwrap();
    gate.add_permits(1);
    assert!(old.await.unwrap().unwrap().is_none());
    *state.thumbnails.decode_gate.lock().unwrap() = None;
    let bytes = state
        .thumbnails
        .get(&state, request_for(&path))
        .await
        .unwrap()
        .unwrap();
    let pixel = image::load_from_memory(&bytes)
        .unwrap()
        .to_rgb8()
        .get_pixel(30, 30)
        .0;
    assert!(pixel[1] > pixel[0] + 100);
    core.stop_core().await;
}

#[tokio::test]
async fn background_generation_yields_to_a_foreground_request_for_the_same_image() {
    let (_directory, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("image.jpg");
    image::RgbImage::from_pixel(800, 400, image::Rgb([30, 100, 200]))
        .save(&path)
        .unwrap();
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *state.thumbnails.decode_gate.lock().unwrap() = Some(gate.clone());
    let foreground = tokio::spawn({
        let state = state.clone();
        let request = request_for(&path);
        async move { state.thumbnails.get(&state, request).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst)
            == 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let background = state.thumbnails.get_inner(&state, request_for(&path), true);
    tokio::pin!(background);
    assert!(futures_util::poll!(background.as_mut()).is_pending());
    gate.add_permits(1);
    assert!(foreground.await.unwrap().unwrap().is_some());
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), background)
            .await
            .unwrap()
            .unwrap()
            .is_some()
    );
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    core.stop_core().await;
}

#[tokio::test]
async fn background_work_waits_for_foreground_and_wakes_when_the_last_request_leaves() {
    use std::{future::Future, task::Poll};
    let (_dir, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    state.thumbnails.wait_for_foreground(&state).await.unwrap();
    let first = super::Foreground {
        permit: Some(state.thumbnails.requests.acquire().await.unwrap()),
        idle: &state.thumbnails.idle,
    };
    let second = super::Foreground {
        permit: Some(state.thumbnails.requests.acquire().await.unwrap()),
        idle: &state.thumbnails.idle,
    };
    let waiting = state.thumbnails.wait_for_foreground(&state);
    tokio::pin!(waiting);
    assert!(std::future::poll_fn(|cx| Poll::Ready(waiting.as_mut().poll(cx).is_pending())).await);
    drop(first);
    assert!(std::future::poll_fn(|cx| Poll::Ready(waiting.as_mut().poll(cx).is_pending())).await);
    drop(second);
    waiting.await.unwrap();
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn hot_cache_serves_300_requests_without_disk_reads_and_rechecks_source() {
    use std::sync::atomic::Ordering;
    let (directory, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let path = directory.path().join("hot.jpg");
    image::RgbImage::from_pixel(800, 400, image::Rgb([240, 10, 10]))
        .save(&path)
        .unwrap();
    let first = state
        .thumbnails
        .get(&state, request_for(&path))
        .await
        .unwrap()
        .unwrap();
    let reads = state.thumbnails.cache_reads.load(Ordering::SeqCst);
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..300 {
        let state = state.clone();
        let request = request_for(&path);
        requests.spawn(async move { state.thumbnails.get(&state, request).await });
    }
    while let Some(result) = requests.join_next().await {
        assert_eq!(result.unwrap().unwrap().unwrap(), first);
    }
    assert_eq!(state.thumbnails.cache_reads.load(Ordering::SeqCst), reads);
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 1);
    state.thumbnails.hot.clear();
    assert_eq!(
        state
            .thumbnails
            .get(&state, request_for(&path))
            .await
            .unwrap()
            .unwrap(),
        first
    );
    assert_eq!(
        state.thumbnails.cache_reads.load(Ordering::SeqCst),
        reads + 1
    );
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 1);
    image::RgbImage::from_pixel(900, 450, image::Rgb([10, 240, 10]))
        .save(&path)
        .unwrap();
    let updated = state
        .thumbnails
        .get(&state, request_for(&path))
        .await
        .unwrap()
        .unwrap();
    assert_ne!(updated, first);
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 2);
    std::fs::remove_file(&path).unwrap();
    assert!(
        state
            .thumbnails
            .get(&state, request_for(&path))
            .await
            .unwrap()
            .is_none()
    );
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn sufficient_originals_skip_generation_and_thumbnail_cache_files() {
    let (directory, core, cache) = server_with_cache();
    let state = core.runtime_state();
    for extension in ["jpg", "png", "webp"] {
        let path = directory.path().join(format!("original.{extension}"));
        image::RgbImage::from_pixel(80, 80, image::Rgb([200, 40, 50]))
            .save(&path)
            .unwrap();
        let mut request = request_for(&path);
        request.width = 96;
        request.height = 96;
        let bytes = state
            .thumbnails
            .get(&state, request)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(bytes, std::fs::read(path).unwrap());
    }
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert!(!cache.join("thumbs").exists());
    core.stop_core().await;
}
