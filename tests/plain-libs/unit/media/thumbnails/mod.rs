use super::*;

fn cache_dir() -> std::path::PathBuf {
    static CACHE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            let directory = crate::temporary::tempdir().unwrap();
            let path = directory.path().to_path_buf();
            crate::test_tempdirs::retain(directory);
            path
        })
        .clone()
}

async fn thumbnail(spec: ThumbSpec) -> Result<ThumbOutcome> {
    super::get_thumbnail(spec, &cache_dir()).await
}

#[tokio::test]
async fn render_keeps_the_encoded_cache_allocation() {
    let dir = crate::temporary::tempdir().unwrap();
    let path = dir.path().join("shared.jpg");
    write_jpeg(&path, 640, 480);
    let first = render(path.to_str().unwrap(), 128, 128, true, dir.path())
        .await
        .unwrap()
        .unwrap();
    let second = render(path.to_str().unwrap(), 128, 128, false, dir.path())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.as_ptr(), second.as_ptr());
}

fn image_cache_path(
    root: &std::path::Path,
    path: &str,
    w: u32,
    h: u32,
    quality: u8,
    modified: i64,
    size: i64,
) -> std::path::PathBuf {
    thumb_cache_path(
        &root.join("thumbs/fit-v1-0"),
        path,
        w,
        h,
        quality,
        modified,
        size,
    )
}

fn write_jpeg(path: &std::path::Path, w: u32, h: u32) {
    let img = crate::image::RgbImage::from_fn(w, h, |x, y| {
        crate::image::Rgb([(x % 200) as u8, (y % 200) as u8, 90])
    });
    img.save_with_format(path, crate::image::ImageFormat::Jpeg)
        .unwrap();
}

fn spec_for(path: &std::path::Path, w: u32, h: u32) -> ThumbSpec {
    let meta = std::fs::metadata(path).unwrap();
    let mod_unix = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    ThumbSpec::sanitize(
        path.to_str().unwrap(),
        w as i32,
        h as i32,
        75,
        mod_unix,
        meta.len() as i64,
    )
}

fn fresh_dir(_tag: &str) -> crate::temporary::TempDir {
    // Under $HOME (macOS temp lives under /var/folders which the media
    // scan would exclude; irrelevant here, but consistent with the rest
    // of the suite).
    crate::temporary::tempdir().unwrap()
}

#[test]
fn compute_target_size_matrix() {
    // Box fit.
    let (tw, th) = compute_target_size(800, 600, 200, 200);
    assert_eq!((tw, th), (200, 150));
    // No upscale.
    assert_eq!(compute_target_size(100, 100, 200, 200), (100, 100));
    // Width-only.
    assert_eq!(compute_target_size(800, 600, 400, 0), (400, 300));
    // Height-only.
    assert_eq!(compute_target_size(800, 600, 0, 150), (200, 150));
    // Unconstrained.
    assert_eq!(compute_target_size(800, 600, 0, 0), (800, 600));
    // Extreme aspect: never collapses to zero.
    let (tw, th) = compute_target_size(10000, 2, 100, 100);
    assert_eq!((tw, th), (100, 1));
}

#[test]
fn jpeg_decode_strategy_matrix() {
    let prog = Sniffed {
        kind: ImageKind::Jpeg,
        width: 4000,
        height: 6000,
        progressive: true,
        preserve_alpha: false,
        orientation: 1,
        work_bytes: 0,
    };
    let base = Sniffed {
        progressive: false,
        preserve_alpha: false,
        orientation: 1,
        work_bytes: 0,
        ..prog
    };
    // Big-to-small: scaled decode (either-axis ratio >= 2).
    assert!(jpeg_use_scaled(4000, 3000, 256, 256));
    assert!(jpeg_use_scaled(4000, 3000, 2000, 1500));
    assert!(!jpeg_use_scaled(4000, 3000, 3000, 2000));
    // Progressive + 1/8 grid covers the actual target: DC-only fast path.
    // (A 512 box on 4000x6000 computes a 341x512 target; 1/8 = 500x750.)
    assert!(dc_fast_eligible(prog, 341, 512));
    assert!(dc_fast_eligible(prog, 128, 128));
    // Full 512x512 target exceeds the 1/8 width (500) → scaled decoder.
    assert!(!dc_fast_eligible(prog, 512, 512));
    // 1024px needs more than the 1/8 grid → scaled decoder.
    assert!(!dc_fast_eligible(prog, 1024, 1024));
    assert!(dc_fast_eligible(base, 128, 128));
    // Small progressive files (1/8 grid below target) neither.
    let small_prog = Sniffed {
        kind: ImageKind::Jpeg,
        width: 1536,
        height: 2288,
        progressive: true,
        preserve_alpha: false,
        orientation: 1,
        work_bytes: 0,
    };
    assert!(!dc_fast_eligible(small_prog, 512, 512)); // 192x286 < 512
    assert!(dc_fast_eligible(small_prog, 128, 128));
}

