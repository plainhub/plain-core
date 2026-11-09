use super::*;

#[test]
fn zigzag_and_transform_follow_frequency_order() {
    let mut sorted = *ZIGZAG;
    sorted.sort_unstable();
    assert_eq!(sorted, std::array::from_fn(|i| i));
    assert_eq!(&ZIGZAG[..10], &[0, 1, 8, 16, 9, 2, 3, 10, 17, 24]);
    let dc = transform(&[160.0; 64], &Quantizer::new([1; 64]).unwrap());
    assert_eq!(dc[0], 256);
    assert_eq!(dc[1..], [0; 63]);
    let horizontal = std::array::from_fn(|i| 128.0 + 50.0 * BASIS[1][i % 8]);
    let coefficients = transform(&horizontal, &Quantizer::new([1; 64]).unwrap());
    assert!(coefficients[1] > 100);
    assert_eq!(coefficients[2], 0);
}

#[test]
fn entropy_bits_stuff_markers_and_encode_signed_values() {
    let mut scan = Scan {
        bytes: Vec::new(),
        pending: 0,
        len: 0,
    };
    scan.bits(0xff, 8);
    scan.bits(0b101, 3);
    scan.bits(0b01010, 5);
    assert_eq!(scan.bytes, [0xff, 0, 0xaa]);
    assert_eq!((scan.len, scan.pending), (0, 0));
    for (value, len) in [
        (0, 0),
        (-1, 1),
        (1, 1),
        (-3, 2),
        (3, 2),
        (1023, 10),
        (-2047, 11),
    ] {
        assert_eq!(category(value), len);
    }
    scan.bytes.clear();
    scan.value(-3);
    scan.value(3);
    scan.bits(0, 4);
    assert_eq!(scan.bytes, [0x30]);
    assert!(scan.symbol(&[Code::default(); 256], 0).is_err());
}

#[test]
fn table_reader_rejects_truncated_and_missing_tables() {
    for bytes in [
        &[][..],
        &[0xff, 0xd8],
        &[0xff, 0xd8, 0xff, 0xdb, 0, 67, 0],
        &[0xff, 0xd8, 0xff, 0xc4, 0, 3, 0],
    ] {
        assert!(Profile::from_template(bytes).is_err());
    }
    assert!(encode(&[], 1, 1, 75).is_err());
}

fn sampling(bytes: &[u8]) -> u8 {
    let mut pos = 2;
    while bytes[pos + 1] != 0xc0 {
        pos += 2 + usize::from(u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]));
    }
    bytes[pos + 11]
}

#[test]
fn every_quality_preserves_sampling_and_odd_image_edges() {
    let width = 17;
    let height = 19;
    let rgb = (0..width * height)
        .flat_map(|i| [(i % width * 15) as u8, (i / width * 13) as u8, 100])
        .collect::<Vec<_>>();
    for quality in 1..=100 {
        let bytes = super::super::encode_jpeg(
            crate::media::thumbnails::scale::Bitmap::new_rgb(
                rgb.clone(),
                width as u32,
                height as u32,
            ),
            quality,
        )
        .unwrap();
        assert_eq!(sampling(&bytes), if quality < 90 { 0x22 } else { 0x11 });
        let mut decoder = jpeg_decoder::Decoder::new(bytes.as_slice());
        let decoded = decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        assert_eq!((info.width, info.height), (width as u16, height as u16));
        if quality >= 90 {
            let max_error = rgb
                .iter()
                .zip(&decoded)
                .map(|(&a, &b)| a.abs_diff(b))
                .max()
                .unwrap();
            assert!(max_error <= 12, "quality {quality}: error {max_error}");
        }
    }
}

#[test]
fn single_pixels_and_constant_colors_have_no_edge_contamination() {
    for (w, h) in [(1, 1), (1, 17), (17, 1), (8, 8), (16, 16), (31, 33)] {
        for color in [[0, 0, 0], [255, 255, 255], [240, 20, 35], [15, 170, 225]] {
            let rgb = color.repeat(w * h);
            for quality in [75, 89] {
                let bytes = encode(&rgb, w as u16, h as u16, quality).unwrap();
                let decoded = image::load_from_memory(&bytes).unwrap().to_rgb8();
                let error = rgb
                    .iter()
                    .zip(decoded.as_raw())
                    .map(|(&a, &b)| a.abs_diff(b))
                    .max()
                    .unwrap();
                assert!(error <= 4, "{w}x{h} quality {quality}, {color:?}: {error}");
            }
        }
    }
}

