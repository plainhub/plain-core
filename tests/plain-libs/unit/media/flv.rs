use super::*;
use std::io::Cursor;

fn short(value: &str) -> Vec<u8> {
    [
        (value.len() as u16).to_be_bytes().as_slice(),
        value.as_bytes(),
    ]
    .concat()
}
fn string(value: &str) -> Vec<u8> {
    [vec![2], short(value)].concat()
}
fn number(value: f64) -> Vec<u8> {
    [vec![0], value.to_be_bytes().to_vec()].concat()
}
fn property(key: &str, value: &[u8]) -> Vec<u8> {
    [short(key), value.to_vec()].concat()
}
fn metadata(prefix: bool, object: bool, duration: f64) -> Vec<u8> {
    let mut bytes = if prefix {
        string("@setDataFrame")
    } else {
        Vec::new()
    };
    bytes.extend(string("onMetaData"));
    if object {
        bytes.push(3);
    } else {
        bytes.extend([8, 255, 255, 255, 255]);
    }
    for (key, value) in [
        ("duration", duration),
        ("width", 640.0),
        ("height", 360.0),
        ("framerate", 29.97),
    ] {
        bytes.extend(property(key, &number(value)));
    }
    bytes.extend(property("title", &string("FLV Title")));
    bytes.extend(property("artist", &string("Artist")));
    bytes.extend(property("creationdate", &string("2026-10-16T00:00:00Z")));
    bytes.extend(property(
        "keyframes",
        &[vec![3], property("width", &number(999.0)), vec![0, 0, 9]].concat(),
    ));
    bytes.extend([0, 0, 9]);
    bytes
}
fn tag_bytes(kind: u8, timestamp: u32, body: &[u8]) -> Vec<u8> {
    let mut bytes = vec![kind];
    bytes.extend(&(body.len() as u32).to_be_bytes()[1..]);
    bytes.extend(&timestamp.to_be_bytes()[1..]);
    bytes.push((timestamp >> 24) as u8);
    bytes.extend([0; 3]);
    bytes.extend(body);
    bytes.extend((body.len() as u32 + 11).to_be_bytes());
    bytes
}
fn document(tags: &[u8]) -> Vec<u8> {
    [
        b"FLV\x01\x05\x00\x00\x00\x09\x00\x00\x00\x00".as_slice(),
        tags,
    ]
    .concat()
}
#[test]
fn amf_objects_and_ecma_arrays_share_precise_metadata_without_nested_key_collisions() {
    for prefix in [false, true] {
        for object in [false, true] {
            let bytes = document(&tag_bytes(18, 0, &metadata(prefix, object, 1.2345)));
            let info = super::super::video_info::read(&mut Cursor::new(&bytes)).unwrap();
            assert_eq!((info.width, info.height), (640, 360));
            assert_eq!(info.duration_ms, 1234);
            assert_eq!(info.frame_rate, 29.97);
            assert_eq!(info.title, "FLV Title");
            assert_eq!(info.artist, "Artist");
            assert_eq!(info.raw_taken_at.as_deref(), Some("2026-10-16T00:00:00Z"));
            assert_eq!(
                super::super::probe::probe_duration_reader(&mut Cursor::new(bytes)),
                Some(1234)
            );
        }
    }
}
struct Counted {
    inner: Cursor<Vec<u8>>,
    bytes: usize,
}
impl Read for Counted {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(output)?;
        self.bytes += count;
        Ok(count)
    }
}
impl Seek for Counted {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(from)
    }
}
#[test]
fn metadata_sampling_skips_large_media_inside_a_descriptor() {
    let mut tags = tag_bytes(9, 0, &vec![9; 4 * 1024 * 1024]);
    tags.extend(tag_bytes(18, 0, &metadata(false, false, 1.25)));
    let bytes = document(&tags);
    let length = bytes.len();
    let mut wrapped = b"outside".to_vec();
    wrapped.extend(bytes);
    wrapped.extend(b"outside");
    let mut counted = Counted {
        inner: Cursor::new(wrapped),
        bytes: 0,
    };
    let mut range =
        crate::utils::range_reader::RangeReader::new(&mut counted, 7, length as u64).unwrap();
    let info = video(&mut range).unwrap();
    assert_eq!(info.duration_ms, 1250);
    assert_eq!((info.width, info.height), (640, 360));
    drop(range);
    assert!(counted.bytes < 4096, "read {} bytes", counted.bytes);
}
#[test]
fn timestamps_alone_do_not_become_a_truncated_measured_duration() {
    let mut tags = tag_bytes(9, 0, &[0x17, 0, 0, 0, 0]);
    tags.extend(tag_bytes(9, 1000, &[0x27, 1, 255, 255, 216]));
    tags.extend(tag_bytes(9, 9999, &[0x17, 2, 0, 0, 0]));
    assert_eq!(milliseconds(&mut Cursor::new(document(&tags))), None);
    let bytes = document(&tag_bytes(8, u32::MAX, &[0x2f, 1]));
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), None);
    assert_eq!(milliseconds(&mut Cursor::new(document(&[]))), None);
    assert_eq!(
        milliseconds(&mut Cursor::new(document(&tag_bytes(8, 0, &[0x2f, 1])))),
        None
    );
}