#[test]
fn spec_sanitize_clamps() {
    let s = ThumbSpec::sanitize("/x", -5, 9_999_999, 1000, 1, 2);
    assert_eq!((s.w, s.h, s.quality), (0, MAX_TARGET_DIM, 100));
}

#[test]
fn cache_path_determinism_and_invalidation() {
    let dir = std::path::Path::new("/tmp/cache");
    let a = thumb_cache_path(dir, "/a/b.jpg", 200, 200, 75, 1000, 5000);
    let b = thumb_cache_path(dir, "/a/b.jpg", 200, 200, 75, 1000, 5000);
    assert_eq!(a, b);
    assert!(a.to_str().unwrap().contains("thumbs/"));
    assert!(a.to_str().unwrap().ends_with(".jpg"));
    assert_ne!(
        a,
        thumb_cache_path(dir, "/a/b.jpg", 200, 200, 75, 1001, 5000)
    );
    assert_ne!(
        a,
        thumb_cache_path(dir, "/a/b.jpg", 256, 256, 75, 1000, 5000)
    );
    assert_eq!(a, dir.join("thumbs").join("71").join("71a2b618449793c9b1069e1339ebd8e7cb1c4ca69d6749ce3093c69e7e199d759c6c0fc2533775ce9af4b3619d8be971322c95126eeda5890ddad9f8b10a211f.jpg"));
    for changed in [
        thumb_cache_path(dir, "/other/b.jpg", 200, 200, 75, 1000, 5000),
        thumb_cache_path(dir, "/a/b.jpg", 200, 201, 75, 1000, 5000),
        thumb_cache_path(dir, "/a/b.jpg", 200, 200, 76, 1000, 5000),
        thumb_cache_path(dir, "/a/b.jpg", 200, 200, 75, 1000, 5001),
    ] {
        assert_ne!(a, changed);
    }
    let etag = cache_etag(&a);
    assert_eq!(
        etag,
        "\"t71a2b618449793c9b1069e1339ebd8e7cb1c4ca69d6749ce3093c69e7e199d759c6c0fc2533775ce9af4b3619d8be971322c95126eeda5890ddad9f8b10a211f\""
    );
    assert_eq!(etag, cache_etag(&b));
}

#[tokio::test]
async fn end_to_end_generate_then_cache_hit() {
    let dir = fresh_dir("e2e");
    let img = dir.path().join("photo.jpg");
    write_jpeg(&img, 640, 480);

    let spec = spec_for(&img, 128, 128);
    let out = thumbnail(spec.clone()).await.unwrap();
    let ThumbOutcome::Generated(d1) = out else {
        panic!("640x480 source must be generated, not passed through");
    };
    assert!(!d1.is_empty());
    assert!(d1.starts_with(&[0xFF, 0xD8]));
    // Cache file exists with identical bytes.
    let cp = image_cache_path(
        &cache_dir(),
        &spec.path,
        spec.w,
        spec.h,
        spec.quality,
        spec.mod_unix,
        spec.file_size,
    );
    assert!(cp.exists());
    assert_eq!(std::fs::read(&cp).unwrap(), d1.as_ref());

    // Second call: served from cache, byte-identical, no new generation.
    let key = cp.to_string_lossy().into_owned();
    let before = generations_for(&key);
    let out2 = thumbnail(spec.clone()).await.unwrap();
    let ThumbOutcome::Generated(d2) = out2 else {
        panic!("cached response must be Generated");
    };
    assert_eq!(d1, d2);
    assert_eq!(
        generations_for(&key),
        before,
        "cache hit must not regenerate"
    );
    std::fs::remove_file(&cp).ok();
}

#[tokio::test]
async fn concurrent_identical_requests_coalesce() {
    let dir = fresh_dir("coalesce");
    let img = dir.path().join("big.jpg");
    write_jpeg(&img, 1200, 900);
    let spec = spec_for(&img, 256, 256);

    let key = image_cache_path(
        &cache_dir(),
        &spec.path,
        spec.w,
        spec.h,
        spec.quality,
        spec.mod_unix,
        spec.file_size,
    )
    .to_string_lossy()
    .into_owned();
    let before = generations_for(&key);
    let mut handles = Vec::new();
    for _ in 0..32 {
        let s = spec.clone();
        handles.push(tokio::spawn(async move { thumbnail(s).await.unwrap() }));
    }
    for h in handles {
        let ThumbOutcome::Generated(d) = h.await.unwrap() else {
            panic!("expected Generated");
        };
        assert!(!d.is_empty());
    }
    let after = generations_for(&key);
    assert!(
        after <= before + 1,
        "32 identical requests produced {} generations",
        after - before
    );
    // Cleanup for other tests.
    let cp = image_cache_path(
        &cache_dir(),
        &spec.path,
        spec.w,
        spec.h,
        spec.quality,
        spec.mod_unix,
        spec.file_size,
    );
    std::fs::remove_file(&cp).ok();
}

