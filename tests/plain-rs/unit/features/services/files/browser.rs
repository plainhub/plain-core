use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    [
        ((payload.len() + 8) as u32).to_be_bytes().as_slice(),
        kind,
        payload,
    ]
    .concat()
}
fn movie(codec: &[u8; 4], duration: u32, metadata: bool) -> Vec<u8> {
    let mut time = vec![0; 12];
    time.extend(1000u32.to_be_bytes());
    time.extend(duration.to_be_bytes());
    let track = |codec: &[u8; 4], handler: &[u8; 4]| {
        let mut header = vec![0; 16];
        header[12..16].copy_from_slice(&if handler == b"vide" { 1u32 } else { 2u32 }.to_be_bytes());
        let mut hdlr = vec![0; 8];
        hdlr.extend(handler);
        let stsd = atom(
            b"stsd",
            &[vec![0, 0, 0, 0, 0, 0, 0, 1], atom(codec, &[])].concat(),
        );
        atom(
            b"trak",
            &[
                atom(b"tkhd", &header),
                atom(
                    b"mdia",
                    &[atom(b"hdlr", &hdlr), atom(b"minf", &atom(b"stbl", &stsd))].concat(),
                ),
            ]
            .concat(),
        )
    };
    let mut tracks = vec![atom(b"mvhd", &time), track(codec, b"vide")];
    if metadata {
        tracks.push(track(b"mebx", b"meta"));
    }
    atom(b"moov", &tracks.concat())
}
async fn supported() -> Result<Vec<String>, String> {
    Ok(vec!["VIDEO_REMUX".into(), "VIDEO_TRANSCODE".into()])
}
#[test]
fn cache_identity_includes_operation_size_and_full_modification_time() {
    let root = Path::new("/cache");
    let source = Source {
        size: 100,
        modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_nanos(100),
    };
    let first = cache_path(root, "/video", &source, Operation::Transcode);
    assert_eq!(
        first,
        cache_path(root, "/video", &source, Operation::Transcode)
    );
    assert_ne!(first, cache_path(root, "/video", &source, Operation::Remux));
    let changed = Source {
        size: 100,
        modified: source.modified + std::time::Duration::from_nanos(1),
    };
    assert_ne!(
        first,
        cache_path(root, "/video", &changed, Operation::Transcode)
    );
    assert_ne!(
        first,
        cache_path(root, "/other", &source, Operation::Transcode)
    );
}
#[tokio::test]
async fn ordinary_files_and_long_videos_do_not_probe_capabilities_or_encode() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let runtime = Runtime::new(dir.path().join("core/cache"));
    let file = dir.path().join("source.mp4");
    for (codec, duration, operation) in [
        (*b"avc1", 3000, Operation::Transcode),
        (*b"avc1", 3000, Operation::Remux),
        (*b"hvc1", 600_001, Operation::Transcode),
    ] {
        std::fs::write(&file, movie(&codec, duration, false)).unwrap();
        assert!(
            runtime
                .convert_with(
                    file.to_str().unwrap(),
                    operation,
                    || async { panic!("unexpected capability lookup") },
                    |_, _| async { panic!("unexpected encoder") }
                )
                .await
                .unwrap()
                .is_none()
        );
    }
}
#[tokio::test]
async fn missing_capability_never_enters_the_encoder() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let file = dir.path().join("source.mp4");
    std::fs::write(&file, movie(b"hvc1", 3000, false)).unwrap();
    let runtime = Runtime::new(dir.path().join("core/cache"));
    assert!(
        runtime
            .convert_with(
                file.to_str().unwrap(),
                Operation::Transcode,
                || async { Ok(Vec::new()) },
                |_, _| async { panic!("unsupported encoder") }
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(!dir.path().join("core/cache").exists());
}
#[tokio::test]
async fn concurrent_requests_encode_once_and_reuse_the_same_committed_bytes() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let file = dir.path().join("source.mp4");
    std::fs::write(&file, movie(b"hvc1", 3000, false)).unwrap();
    let runtime = Runtime::new(dir.path().join("core/cache"));
    let calls = AtomicUsize::new(0);
    let encode = |_: PathBuf, output: Arc<Output>| {
        let calls = &calls;
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::fs::write(&output.0, b"encoded bytes").await.unwrap();
            Ok(true)
        }
    };
    let (first, second) = tokio::join!(
        runtime.convert_with(
            file.to_str().unwrap(),
            Operation::Transcode,
            supported,
            encode
        ),
        runtime.convert_with(
            file.to_str().unwrap(),
            Operation::Transcode,
            supported,
            encode
        )
    );
    let first = first.unwrap().unwrap();
    assert_eq!(Some(&first), second.unwrap().as_ref());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(std::fs::read(first).unwrap(), b"encoded bytes");
}
#[tokio::test]
async fn remux_drops_metadata_by_selecting_the_remux_primitive_once() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let file = dir.path().join("source.mp4");
    std::fs::write(&file, movie(b"avc1", 3000, true)).unwrap();
    let runtime = Runtime::new(dir.path().join("core/cache"));
    let output = runtime
        .convert_with(
            file.to_str().unwrap(),
            Operation::Remux,
            supported,
            |input, output| async move {
                assert_eq!(input.file_name().unwrap(), "source.mp4");
                tokio::fs::write(&output.0, b"remux").await.unwrap();
                Ok(true)
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(output).unwrap(), b"remux");
}

