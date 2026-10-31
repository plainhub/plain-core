use super::*;

#[test]
fn extract_cover_nonexistent() {
    assert!(extract_cover("/nonexistent/file.mp3").is_none());
}

#[test]
fn thumbnail_cache_ref_path_no_sidecar() {
    let p = "/music/song.mp3";
    assert_eq!(thumbnail_cache_ref_path(p), p);
}

#[test]
fn find_sidecar_returns_none_for_missing() {
    assert!(find_sidecar_cover_path("/nonexistent/song.mp3").is_none());
}

#[test]
fn embedded_cover_reader_preserves_exact_image_bytes_and_respects_descriptor_bounds() {
    let audio = include_bytes!("../../../../../plain-rs/testdata/audio-cover.mp3");
    let image = include_bytes!("../../../../../plain-rs/testdata/v-prog-gray.jpg");
    let mut bytes = b"outside-prefix".to_vec();
    let offset = bytes.len() as u64;
    bytes.extend_from_slice(audio);
    bytes.extend_from_slice(b"outside-suffix");
    let mut source = std::io::Cursor::new(bytes);
    let mut reader =
        crate::libs::utils::range_reader::RangeReader::new(&mut source, offset, audio.len() as u64)
            .unwrap();
    let cover = extract_embedded_cover_reader(&mut reader).unwrap();
    assert_eq!(cover.bytes, image);
    assert_eq!(cover.mime, "image/jpeg");
    let mut absent = std::io::Cursor::new(include_bytes!(
        "../../../../../plain-rs/testdata/audio-untagged.mp3"
    ));
    assert!(extract_embedded_cover_reader(&mut absent).is_none());
}