#[tokio::test]
async fn passthrough_small_images() {
    let dir = fresh_dir("pass");
    // Small JPEG (fits box): bytes served as-is.
    let img = dir.path().join("small.jpg");
    write_jpeg(&img, 64, 48);
    let out = thumbnail(spec_for(&img, 256, 256)).await.unwrap();
    match out {
        ThumbOutcome::Original { data, mime } => {
            assert_eq!(mime, "image/jpeg");
            assert_eq!(data.as_ref(), std::fs::read(&img).unwrap());
        }
        _ => panic!("small jpeg must pass through"),
    }

    // Small PNG with alpha also passes through (alpha preserved).
    let png = dir.path().join("small.png");
    crate::image::RgbaImage::from_fn(32, 32, |_, _| crate::image::Rgba([1, 2, 3, 0]))
        .save_with_format(&png, crate::image::ImageFormat::Png)
        .unwrap();
    match thumbnail(spec_for(&png, 64, 64)).await.unwrap() {
        ThumbOutcome::Original { data, mime } => {
            assert_eq!(mime, "image/png");
            assert_eq!(data.as_ref(), std::fs::read(&png).unwrap());
        }
        _ => panic!("small png must pass through"),
    }

    // No target dims + small file: also passthrough (cc=1&w=0&h=0).
    match thumbnail(spec_for(&img, 0, 0)).await.unwrap() {
        ThumbOutcome::Original { .. } => {}
        _ => panic!("no-dim request on small file must pass through"),
    }
}

#[tokio::test]
async fn exif_oriented_thumbnail_dimensions() {
    let dir = fresh_dir("exif");
    // 400x300 landscape stored with orientation 6 (rotate 90 CW to view):
    // upright is 300x400. Target 100x100 → upright thumb 75x100.
    let img = dir.path().join("rot6.jpg");
    write_jpeg(&img, 400, 300);
    let mut bytes = std::fs::read(&img).unwrap();
    let app1 = exif::test_app1_orientation(6);
    let mut with_exif = bytes[..2].to_vec();
    with_exif.extend_from_slice(&app1);
    with_exif.extend_from_slice(&bytes[2..]);
    bytes = with_exif;
    std::fs::write(&img, &bytes).unwrap();

    let spec = spec_for(&img, 100, 100);
    let out = thumbnail(spec.clone()).await.unwrap();
    let ThumbOutcome::Generated(d) = out else {
        panic!("must generate");
    };
    let decoded = crate::image::load_from_memory(&d).unwrap();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (75, 100),
        "orientation-6 thumb must be portrait 75x100"
    );
    let cp = image_cache_path(
        &cache_dir(),
        &spec.path,
        spec.w,
        spec.h,
        spec.quality,
        spec.mod_unix,
        spec.file_size,
    );
    std::fs::remove_file(&cp).ok();
}

#[tokio::test]
async fn unicode_and_spaced_paths() {
    let dir = fresh_dir("paths");
    let sub = dir.path().join("ünïcode 空 目録");
    std::fs::create_dir_all(&sub).unwrap();
    let img = sub.join("photo name #1.JPG");
    write_jpeg(&img, 320, 240);
    let out = thumbnail(spec_for(&img, 64, 64)).await.unwrap();
    assert!(matches!(out, ThumbOutcome::Generated(_)));
    let img2 = dir.path().join("Ünicode 名前 test.jpg");
    write_jpeg(&img2, 320, 240);
    assert!(matches!(
        thumbnail(spec_for(&img2, 64, 64)).await.unwrap(),
        ThumbOutcome::Generated(_)
    ));
    // Cleanup: engine writes to the real cache dir.
    for p in [&img, &img2] {
        let s = spec_for(p, 64, 64);
        let cp = image_cache_path(
            &cache_dir(),
            &s.path,
            s.w,
            s.h,
            s.quality,
            s.mod_unix,
            s.file_size,
        );
        std::fs::remove_file(&cp).ok();
    }
}

#[tokio::test]
async fn bomb_header_rejected() {
    let dir = fresh_dir("bomb");
    let fake = dir.path().join("bomb.jpg");
    // A JPEG header claiming 40000x40000 with garbage payload.
    let mut v = vec![0xFF, 0xD8];
    v.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
    v.extend_from_slice(&40000u16.to_be_bytes());
    v.extend_from_slice(&40000u16.to_be_bytes());
    v.extend_from_slice(&[3, 1, 0x11, 1, 0x11, 1, 0x11]);
    v.extend_from_slice(&[0x55; 64]);
    std::fs::write(&fake, &v).unwrap();
    // Not JPEG-scalable-check first: sniff OK → guard trips (1.6G px
    // bitmap for a full decode path... ratio≥2 so scaled path is chosen
    // and est is small; decode then fails on garbage → Err either way).
    let r = thumbnail(spec_for(&fake, 64, 64)).await;
    assert!(r.is_err(), "garbage payload must not yield a thumbnail");

    // True bomb: PNG claiming 50000x50000 (full-decode path).
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&13u32.to_be_bytes());
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&50000u32.to_be_bytes());
    png.extend_from_slice(&50000u32.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0]);
    png.extend_from_slice(&[0; 16]);
    let fakepng = dir.path().join("bomb.png");
    std::fs::write(&fakepng, &png).unwrap();
    let err = thumbnail(spec_for(&fakepng, 64, 64))
        .await
        .expect_err("50000x50000 PNG must be rejected by the pixel guard");
    assert!(err.to_string().contains("pixel guard"), "got: {err}");
}

