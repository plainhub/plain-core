use super::*;

async fn thumbnail(path: &std::path::Path, width: u32, height: u32) -> Result<Bytes> {
    render(path.to_str().unwrap(), width, height, false, None).await
}

#[tokio::test]
async fn png_inspection_reads_only_the_header() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("image.png");
    crate::image::RgbImage::from_pixel(600, 480, crate::image::Rgb([5, 10, 30]))
        .save(&path)
        .unwrap();
    assert!(path.metadata().unwrap().len() > 64);
    let (head, image) = inspect_image(path.to_str().unwrap()).await.unwrap();
    assert_eq!(head.len(), 64);
    let image = image.unwrap();
    assert_eq!((image.width, image.height), (600, 480));
}

fn write_jpeg(path: &std::path::Path, w: u32, h: u32) {
    let img = crate::image::RgbImage::from_fn(w, h, |x, y| {
        crate::image::Rgb([(x % 200) as u8, (y % 200) as u8, 90])
    });
    img.save_with_format(path, crate::image::ImageFormat::Jpeg)
        .unwrap();
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
    let s = ThumbSpec::sanitize("/x", -5, 9_999_999, 1000, 2);
    assert_eq!((s.w, s.h, s.quality), (0, MAX_TARGET_DIM, 100));
}

#[test]
fn passthrough_small_images() {
    let dir = fresh_dir("pass");
    let jpg = dir.path().join("small.jpg");
    write_jpeg(&jpg, 64, 48);
    let png = dir.path().join("small.png");
    crate::image::RgbaImage::from_pixel(32, 32, crate::image::Rgba([1, 2, 3, 0]))
        .save(&png)
        .unwrap();
    for path in [&jpg, &png] {
        assert_eq!(read_original_thumbnail(path, 64, 64, None).unwrap().unwrap(), std::fs::read(path).unwrap());
        assert!(read_original_thumbnail(path, 16, 16, None).unwrap().is_none());
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

    let d = thumbnail(&img, 100, 100).await.unwrap();
    let decoded = crate::image::load_from_memory(&d).unwrap();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (75, 100),
        "orientation-6 thumb must be portrait 75x100"
    );
}

#[tokio::test]
async fn unicode_and_spaced_paths() {
    let dir = fresh_dir("paths");
    let sub = dir.path().join("ünïcode 空 目録");
    std::fs::create_dir_all(&sub).unwrap();
    for path in [sub.join("photo name #1.JPG"), dir.path().join("Ünicode 名前 test.jpg")] {
        write_jpeg(&path, 320, 240);
        let bytes = thumbnail(&path, 64, 64).await.unwrap();
        let decoded = crate::image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 48));
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
    let r = thumbnail(&fake, 64, 64).await;
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
    let err = thumbnail(&fakepng, 64, 64)
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
    assert!(thumbnail(&txt, 64, 64).await.is_err());
    // Empty file with .jpg extension.
    let empty = dir.path().join("empty.jpg");
    std::fs::write(&empty, b"").unwrap();
    assert!(thumbnail(&empty, 64, 64).await.is_err());
    // Truncated real JPEG (header ok, body cut): decoders are lenient —
    // zune fills the missing scan data, so a thumbnail may still come
    // out (same behavior as the previous pipeline). The contract is
    // "never panic, output is always a decodable JPEG when Ok".
    let trunc = dir.path().join("trunc.jpg");
    write_jpeg(&trunc, 200, 150);
    let bytes = std::fs::read(&trunc).unwrap();
    std::fs::write(&trunc, &bytes[..bytes.len() / 3]).unwrap();
    if let Ok(d) = thumbnail(&trunc, 64, 64).await {
        assert!(crate::image::load_from_memory(&d).is_ok(), "output must decode");
    }
}

#[tokio::test]
async fn cover_sidecar_thumbnail() {
    let dir = fresh_dir("cover");
    let song = dir.path().join("song.mp3");
    std::fs::write(&song, b"not really an mp3").unwrap();
    write_jpeg(&dir.path().join("cover.jpg"), 500, 400);
    let bytes = thumbnail(&song, 128, 128).await.unwrap();
    assert!(bytes.starts_with(&[0xff, 0xd8]));
    let decoded = crate::image::load_from_memory(&bytes).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (128, 102));
}

#[tokio::test]
async fn progressive_jpeg_dc_fast_path_end_to_end() {
    let p = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../plain-rs/testdata"))
        .join("v-synthetic-prog.jpg");
    let d = thumbnail(&p, 128, 128).await.unwrap();
    let decoded = crate::image::load_from_memory(&d).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (128, 85));
}

