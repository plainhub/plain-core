use super::*;

#[test]
fn png_animation_detection_seeks_over_metadata_without_reading_pixels() {
    struct Input<'a> {
        cursor: std::io::Cursor<&'a [u8]>,
        bytes_read: usize,
    }
    impl Read for Input<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let bytes = self.cursor.read(buffer)?;
            self.bytes_read += bytes;
            Ok(bytes)
        }
    }
    impl Seek for Input<'_> {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            self.cursor.seek(position)
        }
    }
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/two-frame.apng"
    ));
    let mut input = Input {
        cursor: std::io::Cursor::new(bytes.as_slice()),
        bytes_read: 0,
    };
    assert!(is_animated_png(&mut input));
    assert_eq!(input.bytes_read, 40);
}

#[test]
fn png_animation_detection_preserves_both_extensions_and_skips_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let animated = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/two-frame.apng"
    ));
    for name in [
        "animation.png",
        "animation.apng",
        "animation.PNG",
        "animation.APNG",
    ] {
        let path = directory.path().join(name);
        std::fs::write(&path, animated).unwrap();
        assert!(is_animated_image_or_svg(&path), "{name}");
        for length in [0, 8, 20, 33, 256] {
            std::fs::write(&path, &animated[..length]).unwrap();
            assert!(!is_animated_image_or_svg(&path), "{name}: {length}");
        }
        image::RgbaImage::new(1, 1)
            .save_with_format(&path, image::ImageFormat::Png)
            .unwrap();
        assert!(!is_animated_image_or_svg(&path), "static {name}");
    }
}

#[test]
fn shared_header_detection_preserves_animated_and_static_container_rules() {
    let directory = tempfile::tempdir().unwrap();
    assert!(is_animated_image_or_svg(&directory.path().join("logo.svg")));
    assert!(!is_animated_image_or_svg(
        &directory.path().join("photo.png")
    ));
    for (name, bytes, animated) in [
        ("one.gif", b"GIF89a\0\0".to_vec(), true),
        ("two.webp", b"RIFF\0\0\0\0WEBPVP8X\x02\0".to_vec(), true),
        ("three.webp", b"RIFF\0\0\0\0WEBPVP8X\0\0".to_vec(), false),
        ("four.heic", b"\0\0\0\0ftypmsf1".to_vec(), true),
        ("five.heic", b"\0\0\0\0ftypheic".to_vec(), false),
        ("six.txt", b"<svg/>".to_vec(), false),
    ] {
        let path = directory.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(is_animated_image_or_svg(&path), animated, "{name}");
    }
}

#[test]
fn animated_header_matrix_covers_removed_app_parser_cases() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.heic");
    for (bytes, expected) in [
        (b"GIF87a".to_vec(), true),
        (b"GIF89a".to_vec(), true),
        (b"\0\0\0\x18ftyphevc".to_vec(), true),
        (b"\0\0\0\x18ftyphevx".to_vec(), true),
        (b"\0\0\0\x18ftypavif".to_vec(), false),
        (b"\0\0\0\x18ftypheic".to_vec(), false),
        (b"<svg xmlns=\"synthetic\">".to_vec(), true),
        (vec![0xff, 0xd8, 0xff], false),
        (b"\x89PNG".to_vec(), false),
        (b"RIFF\0\0\0\0WEBPVP8X\x02".to_vec(), false),
        (vec![0; 256], false),
        (Vec::new(), false),
    ] {
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(is_animated_image_or_svg(&path), expected, "{bytes:?}");
    }
}

#[test]
fn isobmff_sequences_are_recognized_by_major_and_compatible_brands() {
    let directory = tempfile::tempdir().unwrap();
    for extension in ["avif", "AVIF", "heic", "heif"] {
        let path = directory.path().join(format!("sequence.{extension}"));
        for brand in [b"avis", b"msf1", b"hevc", b"hevx"] {
            let mut bytes = 24_u32.to_be_bytes().to_vec();
            bytes.extend_from_slice(b"ftyp");
            bytes.extend_from_slice(brand);
            bytes.extend_from_slice(b"\0\0\0\0mif1avif");
            std::fs::write(&path, &bytes).unwrap();
            assert!(is_animated_image_or_svg(&path), "major {brand:?}");
            bytes[8..12].copy_from_slice(b"mif1");
            bytes[20..24].copy_from_slice(brand);
            std::fs::write(&path, &bytes).unwrap();
            assert!(is_animated_image_or_svg(&path), "compatible {brand:?}");
        }
    }
}

#[test]
fn isobmff_animation_scan_stays_inside_ftyp_and_handles_extended_sizes() {
    for extended in [false, true] {
        let mut bytes = Vec::new();
        let size = if extended { 1048_u64 } else { 1040_u64 };
        bytes.extend_from_slice(&(if extended { 1_u32 } else { size as u32 }).to_be_bytes());
        bytes.extend_from_slice(b"ftyp");
        if extended {
            bytes.extend_from_slice(&size.to_be_bytes());
        }
        bytes.extend_from_slice(b"avif\0\0\0\0");
        bytes.resize(size as usize, 0);
        bytes.extend_from_slice(b"\0\0\0\x10mdatavishevc");
        let mut input = std::io::Cursor::new(bytes.clone());
        assert!(!is_animated_isobmff(&mut input));
        assert_eq!(input.position(), size);
        bytes[size as usize - 4..size as usize].copy_from_slice(b"avis");
        let mut input = std::io::Cursor::new(bytes);
        assert!(is_animated_isobmff(&mut input));
        assert_eq!(input.position(), size);
    }
    for bytes in [
        b"\0\0\0\x08ftypavif".as_slice(),
        b"\0\0\0\x15ftypavif\0\0\0\0avis",
    ] {
        assert!(!is_animated_isobmff(&mut std::io::Cursor::new(bytes)));
    }
}