#[tokio::test]
async fn corrupt_and_foreign_files_error() {
    let dir = fresh_dir("err");
    // Garbage with a non-image extension.
    let txt = dir.path().join("note.txt");
    std::fs::write(&txt, b"hello world 1234").unwrap();
    assert!(thumbnail(spec_for(&txt, 64, 64)).await.is_err());
    // Empty file with .jpg extension.
    let empty = dir.path().join("empty.jpg");
    std::fs::write(&empty, b"").unwrap();
    assert!(thumbnail(spec_for(&empty, 64, 64)).await.is_err());
    // Truncated real JPEG (header ok, body cut): decoders are lenient —
    // zune fills the missing scan data, so a thumbnail may still come
    // out (same behavior as the previous pipeline). The contract is
    // "never panic, output is always a decodable JPEG when Ok".
    let trunc = dir.path().join("trunc.jpg");
    write_jpeg(&trunc, 200, 150);
    let bytes = std::fs::read(&trunc).unwrap();
    std::fs::write(&trunc, &bytes[..bytes.len() / 3]).unwrap();
    if let ThumbOutcome::Generated(d) = thumbnail(spec_for(&trunc, 64, 64)).await.unwrap() {
        assert!(crate::image::load_from_memory(&d).is_ok(), "output must decode");
    }
}

#[tokio::test]
async fn cover_sidecar_thumbnail() {
    let dir = fresh_dir("cover");
    // song.mp3 (garbage body ok — sidecar wins before lofty parses) +
    // cover.jpg next to it.
    let song = dir.path().join("song.mp3");
    std::fs::write(&song, b"not really an mp3").unwrap();
    let cover = dir.path().join("cover.jpg");
    write_jpeg(&cover, 500, 400);
    let spec = spec_for(&song, 128, 128);
    let out = thumbnail(spec.clone()).await.unwrap();
    let ThumbOutcome::Generated(d) = out else {
        panic!("sidecar cover must generate");
    };
    assert!(d.starts_with(&[0xFF, 0xD8]));
    let cp = thumb_cache_path(
        &cache_dir(),
        &spec.path,
        spec.w,
        spec.h,
        spec.quality,
        spec.mod_unix,
        spec.file_size,
    );
    assert!(cp.exists(), "cover thumb must be cached");
    std::fs::remove_file(&cp).ok();
}

#[tokio::test]
async fn progressive_jpeg_dc_fast_path_end_to_end() {
    let p = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../plain-rs/testdata"))
        .join("v-synthetic-prog.jpg");
    let spec = spec_for(&p, 128, 128);
    let out = thumbnail(spec.clone()).await.unwrap();
    let ThumbOutcome::Generated(d) = out else {
        panic!("progressive JPEG must generate");
    };
    let decoded = crate::image::load_from_memory(&d).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (128, 85));
    // Cleanup.
    let cp = image_cache_path(
        &cache_dir(),
        &spec.path,
        spec.w,
        spec.h,
        spec.quality,
        spec.mod_unix,
        spec.file_size,
    );
    std::fs::remove_file(&cp).ok();
}

#[tokio::test]
async fn video_thumbnail_end_to_end_and_cache_hit() {
    let mp4 =
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../plain-rs/testdata"))
            .join("video-h264-high.mp4");
    let spec = spec_for(&mp4, 256, 256);
    let out = thumbnail(spec.clone()).await.unwrap();
    let ThumbOutcome::Generated(d1) = out else {
        panic!("cover-less mp4 must generate a frame thumbnail");
    };
    assert!(d1.starts_with(&[0xFF, 0xD8]));
    assert!(crate::image::load_from_memory(&d1).is_ok());

    let cp = thumb_cache_path(
        &cache_dir(),
        &spec.path,
        spec.w,
        spec.h,
        spec.quality,
        spec.mod_unix,
        spec.file_size,
    );
    assert!(cp.exists(), "video thumb must be cached");

    // Second call: served from cache, byte-identical, no regeneration.
    let key = cp.to_string_lossy().into_owned();
    let before = generations_for(&key);
    let out2 = thumbnail(spec.clone()).await.unwrap();
    let ThumbOutcome::Generated(d2) = out2 else {
        panic!("cached video thumb must be Generated");
    };
    assert_eq!(d1, d2);
    assert_eq!(generations_for(&key), before);

    // Non-MP4 / undecodable containers must Err (→ 204 upstream), not panic.
    let tmp = crate::temporary::tempdir().unwrap();
    let mkv = tmp.path().join("clip.mkv");
    std::fs::write(&mkv, b"matroska-ish garbage").unwrap();
    assert!(thumbnail(spec_for(&mkv, 128, 128)).await.is_err());

    std::fs::remove_file(&cp).ok();
}

