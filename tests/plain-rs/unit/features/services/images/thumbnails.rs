use crate::features::services::images::thumbnails::Request;
use crate::test_runtime::Core as ContentServer;
use sha2::Digest;
use std::sync::Arc;

#[tokio::test]
async fn native_thumbnail_output_is_one_shot_and_validates_before_consuming() {
    let thumbnails = super::Thumbnails::new();
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
    let thumbnails = super::Thumbnails::new();
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
        access_order: None,
        file_name: String::new(),
    }
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
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
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
    assert_eq!(state.thumbnails.queue.waiting(), 0);
    core.stop_core().await;
}

#[tokio::test]
async fn disconnected_queued_requests_are_removed_without_starting_a_decode() {
    let (_directory, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let workers = state.thumbnails.workers();
    let queued = 300;
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("image.jpg");
    image::RgbImage::from_pixel(800, 400, image::Rgb([10, 200, 20]))
        .save(&path)
        .unwrap();
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *state.thumbnails.decode_gate.lock().unwrap() = Some(gate.clone());
    let mut active = tokio::task::JoinSet::new();
    for index in 0..workers {
        let state = state.clone();
        let active_path = source.path().join(format!("active-{index}.jpg"));
        std::fs::copy(&path, &active_path).unwrap();
        let request = request_for(&active_path);
        active.spawn(async move { state.thumbnails.get(&state, request).await });
    }
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst)
            != workers
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..queued {
        let state = state.clone();
        let request = request_for(&path);
        requests.spawn(async move { state.thumbnails.get(&state, request).await });
    }
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while state.thumbnails.queue.waiting() != queued + workers {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    requests.abort_all();
    while requests.join_next().await.is_some() {}
    assert_eq!(state.thumbnails.queue.waiting(), workers);
    gate.add_permits(workers);
    while let Some(result) = active.join_next().await {
        assert!(result.unwrap().unwrap().is_some());
    }
    *state.thumbnails.decode_gate.lock().unwrap() = None;
    let next = request_for(&path);
    assert!(state.thumbnails.get(&state, next).await.unwrap().is_some());
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst),
        workers + 1
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
    for index in 0..300 {
        let state = state.clone();
        let size = [50, 96, 128, 256, 512][index % 5];
        let request = Request {
            width: size,
            height: size,
            ..request_for(&path)
        };
        requests.spawn(async move { state.thumbnails.get(&state, request).await });
    }
    while let Some(result) = requests.join_next().await {
        let bytes = result.unwrap().unwrap().unwrap();
        assert_eq!(bytes, first);
        assert_eq!(
            bytes.as_ptr(),
            first.as_ptr(),
            "cache hits must share encoded storage"
        );
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

#[tokio::test]
async fn concurrent_misses_share_one_failed_generation_and_a_later_request_can_retry() {
    use std::sync::atomic::Ordering;
    let (directory, core, _) = server_with_cache();
    let state = core.runtime_state();
    let path = directory.path().join("invalid.jpg");
    std::fs::write(&path, b"invalid jpeg").unwrap();
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *state.thumbnails.decode_gate.lock().unwrap() = Some(gate.clone());
    let mut readers = tokio::task::JoinSet::new();
    for _ in 0..300 {
        let state = state.clone();
        let request = request_for(&path);
        readers.spawn(async move { state.thumbnails.get(&state, request).await });
    }
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while state.thumbnails.queue.waiting() != 300 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    gate.add_permits(301);
    let mut errors = std::collections::HashSet::new();
    while let Some(result) = readers.join_next().await {
        errors.insert(result.unwrap().unwrap_err().to_string());
    }
    assert_eq!(errors.len(), 1);
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 1);
    assert!(
        state
            .thumbnails
            .get(&state, request_for(&path))
            .await
            .is_err()
    );
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 2);
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn concurrent_generation_delivers_one_shared_allocation_to_300_readers() {
    use std::sync::atomic::Ordering;
    let (directory, core, _) = server_with_cache();
    let state = core.runtime_state();
    let path = directory.path().join("shared.jpg");
    image::RgbImage::from_pixel(800, 400, image::Rgb([200, 40, 50]))
        .save(&path)
        .unwrap();
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *state.thumbnails.decode_gate.lock().unwrap() = Some(gate.clone());
    let mut readers = tokio::task::JoinSet::new();
    for _ in 0..300 {
        let state = state.clone();
        let request = request_for(&path);
        readers.spawn(async move { state.thumbnails.get(&state, request).await });
    }
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while state.thumbnails.queue.waiting() != 300 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    gate.add_permits(300);
    let first = readers
        .join_next()
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .unwrap();
    while let Some(result) = readers.join_next().await {
        let bytes = result.unwrap().unwrap().unwrap();
        assert_eq!(bytes, first);
        assert_eq!(bytes.as_ptr(), first.as_ptr());
    }
    let cached = state
        .thumbnails
        .get(&state, request_for(&path))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cached.as_ptr(), first.as_ptr());
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 1);
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn thumbnail_sizes_share_256_bytes_and_preserve_large_previews() {
    use std::sync::atomic::Ordering;
    let (directory, core, cache) = server_with_cache();
    let state = core.runtime_state();
    let path = directory.path().join("sizes.jpg");
    image::RgbImage::from_fn(1600, 1200, |x, y| {
        image::Rgb([(x % 251) as u8, (y % 239) as u8, 90])
    })
    .save(&path)
    .unwrap();
    let expected =
        plain_libs::media::thumbnails::render(path.to_str().unwrap(), 256, 256, true, &cache)
            .await
            .unwrap()
            .unwrap();
    let mut requests = tokio::task::JoinSet::new();
    for size in [50, 96, 128, 256, 512] {
        let state = state.clone();
        let request = Request {
            width: size,
            height: size,
            ..request_for(&path)
        };
        requests.spawn(async move { state.thumbnails.get(&state, request).await });
    }
    while let Some(result) = requests.join_next().await {
        assert_eq!(result.unwrap().unwrap().unwrap(), expected);
    }
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 1);
    assert_eq!(
        state.thumbnails.cache_bytes_written.load(Ordering::Relaxed),
        expected.len() as u64
    );
    for (width, height, crop, dimensions) in [
        (96, 96, false, (256, 192)),
        (1024, 1024, false, (1024, 768)),
        (512, 1024, false, (512, 384)),
        (1024, 512, false, (682, 512)),
    ] {
        let request = Request {
            width,
            height,
            center_crop: crop,
            ..request_for(&path)
        };
        let bytes = state
            .thumbnails
            .get(&state, request)
            .await
            .unwrap()
            .unwrap();
        let image = image::load_from_memory(&bytes).unwrap();
        assert_eq!((image.width(), image.height()), dimensions);
    }
    let invalid = Request {
        width: 0,
        ..request_for(&path)
    };
    assert!(state.thumbnails.get(&state, invalid).await.is_err());
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn real_image_extensions_skip_old_white_thumbnails_and_preserve_source_keys() {
    let (directory, core, _) = server_with_cache();
    let state = core.runtime_state();
    let source = directory.path().join("alpha.PNG");
    image::RgbaImage::from_pixel(512, 512, image::Rgba([40, 90, 150, 0]))
        .save(&source)
        .unwrap();
    let source = source.canonicalize().unwrap();
    let request = Request {
        width: 256,
        height: 256,
        ..request_for(&source)
    };
    let legacy_key = |request: &Request| {
        let metadata = std::fs::metadata(&request.path).unwrap();
        let input = serde_json::json!([
            request.path,
            metadata.len(),
            format!("{:?}", metadata.modified().unwrap()),
            request.width,
            request.height,
            request.center_crop,
            "",
            "",
            "thumbnail-v2"
        ])
        .to_string();
        plain_libs::utils::hex::bytes_to_hex(&sha2::Sha256::digest(input.as_bytes()))
    };
    let old_key = legacy_key(&request);
    assert_eq!(
        super::source_fingerprint(&request).unwrap().unwrap(),
        old_key
    );
    let old = state
        .file_platform
        .thumbnail_cache(&request.path)
        .unwrap()
        .join("thumbs/rust")
        .join(format!("{old_key}.thumb"));
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, b"legacy flattened thumbnail").unwrap();
    let bytes = state
        .thumbnails
        .get(&state, request)
        .await
        .unwrap()
        .unwrap();
    let generated = old.with_file_name(format!(
        "{}.png",
        super::source_fingerprint(&Request {
            width: 256,
            height: 256,
            ..request_for(&source)
        })
        .unwrap()
        .unwrap()
    ));
    assert_eq!(std::fs::read(&generated).unwrap(), bytes);
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(
        image::load_from_memory(&bytes)
            .unwrap()
            .into_rgba8()
            .get_pixel(128, 128)[3],
        0
    );
    let jpeg = directory.path().join("photo.jpg");
    image::RgbImage::from_pixel(512, 512, image::Rgb([40, 90, 150]))
        .save(&jpeg)
        .unwrap();
    let request = Request {
        width: 256,
        height: 256,
        ..request_for(&jpeg.canonicalize().unwrap())
    };
    assert_eq!(
        super::source_fingerprint(&request).unwrap().unwrap(),
        legacy_key(&request)
    );
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[test]
fn cached_sdk_png_uses_its_real_extension_and_hot_hits_do_not_read_disk() {
    use std::sync::atomic::Ordering;
    let directory = tempfile::tempdir().unwrap();
    let requested = directory.path().join("key.jpg");
    let actual = requested.with_extension("png");
    let bytes = b"\x89PNG\r\n\x1a\nencoded-sdk-output";
    std::fs::write(&actual, bytes).unwrap();
    let service = super::Thumbnails::new();
    assert_eq!(
        service.cached_bytes(&requested).unwrap().unwrap(),
        bytes.as_slice()
    );
    let reads = service.cache_reads.load(Ordering::SeqCst);
    assert_eq!(reads, 2);
    assert_eq!(
        service.cached_bytes(&requested).unwrap().unwrap(),
        bytes.as_slice()
    );
    assert_eq!(service.cache_reads.load(Ordering::SeqCst), reads);
    assert!(!requested.exists());
    assert!(!requested.with_extension("thumb").exists());
}

#[tokio::test]
async fn waiting_for_generation_does_not_hold_file_lookup_capacity() {
    let (directory, core, _) = server_with_cache();
    let state = core.runtime_state();
    let path = directory.path().join("lookup-capacity.jpg");
    image::RgbImage::from_pixel(640, 480, image::Rgb([40, 90, 150]))
        .save(&path)
        .unwrap();
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *state.thumbnails.decode_gate.lock().unwrap() = Some(gate.clone());
    let current = tokio::spawn({
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
    assert_eq!(
        state.thumbnails.lookups.available_permits(),
        super::LOOKUP_LIMIT
    );
    gate.add_permits(1);
    assert!(current.await.unwrap().unwrap().is_some());
    core.stop_core().await;
}
