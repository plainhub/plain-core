use super::*;

#[test]
fn zigzag_and_transform_follow_frequency_order() {
    let mut sorted = *ZIGZAG;
    sorted.sort_unstable();
    assert_eq!(sorted, std::array::from_fn(|i| i));
    assert_eq!(&ZIGZAG[..10], &[0, 1, 8, 16, 9, 2, 3, 10, 17, 24]);
    let dc = transform(&[160.0; 64], &[1; 64]);
    assert_eq!(dc[0], 256);
    assert_eq!(dc[1..], [0; 63]);
    let horizontal = std::array::from_fn(|i| 128.0 + 50.0 * BASIS[1][i % 8]);
    let coefficients = transform(&horizontal, &[1; 64]);
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
