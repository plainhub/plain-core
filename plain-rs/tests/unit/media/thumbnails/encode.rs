//! Unit tests for `src/media/thumb_engine/encode.rs` — moved out-of-line; compiled
//! as the `tests` child module via `#[cfg(test)] #[path]` there.
use super::*;

fn gradient_rgb(w: u32, h: u32) -> Bitmap {
    let mut data = Vec::with_capacity(w as usize * h as usize * 3);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&[x as u8, y as u8, 128]);
        }
    }
    Bitmap::new_rgb(data, w, h)
}

#[test]
fn encode_produces_valid_jpeg_header() {
    let out = encode_jpeg(gradient_rgb(64, 48), 75).unwrap();
    assert!(out.starts_with(&[0xFF, 0xD8]), "SOI marker");
    assert!(out.ends_with(&[0xFF, 0xD9]), "EOI marker");
    assert!(out.len() > 500);
    // Round-trip through the image crate to prove decodability + dims.
    let img = image::load_from_memory(&out).unwrap();
    assert_eq!((img.width(), img.height()), (64, 48));
}

#[test]
fn encode_rgba_flattens_alpha() {
    let mut data = Vec::new();
    for _ in 0..16 {
        data.extend_from_slice(&[255, 0, 0, 0]); // fully transparent red
    }
    let bm = Bitmap::new_rgba(data, 4, 4);
    let out = encode_jpeg(bm, 90).unwrap();
    let img = image::load_from_memory(&out).unwrap().to_rgba8();
    // Transparent pixels become white.
    assert_eq!(img.get_pixel(0, 0).0[..3], [255, 255, 255]);
}

#[test]
fn encode_quality_affects_size_and_decodes() {
    let bm = gradient_rgb(256, 256);
    let small = encode_jpeg(bm.clone(), 20).unwrap();
    let big = encode_jpeg(bm, 95).unwrap();
    assert!(
        small.len() < big.len(),
        "q20 {} vs q95 {}",
        small.len(),
        big.len()
    );
    assert!(image::load_from_memory(&small).is_ok());
    assert!(image::load_from_memory(&big).is_ok());
}

#[test]
fn encode_rejects_dimension_overflow() {
    // u16::MAX + 1 wide must be rejected, not silently truncated.
    let bm = Bitmap::new_rgb(vec![0u8; 65_536 * 3], 65_536, 1);
    assert!(encode_jpeg(bm, 75).is_err());
}

#[test]
fn encode_clamps_quality_rejects_empty_dimensions_and_flattens_partial_alpha() {
    assert_eq!(
        encode_jpeg(gradient_rgb(7, 9), 0).unwrap(),
        encode_jpeg(gradient_rgb(7, 9), 1).unwrap()
    );
    assert_eq!(
        encode_jpeg(gradient_rgb(7, 9), 255).unwrap(),
        encode_jpeg(gradient_rgb(7, 9), 100).unwrap()
    );
    for (w, h) in [(0, 0), (0, 1), (1, 0)] {
        assert!(encode_jpeg(Bitmap::new_rgb(Vec::new(), w, h), 75).is_err());
    }
    for quality in [75, 90] {
        let image =
            encode_jpeg(Bitmap::new_rgba([255, 0, 0, 128].repeat(81), 9, 9), quality).unwrap();
        let decoded = image::load_from_memory(&image).unwrap().to_rgb8();
        for pixel in decoded.pixels() {
            for (&actual, expected) in pixel.0.iter().zip([255u8, 127, 127]) {
                assert!(actual.abs_diff(expected) <= 3);
            }
        }
    }
}

#[test]
fn subsampled_noise_and_checkerboards_decode_at_all_low_qualities() {
    for quality in 1..90 {
        let data = (0..31 * 33 * 3)
            .map(|i| {
                (((i as u32).wrapping_mul(1103515245).wrapping_add(12345)).rotate_left(13) >> 24)
                    as u8
            })
            .collect();
        let encoded = encode_jpeg(Bitmap::new_rgb(data, 31, 33), quality).unwrap();
        assert!(image::load_from_memory(&encoded).is_ok());
        let checker = (0..31 * 33)
            .flat_map(|i| {
                if ((i % 31) + i / 31) % 2 == 0 {
                    [0; 3]
                } else {
                    [255; 3]
                }
            })
            .collect();
        let encoded = encode_jpeg(Bitmap::new_rgb(checker, 31, 33), quality).unwrap();
        let mut decoder = jpeg_decoder::Decoder::new(encoded.as_slice());
        assert!(decoder.decode().is_ok());
    }
}