#[tokio::test]
async fn real_photos_end_to_end() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tmp-bench-media");
    for (name, w, h) in [
        ("photo-4000x3000.jpg", 4000u32, 3000u32),
        ("photo-4917x3456.jpg", 4917, 3456),
        ("photo-3840x2880.png", 3840, 2880),
    ] {
        let p = std::path::PathBuf::from(dir).join(name);
        if !p.exists() {
            println!("SKIP real_photos_end_to_end: missing {}", p.display());
            continue;
        }
        for target in [256u32, 512, 1024] {
            let spec = spec_for(&p, target, target);
            let out = thumbnail(spec.clone()).await.unwrap();
            let ThumbOutcome::Generated(d) = out else {
                panic!("{name}@{target} must generate");
            };
            let decoded = crate::image::load_from_memory(&d).unwrap();
            // Aspect-preserving box fit with ±1 rounding slack.
            let expect_w = target.min(w);
            let expect_h = target.min(h);
            let scale = f64::from(expect_w.min(expect_h)) / f64::from(w.min(h));
            let _ = scale;
            let (ew, eh) = if w <= target && h <= target {
                (w, h)
            } else {
                let r = f64::from(target) / f64::from(w.max(h));
                (
                    (f64::from(w) * r).round() as u32,
                    (f64::from(h) * r).round() as u32,
                )
            };
            assert!(
                (decoded.width() as i32 - ew as i32).abs() <= 1
                    && (decoded.height() as i32 - eh as i32).abs() <= 1,
                "{name}@{target}: got {}x{}, expected ~{ew}x{eh}",
                decoded.width(),
                decoded.height()
            );
            // Cleanup real cache dir entries.
            let cp = image_cache_path(
                &cache_dir(),
                &spec.path,
                spec.w,
                spec.h,
                spec.quality,
                spec.mod_unix,
                spec.file_size,
            );
            std::fs::remove_file(&cp).ok();
        }
    }
}

#[tokio::test]
async fn explicit_cache_directories_keep_native_runtimes_independent() {
    let directory = crate::temporary::tempdir().unwrap();
    let source = directory.path().join("image.jpg");
    write_jpeg(&source, 160, 100);
    let spec = spec_for(&source, 40, 40);
    for name in ["first-cache", "second-cache"] {
        let cache = directory.path().join(name);
        let key = image_cache_path(
            &cache,
            &spec.path,
            spec.w,
            spec.h,
            spec.quality,
            spec.mod_unix,
            spec.file_size,
        );
        let first = super::get_thumbnail(spec.clone(), &cache).await.unwrap();
        let first = match first {
            ThumbOutcome::Generated(bytes) => bytes,
            _ => panic!("fixture must be downscaled"),
        };
        let second = super::get_thumbnail(spec.clone(), &cache).await.unwrap();
        let second = match second {
            ThumbOutcome::Generated(bytes) => bytes,
            _ => panic!("cache must preserve generated bytes"),
        };
        assert_eq!(first, second);
        assert!(key.is_file());
        assert_eq!(std::fs::read(&key).unwrap(), *first);
        assert_eq!(generations_for(&key.to_string_lossy()), 1);
    }
}

#[tokio::test]
async fn explicit_cache_roots_keep_independent_files_for_the_same_source() {
    let source = crate::temporary::tempdir().unwrap();
    let first = crate::temporary::tempdir().unwrap();
    let second = crate::temporary::tempdir().unwrap();
    let path = source.path().join("photo.jpg");
    write_jpeg(&path, 1200, 600);
    let spec = spec_for(&path, 120, 120);
    let first_bytes = super::get_thumbnail(spec.clone(), first.path())
        .await
        .unwrap();
    let second_bytes = super::get_thumbnail(spec.clone(), second.path())
        .await
        .unwrap();
    let cache_path = |root: &std::path::Path| {
        image_cache_path(
            root,
            &spec.path,
            spec.w,
            spec.h,
            spec.quality,
            spec.mod_unix,
            spec.file_size,
        )
    };
    assert!(cache_path(first.path()).is_file());
    assert!(cache_path(second.path()).is_file());
    match (first_bytes, second_bytes) {
        (ThumbOutcome::Generated(first), ThumbOutcome::Generated(second)) => {
            assert_eq!(first, second)
        }
        _ => panic!("a resized image must populate each owned cache"),
    }
    drop(first);
    assert!(cache_path(second.path()).is_file());
}

