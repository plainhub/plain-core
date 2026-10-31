use super::*;

fn put_u16(bytes: &mut [u8], offset: usize, value: u16, little: bool) {
    bytes[offset..offset + 2].copy_from_slice(&if little {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    });
}
fn put_u32(bytes: &mut [u8], offset: usize, value: u32, little: bool) {
    bytes[offset..offset + 4].copy_from_slice(&if little {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    });
}
fn fixture(little: bool) -> Vec<u8> {
    let mut bytes = vec![0; 256];
    bytes[..2].copy_from_slice(if little { b"II" } else { b"MM" });
    put_u16(&mut bytes, 2, 42, little);
    put_u32(&mut bytes, 4, 8, little);
    put_u16(&mut bytes, 8, 3, little);
    for (index, tag, kind, count, value) in [
        (0, 0x010f, 2, 6, 180),
        (1, 0x0112, 3, 1, 6),
        (2, 0x8769, 4, 1, 64),
    ] {
        let offset = 10 + index * 12;
        put_u16(&mut bytes, offset, tag, little);
        put_u16(&mut bytes, offset + 2, kind, little);
        put_u32(&mut bytes, offset + 4, count, little);
        if kind == 3 {
            put_u16(&mut bytes, offset + 8, value as u16, little);
        } else {
            put_u32(&mut bytes, offset + 8, value, little);
        }
    }
    bytes[180..186].copy_from_slice(b"Maker\0");
    put_u16(&mut bytes, 64, 4, little);
    for (index, tag, kind, count, value) in [
        (0, 0x829a, 5, 1, 190),
        (1, 0x9003, 2, 20, 200),
        (2, 0x9011, 2, 7, 222),
        (3, 0x8827, 3, 1, 400),
    ] {
        let offset = 66 + index * 12;
        put_u16(&mut bytes, offset, tag, little);
        put_u16(&mut bytes, offset + 2, kind, little);
        put_u32(&mut bytes, offset + 4, count, little);
        if kind == 3 {
            put_u16(&mut bytes, offset + 8, value as u16, little);
        } else {
            put_u32(&mut bytes, offset + 8, value, little);
        }
    }
    put_u32(&mut bytes, 190, 1, little);
    put_u32(&mut bytes, 194, 125, little);
    bytes[200..220].copy_from_slice(b"2026:10:16 12:34:56\0");
    bytes[222..229].copy_from_slice(b"+08:00\0");
    bytes
}
#[test]
fn both_endian_orders_decode_inline_offsets_rationals_and_raw_date() {
    for little in [true, false] {
        let result = read(&fixture(little));
        assert_eq!(result.make, "Maker");
        assert_eq!(result.rotation, 90);
        assert_eq!(result.exposure_time, "1/125");
        assert_eq!(result.iso_speed, 400);
        assert_eq!(result.raw_taken_at, "2026:10:16 12:34:56");
        assert_eq!(result.offset_time, "+08:00");
    }
}
#[test]
fn jpeg_png_and_webp_containers_share_the_same_exif_parser() {
    let tiff = fixture(true);
    let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe1];
    jpeg.extend_from_slice(&((tiff.len() + 8) as u16).to_be_bytes());
    jpeg.extend_from_slice(b"Exif\0\0");
    jpeg.extend_from_slice(&tiff);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend_from_slice(&(tiff.len() as u32).to_be_bytes());
    png.extend_from_slice(b"eXIf");
    png.extend_from_slice(&tiff);
    let mut webp = b"RIFF\0\0\0\0WEBPEXIF".to_vec();
    webp.extend_from_slice(&(tiff.len() as u32).to_le_bytes());
    webp.extend_from_slice(&tiff);
    for bytes in [jpeg, png, webp] {
        assert_eq!(read(&bytes).exposure_time, "1/125");
    }
}
#[test]
fn malformed_offsets_truncation_and_zero_denominators_are_bounded() {
    let mut bytes = fixture(true);
    put_u32(&mut bytes, 194, 0, true);
    assert_eq!(read(&bytes).exposure_time, "");
    put_u32(&mut bytes, 18, u32::MAX, true);
    assert_eq!(read(&bytes).make, "");
    for length in 0..bytes.len() {
        let _ = read(&bytes[..length]);
    }
    put_u32(&mut bytes, 4, u32::MAX, true);
    assert_eq!(read(&bytes).rotation, 0);
}

#[test]
fn mirrored_or_unknown_orientations_do_not_invent_a_rotation_only_transform() {
    for little in [false, true] {
        for (orientation, rotation) in [
            (0, 0),
            (1, 0),
            (2, 0),
            (3, 180),
            (4, 0),
            (5, 0),
            (6, 90),
            (7, 0),
            (8, 270),
            (9, 0),
        ] {
            let mut bytes = fixture(little);
            put_u16(&mut bytes, 30, orientation, little);
            let metadata = read(&bytes);
            assert_eq!(
                metadata.rotation, rotation,
                "orientation={orientation},little={little}"
            );
            assert_eq!(
                metadata.orientation,
                (1..=8).contains(&orientation).then_some(orientation)
            );
            assert!(
                serde_json::to_value(metadata)
                    .unwrap()
                    .get("orientation")
                    .is_none()
            );
        }
    }
    assert!(read(b"unrecognized bytes").orientation.is_none());
}

#[cfg(feature = "library")]
#[test]
fn capture_dates_preserve_explicit_offsets_and_default_missing_offsets_to_utc() {
    let mut metadata = read(&fixture(true));
    assert_eq!(
        metadata.taken_at().as_deref(),
        Some("2026-10-16T04:34:56.000Z")
    );
    metadata.offset_time.clear();
    assert_eq!(
        metadata.taken_at().as_deref(),
        Some("2026-10-16T12:34:56.000Z")
    );
    metadata.raw_taken_at = "invalid".into();
    assert!(metadata.taken_at().is_none());
    assert!(read(b"invalid").taken_at().is_none());
}

#[test]
fn orientation_requires_one_exif_short_value_and_agrees_with_full_metadata() {
    for little in [false, true] {
        let original = fixture(little);
        assert_eq!(orientation(&original), Some(6));
        assert_eq!(orientation(&original), read(&original).orientation);
        let mut wrong_type = original.clone();
        put_u16(&mut wrong_type, 24, 4, little);
        assert!(orientation(&wrong_type).is_none());
        assert!(read(&wrong_type).orientation.is_none());
        let mut multiple = original;
        put_u32(&mut multiple, 26, 2, little);
        assert!(orientation(&multiple).is_none());
        assert!(read(&multiple).orientation.is_none());
    }
}