#[test]
fn invalid_numbers_and_malformed_metadata_never_become_measured_zero() {
    for value in [-1.0, f64::NAN, f64::INFINITY, i64::MAX as f64] {
        let bytes = document(&tag_bytes(18, 0, &metadata(false, false, value)));
        assert_eq!(milliseconds(&mut Cursor::new(bytes)), None);
    }
    let zero = document(&tag_bytes(18, 0, &metadata(false, false, 0.0)));
    assert_eq!(milliseconds(&mut Cursor::new(zero)), Some(0));
    let valid = document(&tag_bytes(18, 0, &metadata(false, true, 1.0)));
    for end in 0..valid.len() {
        assert!(video(&mut Cursor::new(&valid[..end])).is_none(), "{end}");
    }
    let mut bad = valid.clone();
    *bad.last_mut().unwrap() ^= 1;
    assert!(video(&mut Cursor::new(bad)).is_none());
    let mut bad = valid;
    bad[13] |= 0x20;
    assert!(video(&mut Cursor::new(bad)).is_none());
}
#[test]
fn amf_depth_and_declared_array_sizes_are_bounded() {
    let mut nested = vec![5];
    for _ in 0..10 {
        nested = [vec![3], property("nested", &nested), vec![0, 0, 9]].concat();
    }
    let deep = [
        string("onMetaData"),
        vec![3],
        property("nested", &nested),
        vec![0, 0, 9],
    ]
    .concat();
    assert!(amf::metadata(&deep).is_none());
    let count = [
        string("onMetaData"),
        vec![3],
        property("array", &[10, 255, 255, 255, 255]),
        vec![0, 0, 9],
    ]
    .concat();
    assert!(amf::metadata(&count).is_none());
}
#[test]
fn generated_sorenson_container_reads_metadata_without_decoding_video() {
    let bytes = include_bytes!("../../../../plain-rs/testdata/video-flv1.flv");
    let info = video(&mut Cursor::new(bytes)).unwrap();
    assert_eq!((info.width, info.height), (16, 16));
    assert_eq!(info.duration_ms, 120);
    assert_eq!(info.frame_rate, 25.0);
    assert_eq!(info.title, "FlvFixture");
    assert_eq!(info.artist, "FixtureArtist");
}

#[test]
fn avc_sequence_header_supplies_cropped_dimensions_without_script_metadata_or_fake_duration() {
    let bytes = include_bytes!("../../../../plain-rs/testdata/video-h264-no-metadata.flv");
    let info = video(&mut Cursor::new(bytes)).unwrap();
    assert_eq!((info.width, info.height), (52, 38));
    assert_eq!(info.frame_rate, 25.0);
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), None);
    assert!(info.title.is_empty());
}

#[test]
fn sorenson_and_screen_video_without_metadata_keep_dimensions_and_unknown_timing() {
    for (bytes, width, height) in [
        (
            include_bytes!("../../../../plain-rs/testdata/video-flv1-no-metadata.flv").as_slice(),
            16,
            16,
        ),
        (
            include_bytes!("../../../../plain-rs/testdata/video-screen-no-metadata.flv").as_slice(),
            52,
            38,
        ),
    ] {
        let info = video(&mut Cursor::new(bytes)).unwrap();
        assert_eq!((info.width, info.height), (width, height));
        assert_eq!(info.frame_rate, 0.0);
        assert_eq!(milliseconds(&mut Cursor::new(bytes)), None);
    }
}

#[test]
fn enhanced_and_legacy_hevc_sequence_headers_share_the_hvcc_reader() {
    let bytes = include_bytes!("../../../../plain-rs/testdata/video-hevc-no-metadata.flv");
    let info = video(&mut Cursor::new(bytes)).unwrap();
    assert_eq!((info.width, info.height), (96, 80));
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), None);
    let mut reader = Cursor::new(bytes);
    let first = tag(&mut reader, 13, bytes.len() as u64).unwrap();
    let payload = &bytes[first.data as usize..(first.end - 4) as usize];
    assert_eq!(&payload[1..5], b"hvc1");
    let mut legacy = vec![0x1c, 0, 0, 0, 0];
    legacy.extend_from_slice(&payload[5..]);
    let legacy = document(&tag_bytes(9, 0, &legacy));
    let info = video(&mut Cursor::new(&legacy)).unwrap();
    assert_eq!((info.width, info.height), (96, 80));
    assert_eq!(milliseconds(&mut Cursor::new(legacy)), None);
}