#[tokio::test]
async fn render_fit_cache_changes_when_source_changes_within_the_same_second() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("source.jpg");
    let base = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    for (nanos, color) in [(100, [240, 10, 10]), (200, [10, 240, 10])] {
        crate::image::RgbImage::from_pixel(800, 400, crate::image::Rgb(color))
            .save(&path)
            .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(base + std::time::Duration::from_nanos(nanos)),
            )
            .unwrap();
        let bytes = render(path.to_str().unwrap(), 128, 128, true, directory.path())
            .await
            .unwrap()
            .unwrap();
        let decoded = crate::image::load_from_memory(&bytes).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (128, 64));
        let pixel = decoded.get_pixel(64, 32);
        if nanos == 100 {
            assert!(pixel[0] > pixel[1] + 100);
        } else {
            assert!(pixel[1] > pixel[0] + 100, "stale fit cache: {pixel:?}");
        }
    }
}

#[tokio::test]
#[ignore]
async fn thumbnail_latency_benchmark() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("photo.jpg");
    write_jpeg(&path, 4000, 3000);
    let mut cold = Vec::new();
    let mut warm = Vec::new();
    for index in 0..20 {
        let cache = directory.path().join(format!("cache-{index}"));
        let start = std::time::Instant::now();
        let bytes = render(path.to_str().unwrap(), 512, 512, true, &cache)
            .await
            .unwrap()
            .unwrap();
        cold.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(crate::image::load_from_memory(&bytes).unwrap().width(), 512);
        let start = std::time::Instant::now();
        let cached = render(path.to_str().unwrap(), 512, 512, true, &cache)
            .await
            .unwrap()
            .unwrap();
        warm.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(bytes, cached);
    }
    cold.sort_by(f64::total_cmp);
    warm.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({"input":"synthetic baseline JPEG 4000x3000","target":"512x384 aspect fit","samples":20,"coldMedianMs":cold[10],"coldP95Ms":cold[18],"warmMedianMs":warm[10],"warmP95Ms":warm[18]})
    );
}

fn jpeg_with_preview(path: &std::path::Path, preview_color: [u8; 3]) {
    let mut preview = Vec::new();
    crate::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut preview, 85)
        .encode_image(&crate::image::RgbImage::from_pixel(
            640,
            480,
            crate::image::Rgb(preview_color),
        ))
        .unwrap();
    let mut tiff = b"II".to_vec();
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&8u32.to_le_bytes());
    tiff.extend_from_slice(&0u16.to_le_bytes());
    tiff.extend_from_slice(&14u32.to_le_bytes());
    tiff.extend_from_slice(&2u16.to_le_bytes());
    for (tag, value) in [(0x0201u16, 44u32), (0x0202u16, preview.len() as u32)] {
        tiff.extend_from_slice(&tag.to_le_bytes());
        tiff.extend_from_slice(&4u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&value.to_le_bytes());
    }
    tiff.extend_from_slice(&0u32.to_le_bytes());
    tiff.extend_from_slice(&preview);
    let original = std::fs::read(path).unwrap();
    let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe1];
    jpeg.extend_from_slice(&((tiff.len() + 8) as u16).to_be_bytes());
    jpeg.extend_from_slice(b"Exif\0\0");
    jpeg.extend_from_slice(&tiff);
    jpeg.extend_from_slice(&original[2..]);
    std::fs::write(path, jpeg).unwrap();
}

#[tokio::test]
async fn sufficiently_large_embedded_preview_is_used_without_decoding_the_original() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("image.jpg");
    crate::image::RgbImage::from_pixel(1280, 960, crate::image::Rgb([240, 10, 10]))
        .save(&path)
        .unwrap();
    jpeg_with_preview(&path, [10, 240, 10]);
    let bytes = render(path.to_str().unwrap(), 128, 128, true, directory.path())
        .await
        .unwrap()
        .unwrap();
    let image = crate::image::load_from_memory(&bytes).unwrap().to_rgb8();
    let pixel = image.get_pixel(64, 64);
    assert!(
        pixel[1] > pixel[0] + 100,
        "embedded preview was not selected: {pixel:?}"
    );
}

#[tokio::test]
async fn fit_disk_cache_contains_the_final_output_without_a_second_encode() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("image.jpg");
    write_jpeg(&path, 800, 400);
    let mut spec = spec_for(&path, 128, 128);
    spec.quality = 85;
    spec.center_crop = true;
    spec.modified_nanos = std::fs::metadata(&path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let namespace = directory
        .path()
        .join("thumbs")
        .join(format!("fit-v1-{}", spec.modified_nanos));
    let cached = thumb_cache_path(
        &namespace,
        path.to_str().unwrap(),
        128,
        128,
        85,
        spec.mod_unix,
        spec.file_size,
    );
    let legacy = thumb_cache_path(
        &directory
            .path()
            .join("thumbs")
            .join(format!("render-v2-true-{}", spec.modified_nanos)),
        path.to_str().unwrap(),
        128,
        128,
        85,
        spec.mod_unix,
        spec.file_size,
    );
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    crate::image::RgbImage::from_pixel(128, 128, crate::image::Rgb([255, 255, 255]))
        .save(&legacy)
        .unwrap();
    let legacy_bytes = std::fs::read(&legacy).unwrap();
    let bytes = render(path.to_str().unwrap(), 128, 128, true, directory.path())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(&cached).unwrap(), bytes);
    let image = crate::image::open(cached).unwrap();
    assert_eq!((image.width(), image.height()), (128, 64));
    assert_eq!(std::fs::read(legacy).unwrap(), legacy_bytes);
}

