//! Unit tests for `src/media/thumb_engine/decode.rs` — moved out-of-line; compiled
//! as the `tests` child module via `#[cfg(test)] #[path]` there.
use super::*;

fn write_test_jpeg(path: &std::path::Path, w: u32, h: u32) {
    let img = crate::image::RgbImage::from_fn(w, h, |x, y| {
        crate::image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    });
    img.save_with_format(path, crate::image::ImageFormat::Jpeg)
        .unwrap();
}

#[test]
fn predict_scaled_matches_decoder_on_real_images() {
    let tmp = crate::temporary::tempdir().unwrap();
    for (w, h) in [(1, 1), (17, 31), (257, 129), (1024, 768)] {
        let path = tmp.path().join(format!("{w}-{h}.jpg"));
        write_test_jpeg(&path, w, h);
        for (tw, th) in [(1, 1), (32, 16), (128, 128), (256, 256), (512, 512)] {
            let bitmap = jpeg_scaled(&path, tw, th).unwrap();
            assert_eq!(
                (bitmap.width(), bitmap.height()),
                predict_scaled(w, h, tw, th)
            );
        }
    }
}

#[test]
fn jpeg_scaled_decodes_smaller_and_correct() {
    let tmp = crate::temporary::tempdir().unwrap();
    let p = tmp.path().join("src.jpg");
    write_test_jpeg(&p, 1024, 768);
    let bm = jpeg_scaled(&p, 256, 256).unwrap();
    // 1024/4 = 256 ≥ 256 → scale 1/4 expected.
    assert_eq!((bm.width(), bm.height()), (256, 192));
    assert_eq!(bm.channels(), 3);
    // The scaled decode must remain a faithful image (not blank/corrupt):
    // decode reference via full decode + box average of corner block.
    let full = image_full(&p).unwrap();
    let (d, _, _) = full.raw();
    // Corner block average (first 4x4) of the source:
    let mut mean = [0f64; 3];
    for y in 0..4 {
        for x in 0..4 {
            for c in 0..3 {
                mean[c] += d[(y * 1024 + x) * 3 + c] as f64;
            }
        }
    }
    for m in mean.iter_mut() {
        *m /= 16.0;
    }
    let (sd, _, _) = bm.raw();
    // Top-left pixel of scaled image should be near the source corner mean
    // (gradient image: x≈0..3 → ~1, y≈0..3 → ~1).
    assert!(
        (sd[0] as f64 - mean[0]).abs() < 4.0,
        "px0 {} vs mean {}",
        sd[0],
        mean[0]
    );
}

#[test]
fn jpeg_scaled_grayscale() {
    let tmp = crate::temporary::tempdir().unwrap();
    let p = tmp.path().join("gray.jpg");
    let img = crate::image::GrayImage::from_fn(512, 512, |x, _| crate::image::Luma([(x % 256) as u8]));
    img.save_with_format(&p, crate::image::ImageFormat::Jpeg).unwrap();
    let bm = jpeg_scaled(&p, 128, 128).unwrap();
    assert_eq!(bm.channels(), 3);
    assert_eq!(bm.width(), 128);
}

#[test]
fn jpeg_scaled_upscale_request_falls_back_to_full() {
    // Request bigger than source: scale() must not blow up.
    let tmp = crate::temporary::tempdir().unwrap();
    let p = tmp.path().join("tiny.jpg");
    write_test_jpeg(&p, 64, 48);
    let bm = jpeg_scaled(&p, 512, 512).unwrap();
    assert_eq!((bm.width(), bm.height()), (64, 48));
}

#[test]
fn image_full_png_webp() {
    let tmp = crate::temporary::tempdir().unwrap();
    let png = tmp.path().join("t.png");
    crate::image::RgbaImage::from_fn(100, 80, |x, y| crate::image::Rgba([x as u8, y as u8, 55, 255]))
        .save_with_format(&png, crate::image::ImageFormat::Png)
        .unwrap();
    let bm = image_full(&png).unwrap();
    assert_eq!((bm.width(), bm.height(), bm.channels()), (100, 80, 4));

    // WebP round-trip via the image crate encoder (lossless).
    let webp = tmp.path().join("t.webp");
    let img = crate::image::RgbImage::from_fn(64, 32, |x, _| crate::image::Rgb([x as u8, 3, 4]));
    img.save_with_format(&webp, crate::image::ImageFormat::WebP)
        .unwrap();
    let bm = image_full(&webp).unwrap();
    assert_eq!((bm.width(), bm.height(), bm.channels()), (64, 32, 3));
}

#[test]
fn image_from_bytes_and_errors() {
    let tmp = crate::temporary::tempdir().unwrap();
    let p = tmp.path().join("x.jpg");
    write_test_jpeg(&p, 32, 32);
    let bytes = std::fs::read(&p).unwrap();
    let bm = image_from_bytes(&bytes).unwrap();
    assert_eq!(bm.width(), 32);

    // Errors: empty, garbage, truncated JPEG header.
    assert!(image_from_bytes(&[]).is_err());
    assert!(image_from_bytes(b"garbage bytes!").is_err());
    let mut trunc = bytes[..20].to_vec();
    trunc[0] = 0xFF;
    assert!(image_from_bytes(&trunc).is_err());
    // Corrupt-but-sniffable file on disk (valid header, dead payload).
    let mut bad = bytes.clone();
    let cut = bad.len() / 2;
    for b in bad.iter_mut().skip(cut) {
        *b = 0x55;
    }
    assert!(image_from_bytes(&bad).is_err() || bad.len() == cut);
    assert!(image_from_bytes(&bad).is_err());
}