#[tokio::test]
async fn video_thumbnail_end_to_end() {
    let mp4 = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../plain-rs/testdata"))
        .join("video-h264-high.mp4");
    let bytes = thumbnail(&mp4, 256, 256).await.unwrap();
    assert!(bytes.starts_with(&[0xff, 0xd8]));
    assert!(crate::image::load_from_memory(&bytes).is_ok());

    let tmp = crate::temporary::tempdir().unwrap();
    let mkv = tmp.path().join("clip.mkv");
    std::fs::write(&mkv, b"matroska-ish garbage").unwrap();
    assert!(thumbnail(&mkv, 128, 128).await.is_err());
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
            let d = thumbnail(&p, target, target).await.unwrap();
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
        }
    }
}

#[tokio::test]
#[ignore]
async fn thumbnail_latency_benchmark() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("photo.jpg");
    write_jpeg(&path, 4000, 3000);
    let mut samples = Vec::new();
    for _ in 0..20 {
        let start = std::time::Instant::now();
        let bytes = thumbnail(&path, 512, 512).await.unwrap();
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(crate::image::load_from_memory(&bytes).unwrap().width(), 512);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{}",
        crate::serde_json::json!({"input":"synthetic baseline JPEG 4000x3000","target":"512x384 aspect fit","samples":20,"medianMs":samples[10],"p95Ms":samples[18]})
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
    let bytes = render(path.to_str().unwrap(), 128, 128, true, None)
        .await
        .unwrap();
    let image = crate::image::load_from_memory(&bytes).unwrap().to_rgb8();
    let pixel = image.get_pixel(64, 64);
    assert!(
        pixel[1] > pixel[0] + 100,
        "embedded preview was not selected: {pixel:?}"
    );
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
    let error = render(path.to_str().unwrap(), 512, 512, true, None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("exceeds memory budget"), "{error}");
}

#[test]
fn original_thumbnail_selection_preserves_transfer_budget() {
    let directory = fresh_dir("passthrough-budget");
    let path = directory.path().join("large.jpg");
    write_jpeg(&path, 80, 80);
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(PASSTHROUGH_MAX_BYTES + 1).unwrap();
    assert!(read_original_thumbnail(&path, 96, 96, None).unwrap().is_none());
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
async fn png_thumbnails_keep_transparency() {
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
    let first = thumbnail(&path, 256, 256).await.unwrap();
    assert!(first.starts_with(b"\x89PNG\r\n\x1a\n"));
    let decoded = crate::image::load_from_memory(&first).unwrap().into_rgba8();
    assert_eq!(decoded.dimensions(), (256, 256));
    for (x, alpha) in [(32, 0), (128, 128), (224, 255)] {
        assert_eq!(decoded.get_pixel(x, 128)[3], alpha);
    }
}

async fn check_still_format_thumbnail(name: &str, fixture: &[u8]) {
    let directory = crate::temporary::tempdir().unwrap();
    let source = directory.path().join(name);
    std::fs::write(&source, fixture).unwrap();
    let bytes = thumbnail(&source, 24, 24)
        .await
        .unwrap_or_else(|error| panic!("{name}: {error:#}"));
    let decoded = crate::image::load_from_memory(&bytes).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (24, 16), "{name}");
    assert_eq!(std::fs::read(source).unwrap(), fixture, "{name}");
}

#[tokio::test]
async fn new_still_format_tiff_thumbnail_preserves_aspect_ratio() {
    check_still_format_thumbnail(
        "source.tiff",
        include_bytes!("../../../../../plain-rs/testdata/still-images/rgb.tiff"),
    )
    .await;
}

#[tokio::test]
async fn new_still_format_avif_thumbnail_preserves_aspect_ratio() {
    check_still_format_thumbnail(
        "source.avif",
        include_bytes!("../../../../../plain-rs/testdata/still-images/rgb.avif"),
    )
    .await;
}

#[tokio::test]
async fn new_still_format_heic_thumbnail_preserves_aspect_ratio() {
    check_still_format_thumbnail(
        "source.heic",
        include_bytes!("../../../../../plain-rs/testdata/still-images/rgb.heic"),
    )
    .await;
}

#[tokio::test]
async fn new_still_format_heif_thumbnail_preserves_aspect_ratio() {
    check_still_format_thumbnail(
        "source.heif",
        include_bytes!("../../../../../plain-rs/testdata/still-images/rgb.heic"),
    )
    .await;
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
    let bytes = render(path.to_str().unwrap(), 256, 256, true, None)
        .await
        .unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let pixels = crate::image::load_from_memory(&bytes).unwrap().into_rgba8();
    assert_eq!(pixels.dimensions(), (256, 192));
    assert!(pixels.pixels().all(|p| p[3] == 128));
}