#[tokio::test]
async fn decoded_pixels_and_output_buffers_are_admitted_before_allocating_them() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("oversized.png");
    let mut bytes = vec![0; 24];
    bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    bytes[16..20].copy_from_slice(&30000u32.to_be_bytes());
    bytes[20..24].copy_from_slice(&10000u32.to_be_bytes());
    std::fs::write(&path, bytes).unwrap();
    let error = render(path.to_str().unwrap(), 512, 512, true, directory.path())
        .await
        .unwrap_err();
    assert!(error.contains("exceeds memory budget"), "{error}");
}

#[test]
fn original_thumbnail_selection_preserves_source_bounds_and_transfer_budget() {
    assert!(super::original_fits(80, 80, 96, 96, true, 1000));
    assert!(super::original_fits(96, 96, 96, 96, true, 1000));
    assert!(super::original_fits(80, 40, 96, 96, false, 1000));
    assert!(super::original_fits(80, 40, 96, 96, true, 1000));
    assert!(!super::original_fits(97, 96, 96, 96, false, 1000));
    assert!(!super::original_fits(96, 97, 96, 96, false, 1000));
    assert!(!super::original_fits(
        80,
        80,
        96,
        96,
        true,
        super::PASSTHROUGH_MAX_BYTES as i64 + 1
    ));
    assert!(!super::original_fits(80, 80, 96, 96, true, -1));
}

#[test]
fn dc_input_memory_is_charged_only_for_the_full_jpeg_read_path() {
    let jpeg = Sniffed {
        kind: ImageKind::Jpeg,
        width: 4096,
        height: 3072,
        progressive: false,
        preserve_alpha: false,
        orientation: 1,
        work_bytes: 0,
    };
    assert_eq!(dc_input_bytes(jpeg, 256, 256, 12_000_000), 12_000_000);
    assert_eq!(dc_input_bytes(jpeg, 1024, 1024, 12_000_000), 0);
    assert_eq!(
        dc_input_bytes(
            Sniffed {
                kind: ImageKind::Png,
                ..jpeg
            },
            256,
            256,
            12_000_000
        ),
        0
    );
    assert_eq!(dc_input_bytes(jpeg, 256, 256, -1), 0);
}

#[tokio::test]
async fn png_thumbnails_keep_transparency_and_share_fit_cache_for_both_crop_flags() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("alpha.png");
    crate::image::RgbaImage::from_fn(768, 768, |x, _| {
        crate::image::Rgba([
            40,
            90,
            150,
            if x < 256 {
                0
            } else if x < 512 {
                128
            } else {
                255
            },
        ])
    })
    .save(&path)
    .unwrap();
    let unrelated_key = directory.path().join("unrelated.jpg");
    let unrelated = Bytes::from(vec![7; 16]);
    lru::global().put(unrelated_key.clone(), unrelated.clone());
    let mut spec = spec_for(&path, 256, 256);
    spec.center_crop = false;
    let ThumbOutcome::Generated(first) =
        get_thumbnail(spec.clone(), directory.path()).await.unwrap()
    else {
        panic!("expected resized PNG")
    };
    assert!(first.starts_with(b"\x89PNG\r\n\x1a\n"));
    let decoded = crate::image::load_from_memory(&first).unwrap().into_rgba8();
    assert_eq!(decoded.dimensions(), (256, 256));
    for (x, alpha) in [(32, 0), (128, 128), (224, 255)] {
        assert_eq!(decoded.get_pixel(x, 128)[3], alpha);
    }
    spec.center_crop = true;
    let ThumbOutcome::Generated(alternate) =
        get_thumbnail(spec.clone(), directory.path()).await.unwrap()
    else {
        panic!("expected shared fit PNG")
    };
    assert_eq!(alternate.as_ptr(), first.as_ptr());
    let disk_cache = directory.path().join("disk");
    std::fs::create_dir(&disk_cache).unwrap();
    std::fs::rename(directory.path().join("thumbs"), disk_cache.join("thumbs")).unwrap();
    let ThumbOutcome::Generated(second) = get_thumbnail(spec, &disk_cache).await.unwrap() else {
        panic!("expected cached PNG")
    };
    assert_eq!(first, second);
    assert_eq!(
        lru::global().get(&unrelated_key).unwrap().as_ptr(),
        unrelated.as_ptr()
    );
}