#[test]
fn corrupt_file_on_disk_errors_cleanly() {
    let tmp = crate::temporary::tempdir().unwrap();
    let p = tmp.path().join("corrupt.jpg");
    write_test_jpeg(&p, 128, 128);
    let mut bytes = std::fs::read(&p).unwrap();
    let n = bytes.len();
    for b in bytes.iter_mut().take(n / 2).skip(n / 4) {
        *b ^= 0xAA;
    }
    std::fs::write(&p, bytes).unwrap();
    // Either decoder may or may not tolerate mid-stream corruption, but
    // the call must not panic and must return a Bitmap or an error.
    let _ = jpeg_scaled(&p, 64, 64);
    let _ = image_full(&p);
}

#[test]
fn jpeg_entropy_reads_consume_the_buffer_before_refilling() {
    struct CountReads<'a> {
        bytes: &'a [u8],
        calls: usize,
    }
    impl Read for CountReads<'_> {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            self.calls += 1;
            self.bytes.read(out)
        }
    }
    let source = CountReads {
        bytes: b"abcdefghijk",
        calls: 0,
    };
    let mut input = JpegInput(BufReader::with_capacity(4, source));
    for (i, expected) in b"abcde".iter().enumerate() {
        let mut byte = [0];
        input.read_exact(&mut byte).unwrap();
        assert_eq!(byte[0], *expected);
        assert_eq!(input.0.get_ref().calls, i / 4 + 1);
    }
    let mut segment = [0; 5];
    input.read_exact(&mut segment).unwrap();
    assert_eq!(&segment, b"fghij");
    let mut last = [0];
    input.read_exact(&mut last).unwrap();
    assert_eq!(&last, b"k");
    assert_eq!(
        input.read_exact(&mut last).unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn file_entropy_reader_preserves_scaled_pixels_across_refills() {
    let tmp = crate::temporary::tempdir().unwrap();
    let path = tmp.path().join("original.jpg");
    write_test_jpeg(&path, 1024, 768);
    let data = std::fs::read(&path).unwrap();
    for target in [64, 256, 512] {
        let buffered = jpeg_scaled_reader(
            JpegInput(BufReader::with_capacity(7, data.as_slice())),
            target,
            target,
        )
        .unwrap();
        let reference = jpeg_scaled_bytes(&data, target, target).unwrap();
        assert_eq!(buffered.raw(), reference.raw());
    }
}

#[path = "../../../support/jpeg.rs"]
mod jpeg_fixture;

#[test]
fn generated_baseline_progressive_cmyk_and_lossless_keep_exact_pixels() {
    for progressive in [false, true] {
        let bytes = jpeg_fixture::constant(progressive, 1);
        for target in [1, 2, 4, 8, 16] {
            let bitmap = jpeg_scaled_bytes(&bytes, target, target).unwrap();
            let size = target.min(8);
            assert_eq!((bitmap.width(), bitmap.height()), (size, size));
            assert!(bitmap.raw().0.iter().all(|v| *v == 128));
        }
    }
    let bitmap = jpeg_scaled_bytes(&jpeg_fixture::constant(false, 4), 8, 8).unwrap();
    assert_eq!((bitmap.width(), bitmap.height()), (8, 8));
    assert!(bitmap.raw().0.iter().all(|v| *v == 63));
    for precision in [8, 16] {
        let bitmap = jpeg_scaled_bytes(&jpeg_fixture::lossless(precision), 4, 3).unwrap();
        assert_eq!((bitmap.width(), bitmap.height()), (4, 3));
        assert_eq!(bitmap.raw().0, &[128; 36]);
    }
}

#[test]
fn truncated_segments_and_invalid_huffman_tables_fail_without_panics() {
    let bytes = jpeg_fixture::constant(false, 1);
    for len in 0..bytes.len() - 2 {
        assert!(
            jpeg_scaled_bytes(&bytes[..len], 8, 8).is_err(),
            "prefix {len}"
        );
    }
    let mut invalid = bytes;
    let table = invalid.windows(2).position(|v| v == [0xff, 0xc4]).unwrap();
    invalid[table + 4] = 0x20;
    assert!(jpeg_scaled_bytes(&invalid, 8, 8).is_err());
}

#[test]
#[ignore]
fn benchmark_scaled_jpeg_generated_inputs() {
    let tmp = crate::temporary::tempdir().unwrap();
    let path = tmp.path().join("generated.jpg");
    write_test_jpeg(&path, 2048, 1536);
    let bytes = std::fs::read(path).unwrap();
    for size in [256, 512, 1024] {
        let start = std::time::Instant::now();
        for _ in 0..32 {
            let bitmap = jpeg_scaled_bytes(std::hint::black_box(&bytes), size, size).unwrap();
            assert_eq!((bitmap.width(), bitmap.height()), (size, size * 3 / 4));
            std::hint::black_box(bitmap);
        }
        println!("jpeg {size}: {:?} per image", start.elapsed() / 32);
    }
}