#[tokio::test]
async fn metadata_removal_uses_shared_rust_without_a_platform_encoder() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let input = directory.path().join("metadata.mp4");
    let source = movie(b"avc1", 3000, true);
    std::fs::write(&input, &source).unwrap();
    let runtime = Runtime::new(directory.path().join("cache"));
    let output = runtime
        .convert(input.to_str().unwrap(), Operation::Remux)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !plain_libs::media::isobmff::tracks(&output)
            .unwrap()
            .needs_remux
    );
    assert_eq!(std::fs::read(&input).unwrap(), source);
    assert_eq!(
        std::fs::metadata(&output).unwrap().len(),
        source.len() as u64
    );
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[test]
fn remux_support_does_not_depend_on_an_installed_encoder() {
    assert_eq!(declared_capabilities(false), vec!["VIDEO_REMUX"]);
    assert_eq!(
        declared_capabilities(true),
        vec!["VIDEO_REMUX", "VIDEO_TRANSCODE"]
    );
}

#[tokio::test]
async fn cancellation_retains_the_temporary_output_until_the_worker_exits() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("owned.mp4");
    std::fs::write(&path, b"reserved").unwrap();
    let output = Arc::new(Output(path.clone()));
    let (entered, entry) = tokio::sync::oneshot::channel();
    let (release, resume) = std::sync::mpsc::channel();
    let (finished, exit) = tokio::sync::oneshot::channel();
    let worker = tokio::spawn(media_task(move |cancelled| {
        entered.send(()).unwrap();
        resume
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(cancelled.load(Ordering::Acquire));
        std::fs::write(&output.0, b"last native write")?;
        drop(output);
        finished.send(()).unwrap();
        Ok(true)
    }));
    entry.await.unwrap();
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    assert!(path.exists());
    release.send(()).unwrap();
    exit.await.unwrap();
    assert!(!path.exists());
}
#[tokio::test]
async fn replaced_sources_and_encoder_failures_never_publish_partial_cache_entries() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let file = dir.path().join("source.mp4");
    let data = movie(b"hvc1", 3000, false);
    std::fs::write(&file, &data).unwrap();
    let runtime = Runtime::new(dir.path().join("core/cache"));
    let failed = runtime
        .convert_with(
            file.to_str().unwrap(),
            Operation::Transcode,
            supported,
            |input, output| async move {
                tokio::fs::write(input, b"replaced").await.unwrap();
                tokio::fs::write(&output.0, b"partial").await.unwrap();
                Ok(true)
            },
        )
        .await;
    assert!(failed.unwrap_err().contains("Source changed"));
    std::fs::write(&file, &data).unwrap();
    let failed = runtime
        .convert_with(
            file.to_str().unwrap(),
            Operation::Transcode,
            supported,
            |_, output| async move {
                tokio::fs::write(&output.0, b"partial").await.unwrap();
                Err("synthetic failure".into())
            },
        )
        .await;
    assert_eq!(failed.unwrap_err(), "synthetic failure");
    let output = runtime
        .convert_with(
            file.to_str().unwrap(),
            Operation::Transcode,
            supported,
            |_, output| async move {
                tokio::fs::write(&output.0, b"complete").await.unwrap();
                Ok(true)
            },
        )
        .await
        .unwrap()
        .unwrap();
    let siblings = std::fs::read_dir(output.parent().unwrap()).unwrap().count();
    assert_eq!(siblings, 1);
    assert_eq!(std::fs::read(output).unwrap(), b"complete");
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[tokio::test]
async fn native_hev1_conversion_caches_the_result_and_cleans_intermediates() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let source = directory.path().join("source.mp4");
    let bytes = include_bytes!("../../../../../../plain-rs/testdata/video-h265.mp4");
    std::fs::write(&source, bytes).unwrap();
    let runtime = Runtime::new(directory.path().join("cache"));
    let output = runtime.convert(source.to_str().unwrap(), Operation::Transcode).await.unwrap().unwrap();
    assert_eq!(plain_libs::media::isobmff::tracks(&output).unwrap().first_video_codec.as_deref(), Some("avc1"));
    let modified = std::fs::metadata(&output).unwrap().modified().unwrap();
    assert_eq!(runtime.convert(source.to_str().unwrap(), Operation::Transcode).await.unwrap(), Some(output.clone()));
    assert_eq!(std::fs::metadata(&output).unwrap().modified().unwrap(), modified);
    assert_eq!(std::fs::read(source).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(output.parent().unwrap()).unwrap().count(), 1);
}
