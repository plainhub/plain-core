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
fn canceled_thumbnail_output_discards_its_token() {
    let thumbnails = super::Thumbnails::new();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    thumbnails
        .outputs
        .lock()
        .unwrap()
        .insert("CANCELED".into(), sender);
    let output = super::Output {
        service: &thumbnails,
        token: "CANCELED".into(),
    };
    drop(output);
    assert!(thumbnails.submit("CANCELED", vec![1]).is_err());
    assert!(receiver.blocking_recv().is_err());
}

fn request_for(path: &std::path::Path) -> Request {
    Request {
        path: path.to_str().unwrap().to_owned(),
        width: 80,
        height: 60,
        center_crop: true,
        short_edge: None,
        file_id: String::new(),
        access_order: None,
        file_name: String::new(),
    }
}

fn server_with_cache() -> (plain_libs::temporary::TempDir, ContentServer, std::path::PathBuf) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(crate::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "706c6169-9052-8f83-87af-3929ef3aa462").unwrap();
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
            plain_libs::serde_json::json!(true),
            Some(vec![0, 255, 1]),
            Ok(Some(vec![0, 255, 1])),
        ),
        (plain_libs::serde_json::json!(null), None, Ok(None)),
        (
            plain_libs::serde_json::json!(false),
            None,
            Err("invalid thumbnail receipt"),
        ),
        (
            plain_libs::serde_json::json!(true),
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
                    plain_libs::serde_json::json!({"id":request["id"],"result":receipt}),
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
async fn canceling_a_pending_sdk_decode_removes_its_token() {
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
async fn canceling_a_started_generation_keeps_its_slot_and_single_flight_until_it_finishes() {
    let (_directory, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let source = plain_libs::temporary::tempdir().unwrap();
    let path = source.path().join("image.jpg");
    plain_libs::image::RgbImage::from_pixel(800, 400, plain_libs::image::Rgb([200, 10, 20]))
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
    let source = plain_libs::temporary::tempdir().unwrap();
    let path = source.path().join("image.jpg");
    plain_libs::image::RgbImage::from_pixel(800, 400, plain_libs::image::Rgb([10, 200, 20]))
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
    let source = plain_libs::temporary::tempdir().unwrap();
    let path = source.path().join("image.jpg");
    plain_libs::image::RgbImage::from_pixel(800, 400, plain_libs::image::Rgb([240, 10, 10]))
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
    plain_libs::image::RgbImage::from_pixel(800, 400, plain_libs::image::Rgb([10, 240, 10]))
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
    let pixel = plain_libs::image::load_from_memory(&bytes)
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
    plain_libs::image::RgbImage::from_pixel(800, 400, plain_libs::image::Rgb([240, 10, 10]))
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
    plain_libs::image::RgbImage::from_pixel(900, 450, plain_libs::image::Rgb([10, 240, 10]))
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
        for (width, height) in [(80, 80), (80, 40), (40, 80)] {
            let path = directory
                .path()
                .join(format!("original-{width}-{height}.{extension}"));
            plain_libs::image::RgbImage::from_pixel(width, height, plain_libs::image::Rgb([200, 40, 50]))
                .save(&path)
                .unwrap();
            for center_crop in [true, false] {
                let mut request = request_for(&path);
                request.width = 96;
                request.height = 96;
                request.center_crop = center_crop;
                let bytes = state
                    .thumbnails
                    .get(&state, request)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(bytes, std::fs::read(&path).unwrap());
                let image = plain_libs::image::load_from_memory(&bytes).unwrap();
                assert_eq!((image.width(), image.height()), (width, height));
            }
        }
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
    plain_libs::image::RgbImage::from_pixel(800, 400, plain_libs::image::Rgb([200, 40, 50]))
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

#[tokio::test]
async fn startup_trims_the_shared_thumbnail_directory_to_eighty_percent() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let cache = directory.path().join("cache");
    let thumbs = cache.join("thumbs");
    std::fs::create_dir_all(&thumbs).unwrap();
    let mut files = Vec::new();
    for (index, size) in [150_000_000, 150_000_000, 750_000_000].into_iter().enumerate() {
        let path = thumbs.join(format!("{index:064x}.jpg"));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(size).unwrap();
        file.set_times(std::fs::FileTimes::new().set_modified(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000 + index as u64),
        )).unwrap();
        files.push(path);
    }
    let other = cache.join("not-a-thumbnail.jpg");
    std::fs::write(&other, b"leave separate caches alone").unwrap();
    let core = ContentServer::open_with_cache(&directory.path().join("data"), &cache).unwrap();

    let trimmed = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while files[0].exists() || files[1].exists() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await;
    core.stop_core().await;
    assert!(trimmed.is_ok(), "the cache must remove its two oldest thumbnails");
    assert_eq!(std::fs::metadata(&files[2]).unwrap().len(), 750_000_000);
    assert_eq!(std::fs::read(&other).unwrap(), b"leave separate caches alone");
}

#[cfg(feature = "media")]
#[tokio::test]
async fn full_disk_cache_still_returns_the_thumbnail_and_cleans_in_the_background() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let cache = directory.path().join("cache");
    let thumbs = cache.join("thumbs");
    std::fs::create_dir_all(&thumbs).unwrap();
    let old = thumbs.join("old.jpg");
    std::fs::File::create(&old).unwrap().set_len(super::disk_cache::LIMIT).unwrap();
    let input = directory.path().join("source.png");
    plain_libs::image::RgbImage::from_pixel(640, 480, plain_libs::image::Rgb([80, 60, 100]))
        .save(&input).unwrap();
    let core = ContentServer::open_with_cache(&directory.path().join("data"), &cache).unwrap();
    let state = core.runtime_state();

    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while state.thumbnails.disk_cache.total() != super::disk_cache::LIMIT {
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    let bytes = state.thumbnails.get(&state, request_for(&input)).await.unwrap().unwrap();
    assert!(plain_libs::image::load_from_memory(&bytes).is_ok());
    assert_eq!(state.thumbnails.cache_bytes_written.load(std::sync::atomic::Ordering::Relaxed), 0);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while old.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    core.stop_core().await;
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
#[tokio::test]
async fn audio_cover_writes_only_the_shared_thumbnail_cache() {
    let (directory, core, _) = server_with_cache();
    let state = core.runtime_state();
    let path = directory.path().join("song.mp3");
    std::fs::write(&path, b"sidecar cover fixture").unwrap();
    plain_libs::image::RgbImage::from_pixel(400, 300, plain_libs::image::Rgb([80, 60, 100]))
        .save(directory.path().join("cover.jpg"))
        .unwrap();

    let first = state.thumbnails.get(&state, request_for(&path)).await.unwrap().unwrap();
    let cover = plain_libs::image::load_from_memory(&first).unwrap();
    assert_eq!((cover.width(), cover.height()), (256, 256));
    let thumbs = state.file_platform.thumbnail_cache(path.to_str().unwrap()).unwrap().join("thumbs");
    let entries = std::fs::read_dir(&thumbs).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(entries.len(), 1, "audio covers must not create a second cache tree");
    let cached_file = entries[0].path();
    assert_eq!(cached_file.extension().unwrap(), "jpg");
    assert_eq!(cached_file.file_stem().unwrap().len(), 64);
    assert_eq!(std::fs::read(cached_file).unwrap(), first.as_ref());

    state.thumbnails.hot.clear();
    let second = state.thumbnails.get(&state, request_for(&path)).await.unwrap().unwrap();
    assert_eq!(first, second);
    assert_eq!(state.thumbnails.native_decodes.load(std::sync::atomic::Ordering::Relaxed), 1);
    core.stop_core().await;
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
#[tokio::test]
async fn video_thumbnail_batch_decodes_and_persists_once_per_file() {
    use std::sync::atomic::Ordering;

    let (directory, core, _) = server_with_cache();
    let state = core.runtime_state();
    let paths = video_thumbnail_fixtures(directory.path());
    let (first, concurrent) = tokio::join!(
        video_thumbnail_batch(&state, &paths),
        video_thumbnail_batch(&state, &paths)
    );
    let reads = state.thumbnails.cache_reads.load(Ordering::Relaxed);
    let cached = video_thumbnail_batch(&state, &paths).await;

    let mut jpeg_bytes = 0;
    for ((first, concurrent), cached) in first.iter().zip(concurrent).zip(cached) {
        assert_eq!(first.as_ptr(), concurrent.as_ptr());
        assert_eq!(first.as_ptr(), cached.as_ptr());
        let image = plain_libs::image::load_from_memory(first).unwrap();
        assert_eq!((image.width(), image.height()), (256, 256));
        jpeg_bytes += first.len() as u64;
    }
    assert_eq!(first.len(), paths.len());
    assert_eq!(
        state.thumbnails.native_decodes.load(Ordering::Relaxed),
        paths.len()
    );
    assert_eq!(state.thumbnails.cache_reads.load(Ordering::Relaxed), reads);
    let cache = state
        .file_platform
        .thumbnail_cache(paths[0].to_str().unwrap())
        .unwrap();
    assert_eq!(
        video_cache_entries(&cache.join("thumbs")),
        (paths.len(), jpeg_bytes)
    );
    core.stop_core().await;
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
fn video_thumbnail_fixtures(directory: &std::path::Path) -> Vec<std::path::PathBuf> {
    let fixtures: [&[u8]; 3] = [
        include_bytes!("../../../../../../plain-rs/testdata/video-h265.mp4"),
        include_bytes!("../../../../../../plain-rs/testdata/video-h265-hvc1.mp4"),
        include_bytes!("../../../../../../plain-rs/testdata/video-h264-high.mp4"),
    ];
    (0..12)
        .map(|i| {
            let path = directory.join(format!("video-{i}.mp4"));
            std::fs::write(&path, fixtures[i % fixtures.len()]).unwrap();
            path
        })
        .collect()
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
async fn video_thumbnail_batch(
    state: &crate::runtime::Runtime,
    paths: &[std::path::PathBuf],
) -> Vec<bytes::Bytes> {
    let jobs: Vec<_> = paths
        .iter()
        .map(|path| {
            let state = state.clone();
            let request = request_for(path);
            tokio::spawn(async move {
                state
                    .thumbnails
                    .get(&state, request)
                    .await
                    .unwrap()
                    .unwrap()
            })
        })
        .collect();
    let mut result = Vec::with_capacity(jobs.len());
    for job in jobs {
        result.push(job.await.unwrap());
    }
    result
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
fn video_cache_entries(directory: &std::path::Path) -> (usize, u64) {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();
            if metadata.is_dir() {
                video_cache_entries(&entry.path())
            } else {
                (1, metadata.len())
            }
        })
        .fold((0, 0), |(count, size), (files, bytes)| {
            (count + files, size + bytes)
        })
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn video_thumbnail_batch_benchmark() {
    use std::time::Instant;
    let mut cold = Vec::new();
    let mut warm = Vec::new();
    let mut cache_entries = (0, 0);
    for round in 0..8 {
        let (directory, core, _) = server_with_cache();
        let state = core.runtime_state();
        let paths = video_thumbnail_fixtures(directory.path());
        let mut timings = [0.; 2];
        for elapsed in &mut timings {
            let start = Instant::now();
            let results = video_thumbnail_batch(&state, &paths).await;
            assert!(results.iter().all(|bytes| bytes.starts_with(&[0xff, 0xd8])));
            *elapsed = start.elapsed().as_secs_f64() * 1000.;
        }
        let cache = state
            .file_platform
            .thumbnail_cache(paths[0].to_str().unwrap())
            .unwrap();
        cache_entries = video_cache_entries(&cache.join("thumbs"));
        if round > 0 {
            cold.push(timings[0]);
            warm.push(timings[1]);
        }
        core.stop_core().await;
    }
    cold.sort_by(f64::total_cmp);
    warm.sort_by(f64::total_cmp);
    println!(
        "batch=12 rounds=7 cold_median_ms={:.3} warm_median_ms={:.3} cache_files={} cache_bytes={}",
        cold[3], warm[3], cache_entries.0, cache_entries.1
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn thumbnail_sizes_share_256_bytes_and_preserve_large_previews() {
    use std::sync::atomic::Ordering;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let cache = directory.path().join("cache");
    let core = ContentServer::open_with_cache(&directory.path().join("data"), &cache).unwrap();
    let state = core.runtime_state();
    let path = directory.path().join("sizes.jpg");
    plain_libs::image::RgbImage::from_fn(1600, 1200, |x, y| {
        plain_libs::image::Rgb([(x % 251) as u8, (y % 239) as u8, 90])
    })
    .save(&path)
    .unwrap();
    let expected =
        plain_libs::media::thumbnails::render(path.to_str().unwrap(), 256, 256, true, None)
            .await
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
    let mut first = None;
    while let Some(result) = requests.join_next().await {
        let bytes = result.unwrap().unwrap().unwrap();
        assert_eq!(bytes, expected);
        first.get_or_insert(bytes);
    }
    let alternate = state
        .thumbnails
        .get(
            &state,
            Request {
                width: 256,
                height: 256,
                center_crop: false,
                short_edge: None,
                ..request_for(&path)
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(alternate.as_ptr(), first.unwrap().as_ptr());
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 1);
    assert_eq!(
        state.thumbnails.cache_bytes_written.load(Ordering::Relaxed),
        expected.len() as u64
    );
    let entries: Vec<_> = std::fs::read_dir(cache.join("thumbs"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert!(entries[0].is_file());
    for (width, height, crop, dimensions) in [
        (96, 96, false, (256, 192)),
        (96, 96, true, (256, 192)),
        (1024, 1024, false, (1024, 768)),
        (1024, 1024, true, (1024, 768)),
        (512, 1024, false, (512, 384)),
        (512, 1024, true, (512, 384)),
        (1024, 512, false, (682, 512)),
        (1024, 512, true, (682, 512)),
    ] {
        let request = Request {
            width,
            height,
            center_crop: crop,
            short_edge: None,
            ..request_for(&path)
        };
        let bytes = state
            .thumbnails
            .get(&state, request)
            .await
            .unwrap()
            .unwrap();
        let image = plain_libs::image::load_from_memory(&bytes).unwrap();
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
async fn real_image_extensions_skip_old_thumbnails_and_use_current_cache_keys() {
    let (directory, core, _) = server_with_cache();
    let state = core.runtime_state();
    let source = directory.path().join("alpha.PNG");
    plain_libs::image::RgbaImage::from_pixel(512, 512, plain_libs::image::Rgba([40, 90, 150, 0]))
        .save(&source)
        .unwrap();
    let source = source.canonicalize().unwrap();
    let request = Request {
        width: 256,
        height: 256,
        ..request_for(&source)
    };
    let source_key = |request: &Request, version: &str| {
        let metadata = std::fs::metadata(&request.path).unwrap();
        let input = plain_libs::serde_json::json!([
            request.path,
            metadata.len(),
            format!("{:?}", metadata.modified().unwrap()),
            request.width,
            request.height,
            request.center_crop,
            "",
            "",
            version
        ])
        .to_string();
        plain_libs::utils::hex::bytes_to_hex(&sha2::Sha256::digest(input.as_bytes()))
    };
    let old_key = source_key(&request, "thumbnail-v3");
    assert_ne!(
        super::source_fingerprint(&request).unwrap().unwrap(),
        old_key
    );
    let old = state
        .file_platform
        .thumbnail_cache(&request.path)
        .unwrap()
        .join("thumbs")
        .join(format!("{old_key}.png"));
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
            center_crop: false,
            short_edge: None,
            ..request_for(&source)
        })
        .unwrap()
        .unwrap()
    ));
    assert_eq!(std::fs::read(&generated).unwrap(), bytes);
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(
        plain_libs::image::load_from_memory(&bytes)
            .unwrap()
            .into_rgba8()
            .get_pixel(128, 128)[3],
        0
    );
    let jpeg = directory.path().join("photo.jpg");
    plain_libs::image::RgbImage::from_pixel(512, 512, plain_libs::image::Rgb([40, 90, 150]))
        .save(&jpeg)
        .unwrap();
    let request = Request {
        width: 256,
        height: 256,
        ..request_for(&jpeg.canonicalize().unwrap())
    };
    assert_eq!(
        super::source_fingerprint(&request).unwrap().unwrap(),
        source_key(&request, "thumbnail-v4")
    );
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[test]
fn cached_sdk_png_uses_its_real_extension_and_hot_hits_do_not_read_disk() {
    use std::sync::atomic::Ordering;
    let directory = plain_libs::temporary::tempdir().unwrap();
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
    plain_libs::image::RgbImage::from_pixel(640, 480, plain_libs::image::Rgb([40, 90, 150]))
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