async fn check_still_format_thumbnail(name: &str, fixture: &[u8]) {
    let directory = crate::temporary::tempdir().unwrap();
    let source = directory.path().join(name);
    std::fs::write(&source, fixture).unwrap();
    let cache = directory.path().join("cache");
    let spec = spec_for(&source, 24, 24);
    let key = image_cache_path(
        &cache,
        &spec.path,
        spec.w,
        spec.h,
        spec.quality,
        spec.mod_unix,
        spec.file_size,
    );
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let spec = spec.clone();
        let cache = cache.clone();
        tasks.push(tokio::spawn(
            async move { get_thumbnail(spec, &cache).await },
        ));
    }
    let mut outputs = Vec::new();
    for task in tasks {
        let outcome = task
            .await
            .unwrap()
            .unwrap_or_else(|error| panic!("{name}: {error:#}"));
        let ThumbOutcome::Generated(bytes) = outcome else {
            panic!("{name}: browser thumbnail required")
        };
        let decoded = crate::image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (24, 16), "{name}");
        outputs.push(bytes);
    }
    assert_eq!(generations_for(key.to_str().unwrap()), 1, "{name}");
    assert!(
        outputs
            .iter()
            .all(|bytes| bytes.as_ptr() == outputs[0].as_ptr()),
        "{name}"
    );
    assert_eq!(std::fs::read(source).unwrap(), fixture, "{name}");
}

#[tokio::test]
async fn new_still_format_tiff_thumbnail_coalesces_batch_requests() {
    check_still_format_thumbnail(
        "source.tiff",
        include_bytes!("../../../../../plain-rs/testdata/still-images/rgb.tiff"),
    )
    .await;
}

#[tokio::test]
async fn new_still_format_avif_thumbnail_coalesces_batch_requests() {
    check_still_format_thumbnail(
        "source.avif",
        include_bytes!("../../../../../plain-rs/testdata/still-images/rgb.avif"),
    )
    .await;
}

#[tokio::test]
async fn new_still_format_heic_thumbnail_coalesces_batch_requests() {
    check_still_format_thumbnail(
        "source.heic",
        include_bytes!("../../../../../plain-rs/testdata/still-images/rgb.heic"),
    )
    .await;
}

#[tokio::test]
async fn new_still_format_heif_thumbnail_coalesces_batch_requests() {
    check_still_format_thumbnail(
        "source.heif",
        include_bytes!("../../../../../plain-rs/testdata/still-images/rgb.heic"),
    )
    .await;
}

#[tokio::test]
async fn concurrent_disk_cache_hits_share_the_encoded_storage_without_decoding() {
    let directory = crate::temporary::tempdir().unwrap();
    let source = directory.path().join("image.jpg");
    write_jpeg(&source, 800, 600);
    let spec = spec_for(&source, 128, 128);
    let ThumbOutcome::Generated(encoded) =
        get_thumbnail(spec.clone(), &directory.path().join("first"))
            .await
            .unwrap()
    else {
        panic!("expected generated JPEG")
    };
    let cache = directory.path().join("cold");
    let key = image_cache_path(
        &cache,
        &spec.path,
        spec.w,
        spec.h,
        spec.quality,
        spec.mod_unix,
        spec.file_size,
    );
    std::fs::create_dir_all(key.parent().unwrap()).unwrap();
    std::fs::write(&key, &encoded).unwrap();
    let mut tasks = Vec::new();
    for _ in 0..32 {
        let spec = spec.clone();
        let cache = cache.clone();
        tasks.push(tokio::spawn(async move {
            get_thumbnail(spec, &cache).await.unwrap()
        }));
    }
    let mut outputs = Vec::new();
    for task in tasks {
        let ThumbOutcome::Generated(bytes) = task.await.unwrap() else {
            panic!("expected cached JPEG")
        };
        assert_eq!(bytes, encoded);
        outputs.push(bytes);
    }
    assert!(
        outputs
            .iter()
            .all(|bytes| bytes.as_ptr() == outputs[0].as_ptr())
    );
    assert_eq!(generations_for(key.to_str().unwrap()), 0);
}

#[tokio::test]
async fn transparent_webp_uses_the_shared_aspect_fit_and_alpha_output() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("transparent.webp");
    let image = crate::image::RgbaImage::from_pixel(1024, 768, crate::image::Rgba([40, 90, 150, 128]));
    let mut input = std::io::Cursor::new(Vec::new());
    crate::image::DynamicImage::ImageRgba8(image)
        .write_to(&mut input, crate::image::ImageFormat::WebP)
        .unwrap();
    std::fs::write(&path, input.into_inner()).unwrap();
    let bytes = render(path.to_str().unwrap(), 256, 256, true, directory.path())
        .await
        .unwrap()
        .unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let pixels = crate::image::load_from_memory(&bytes).unwrap().into_rgba8();
    assert_eq!(pixels.dimensions(), (256, 192));
    assert!(pixels.pixels().all(|p| p[3] == 128));
}