#[test]
fn full_resolution_output_agrees_with_existing_encoder() {
    let rgb = (0..32 * 24)
        .flat_map(|i| [(i % 32 * 7) as u8, (i / 32 * 9) as u8, 120])
        .collect::<Vec<_>>();
    for quality in [90, 95, 100] {
        let mut reference = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut reference, quality)
            .encode(&rgb, 32, 24, image::ExtendedColorType::Rgb8)
            .unwrap();
        let output = super::super::encode_jpeg(
            crate::media::thumbnails::scale::Bitmap::new_rgb(rgb.clone(), 32, 24),
            quality,
        )
        .unwrap();
        let expected = image::load_from_memory(&reference).unwrap().to_rgb8();
        let actual = image::load_from_memory(&output).unwrap().to_rgb8();
        let max = expected
            .as_raw()
            .iter()
            .zip(actual.as_raw())
            .map(|(&a, &b)| a.abs_diff(b))
            .max()
            .unwrap();
        assert!(max == 0, "quality {quality}: difference {max}");
    }
}

#[test]
#[ignore]
fn jpeg_encode_latency_benchmark() {
    use sha2::Digest;
    let rgb = (0..512 * 512)
        .flat_map(|index| [(index % 200) as u8, ((index / 512) % 200) as u8, 90])
        .collect::<Vec<_>>();
    let mut samples = Vec::new();
    let mut encoded = Vec::new();
    for _ in 0..20 {
        let start = std::time::Instant::now();
        encoded = encode(&rgb, 512, 512, 85).unwrap();
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({"phase":"JPEG encode 512x512 q85","samples":20,"medianMs":samples[10],"p95Ms":samples[18],"digest":sha2::Sha256::digest(&encoded).to_vec()})
    );
}

#[test]
fn thumbnail_encode_preserves_the_baseline_bytes() {
    use sha2::Digest;
    let rgb = (0..512 * 512)
        .flat_map(|index| [(index % 200) as u8, ((index / 512) % 200) as u8, 90])
        .collect::<Vec<_>>();
    let encoded = encode(&rgb, 512, 512, 85).unwrap();
    assert_eq!(
        &sha2::Sha256::digest(&encoded)[..],
        &[
            152, 87, 193, 129, 234, 240, 16, 17, 76, 73, 162, 104, 17, 210, 223, 54, 9, 76, 41,
            145, 244, 27, 33, 104, 27, 182, 207, 181, 114, 145, 168, 240
        ]
    );
}

#[test]
fn reference_basis_matches_the_dct_definition() {
    for (frequency, weights) in BASIS.iter().enumerate() {
        for (sample, &weight) in weights.iter().enumerate() {
            let expected = if frequency == 0 {
                1.0 / 8.0f64.sqrt()
            } else {
                0.5 * (((2 * sample + 1) * frequency) as f64 * std::f64::consts::PI / 16.0).cos()
            };
            assert!((weight - expected).abs() < 1e-15);
        }
    }
}

#[test]
fn half_integer_chroma_coefficients_preserve_reference_rounding() {
    let quant = &profile(85).unwrap().quant[1];
    for (left, top, expected) in [(320, 96, 3), (128, 208, -3), (320, 496, 3)] {
        let rgb = (0..16 * 16)
            .flat_map(|i| {
                let (x, y) = (left + i % 16, top + i / 16);
                [((y * 512 + x) % 200) as u8, (y % 200) as u8, 90]
            })
            .collect::<Vec<_>>();
        let blocks = sampling_blocks(&rgb, 16, 16, 0, 0);
        let coefficients = transform(&blocks[5], quant);
        assert_eq!(coefficients[10], expected, "chroma block at {left},{top}");
    }
}

#[test]
fn factored_dct_matches_the_definition_at_every_frequency() {
    for seed in 1..=64 {
        let block = std::array::from_fn(|i| ((i * i * 3 + i * seed * 7 + seed * 11) % 256) as f64);
        let quant = std::array::from_fn(|i| (1 + (seed + i) % 23) as u8);
        let encoded = transform(&block, &Quantizer::new(quant).unwrap());
        for i in 0..64 {
            let k = ZIGZAG[i];
            let expected = (0..64)
                .map(|p| (block[p] - 128.0) * BASIS[k % 8][p % 8] * BASIS[k / 8][p / 8])
                .sum::<f64>()
                / f64::from(quant[i]);
            assert!((f64::from(encoded[i]) - expected).abs() <= 0.500000001);
        }
    }
}

