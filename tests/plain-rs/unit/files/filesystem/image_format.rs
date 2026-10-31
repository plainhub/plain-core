use super::*;

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
