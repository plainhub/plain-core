use super::*;
use crate::features::enums::DataType;

#[test]
fn cache_trimming_preserves_non_thumbnail_entries() {
    let directory = tempfile::tempdir().unwrap();
    let old = directory.path().join("old.thumb");
    std::fs::File::create(&old)
        .unwrap()
        .set_len(257 * 1024 * 1024)
        .unwrap();
    let original = directory.path().join("original.txt");
    std::fs::write(&original, b"original").unwrap();
    trim_cache(directory.path()).unwrap();
    assert!(!old.exists());
    assert_eq!(std::fs::read(original).unwrap(), b"original");
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn index_updates_prewarm_the_same_cache_used_by_foreground_requests() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.jpg");
    image::RgbImage::from_pixel(800, 600, image::Rgb([220, 40, 30]))
        .save(&path)
        .unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let unrelated = directory.path().join("untouched.jpg");
    std::fs::copy(&path, &unrelated).unwrap();
    state
        .db
        .record_file_paths(
            &[unrelated.to_string_lossy().into_owned()],
            Some(FileKind::Image),
        )
        .unwrap();
    let catalog =
        crate::features::services::files::catalog_scan::NativeMediaCatalog::new(state.clone());
    catalog
        .scan_file(DataType::Image, path.to_str().unwrap())
        .await
        .unwrap();
    let request = Request {
        path: path.to_str().unwrap().into(),
        file_id: "different-caller-id".into(),
        file_name: "caller-title.jpg".into(),
        width: 512,
        height: 512,
        center_crop: true,
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst)
            < 2
        {
            tokio::task::yield_now().await;
        }
        while state.thumbnails.queue.waiting() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let before = state
        .thumbnails
        .native_decodes
        .load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(before, 2, "Only the committed source should be decoded");
    let written = state.thumbnails.cache_bytes_written.load(Ordering::Relaxed);
    let mut output_bytes = 0;
    for size in [96, 256] {
        let bytes = state
            .thumbnails
            .get(
                &state,
                Request {
                    width: size,
                    height: size,
                    ..request.clone()
                },
            )
            .await
            .unwrap()
            .unwrap();
        output_bytes += bytes.len() as u64;
        assert_eq!(image::load_from_memory(&bytes).unwrap().width(), size);
        assert_eq!(
            state
                .thumbnails
                .native_decodes
                .load(std::sync::atomic::Ordering::SeqCst),
            before
        );
    }
    assert_eq!(written, output_bytes);
    assert_eq!(
        state.thumbnails.cache_bytes_written.load(Ordering::Relaxed),
        written
    );
    core.stop_core().await;
}