fn reference_transform(block: &[f64; 64], quant: &[u8; 64]) -> [i16; 64] {
    let basis = &BASIS;
    let zigzag = &*ZIGZAG;
    let mut rows = [[0.0; 8]; 8];
    for y in 0..8 {
        for k in 0..8 {
            rows[y][k] = (0..8)
                .map(|x| (block[y * 8 + x] - 128.0) * basis[k][x])
                .sum();
        }
    }
    std::array::from_fn(|index| {
        let natural = zigzag[index];
        let value = (0..8)
            .map(|y| rows[y][natural % 8] * basis[natural / 8][y])
            .sum::<f64>();
        (value / f64::from(quant[index])).round() as i16
    })
}

#[test]
#[ignore]
fn compare_quantized_dct_cost() {
    use std::{hint::black_box, time::Instant};
    let blocks: Vec<[f64; 64]> = (0..1024)
        .map(|seed| std::array::from_fn(|i| ((i * i * 3 + i * seed * 7 + seed * 11) % 256) as f64))
        .collect();
    let q = &profile(85).unwrap().quant[0];
    for block in &blocks {
        assert_eq!(transform(block, q), reference_transform(block, &q.divisors));
    }
    for (label, function) in [
        (
            "matrix",
            (|b, q: &Quantizer| reference_transform(b, &q.divisors))
                as fn(&[f64; 64], &Quantizer) -> [i16; 64],
        ),
        ("factored", transform),
    ] {
        let start = Instant::now();
        for _ in 0..64 {
            for block in &blocks {
                black_box(function(black_box(block), black_box(q)));
            }
        }
        println!("{label} 65536 blocks {:?}", start.elapsed());
    }
}

#[test]
fn sampling_blocks_match_separate_channel_sampling_on_padded_edges() {
    let (w, h) = (19, 17);
    let rgb: Vec<u8> = (0..w * h * 3)
        .map(|i| (i * i + 123 * i + 7) as u8)
        .collect();
    for (left, top) in [(0, 0), (16, 0), (0, 16), (16, 16)] {
        let blocks = sampling_blocks(&rgb, w, h, left, top);
        for (block, actual) in blocks.iter().enumerate() {
            let step = if block < 4 { 1 } else { 2 };
            for (i, &sample) in actual.iter().enumerate() {
                let x = left + i % 8 * step + if block < 4 { block % 2 * 8 } else { 0 };
                let y = top + i / 8 * step + if block < 4 { block / 2 * 8 } else { 0 };
                let mut sum = 0.0;
                for dy in 0..step {
                    for dx in 0..step {
                        let off = 3 * ((y + dy).min(h - 1) * w + (x + dx).min(w - 1));
                        let [r, g, b] = [rgb[off], rgb[off + 1], rgb[off + 2]].map(f64::from);
                        sum += match block {
                            4 => 128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b,
                            5 => 128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b,
                            _ => 0.299 * r + 0.587 * g + 0.114 * b,
                        }
                        .round()
                        .clamp(0.0, 255.0);
                    }
                }
                assert_eq!(sample, (sum / (step * step) as f64).round());
            }
        }
    }
}

#[test]
fn quantizer_matches_reference_including_rounding_ties() {
    for quality in [1, 50, 85, 89] {
        for q in &profile(quality).unwrap().quant {
            for seed in 0..64 {
                let block =
                    std::array::from_fn(|i| ((i * i * 3 + i * seed * 7 + seed * 11) % 256) as f64);
                assert_eq!(
                    transform(&block, q),
                    reference_transform(&block, &q.divisors)
                );
            }
        }
    }
}

#[test]
fn color_conversion_preserves_rounding_for_every_rgb_triplet() {
    for r in 0..=255u8 {
        for g in 0..=255u8 {
            for b in 0..=255u8 {
                let rgb = [r, g, b];
                let [r, g, b] = rgb.map(f64::from);
                let expected = [
                    0.299 * r + 0.587 * g + 0.114 * b,
                    128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b,
                    128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b,
                ]
                .map(|v| v.round().clamp(0.0, 255.0) as u16);
                assert_eq!(ycbcr(&rgb), expected);
            }
        }
    }
}