#[cfg(unix)]
#[test]
fn cache_trimming_refuses_a_symlink_root_without_touching_its_target() {
    let cache = tempfile::tempdir().unwrap();
    let original = tempfile::tempdir().unwrap();
    let path = original.path().join("photo.jpg");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(257 * 1024 * 1024)
        .unwrap();
    let link = cache.path().join("thumbs");
    std::os::unix::fs::symlink(original.path(), &link).unwrap();
    assert_eq!(
        trim_cache(&link).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(std::fs::metadata(path).unwrap().len(), 257 * 1024 * 1024);
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn foreground_decode_can_start_while_background_decoders_are_occupied() {
    use std::sync::{Arc, atomic::Ordering};
    let dir = tempfile::Builder::new()
        .prefix("thumbnail-priority-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let core = crate::test_runtime::Core::open(&dir.path().join("core")).unwrap();
    let state = core.runtime_state();
    let workers =
        plain_libs::media::thumbnail_limits::background_workers(state.thumbnails.workers());
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *state.thumbnails.decode_gate.lock().unwrap() = Some(gate.clone());
    let mut paths = Vec::new();
    for n in 0..=workers {
        let path = dir.path().join(format!("source-{n}.jpg"));
        image::RgbImage::from_pixel(800, 600, image::Rgb([20, 100, 60]))
            .save(&path)
            .unwrap();
        paths.push(path.to_string_lossy().into_owned());
    }
    let metadata = paths[..workers]
        .iter()
        .map(|path| {
            crate::features::media::file_facts::read_file_metadata(
                std::path::Path::new(path),
                DataType::Image,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let rows = state.db.record_media_files(&metadata).unwrap();
    state
        .file_platform
        .thumbnail_warmup
        .enqueue(&rows, None)
        .await
        .unwrap();
    let wait_for = |count| {
        let state = state.clone();
        async move {
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                while state.thumbnails.native_decodes.load(Ordering::SeqCst) < count {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        }
    };
    wait_for(workers).await;
    let pending = (0..PENDING_LIMIT)
        .map(|index| {
            let mut row = source_row(&format!("pending-{index}"), "pending.jpg");
            row.path = paths[0].clone();
            row
        })
        .collect::<Vec<_>>();
    state
        .file_platform
        .thumbnail_warmup
        .enqueue(&pending, None)
        .await
        .unwrap();
    let extra = [source_row("extra", "extra.jpg")];
    let mut blocked = Box::pin(state.file_platform.thumbnail_warmup.enqueue(&extra, None));
    assert!(futures_util::poll!(&mut blocked).is_pending());
    let committed = state.file_platform.scan(&paths[..1]).await.unwrap();
    assert_eq!(committed.len(), 1);
    assert_eq!(state.file_platform.thumbnail_warmup.sender.capacity(), 0);
    assert!(futures_util::poll!(&mut blocked).is_pending());
    let request = Request {
        path: paths.pop().unwrap(),
        width: 256,
        height: 256,
        center_crop: true,
        file_id: String::new(),
        file_name: String::new(),
    };
    let foreground = tokio::spawn({
        let state = state.clone();
        async move { state.thumbnails.get(&state, request).await }
    });
    wait_for(workers + 1).await;
    gate.add_permits(2 * workers + 1);
    assert!(foreground.await.unwrap().unwrap().is_some());
    wait_for(2 * workers + 1).await;
    blocked.await.unwrap();
    core.stop_core().await;
}

fn source_row(id: &str, path: &str) -> MediaFileRow {
    MediaFileRow {
        id: id.into(),
        path: std::env::temp_dir()
            .join(path)
            .to_string_lossy()
            .into_owned(),
        data_type: DataType::Image.kind(),
        album_id: String::new(),
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        album_artist: String::new(),
        bucket_id: String::new(),
        size_bytes: 0,
        duration_ms: 0,
        cover_image_id: None,
        width: 0,
        height: 0,
        rotation: 0,
        taken_at: None,
        created_at: String::new(),
        updated_at: String::new(),
        favorited: false,
        trashed: false,
        original_path: None,
    }
}

#[tokio::test]
async fn full_queue_applies_backpressure_without_losing_sources() {
    let schedule = Schedule::default();
    let mut receiver = schedule.receiver.lock().unwrap().take().unwrap();
    let rows = (0..PENDING_LIMIT)
        .map(|id| source_row(&id.to_string(), "image.jpg"))
        .collect::<Vec<_>>();
    schedule.enqueue(&rows, None).await.unwrap();
    let extra = source_row("extra", "image.jpg");
    let mut send = Box::pin(schedule.enqueue(std::slice::from_ref(&extra), None));
    assert!(futures_util::poll!(send.as_mut()).is_pending());
    assert_eq!(receiver.recv().await.unwrap().id, "0");
    assert!(matches!(
        futures_util::poll!(send.as_mut()),
        std::task::Poll::Ready(Ok(()))
    ));
    for id in 1..PENDING_LIMIT {
        assert_eq!(receiver.recv().await.unwrap().id, id.to_string());
    }
    assert_eq!(receiver.recv().await.unwrap().id, "extra");
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn non_preview_changes_do_not_enter_the_queue() {
    let schedule = Schedule::default();
    let mut row = source_row("changed", "image.jpg");
    row.data_type = DataType::Doc.kind();
    schedule
        .enqueue(std::slice::from_ref(&row), None)
        .await
        .unwrap();
    row.data_type = DataType::Image.kind();
    row.trashed = true;
    schedule
        .enqueue(std::slice::from_ref(&row), None)
        .await
        .unwrap();
    row.trashed = false;
    row.path = "content://images/item".into();
    schedule
        .enqueue(std::slice::from_ref(&row), None)
        .await
        .unwrap();
    assert_eq!(schedule.sender.capacity(), PENDING_LIMIT);
}

#[tokio::test]
async fn changes_after_dispatch_remain_pending() {
    let schedule = Schedule::default();
    let mut receiver = schedule.receiver.lock().unwrap().take().unwrap();
    schedule
        .enqueue(&[source_row("changed", "old.jpg")], None)
        .await
        .unwrap();
    let running = receiver.recv().await.unwrap();
    schedule
        .enqueue(&[source_row("changed", "new.jpg")], None)
        .await
        .unwrap();
    assert!(std::path::Path::new(&running.path).ends_with("old.jpg"));
    assert!(std::path::Path::new(&receiver.recv().await.unwrap().path).ends_with("new.jpg"));
}

#[tokio::test]
async fn stopping_prewarm_releases_blocked_index_producers() {
    let schedule = Schedule::default();
    let receiver = schedule.receiver.lock().unwrap().take().unwrap();
    let rows = (0..PENDING_LIMIT)
        .map(|id| source_row(&id.to_string(), "image.jpg"))
        .collect::<Vec<_>>();
    schedule.enqueue(&rows, None).await.unwrap();
    let extra = source_row("extra", "image.jpg");
    let mut send = Box::pin(schedule.enqueue(std::slice::from_ref(&extra), None));
    assert!(futures_util::poll!(send.as_mut()).is_pending());
    drop(receiver);
    assert!(matches!(
        futures_util::poll!(send.as_mut()),
        std::task::Poll::Ready(Err(_))
    ));
}
