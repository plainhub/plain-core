use super::*;
use std::io::Cursor;

fn element(id: u32, bytes: &[u8]) -> Vec<u8> {
    let width = (1..=8)
        .find(|width| (bytes.len() as u64) < (1u64 << (7 * width)) - 1)
        .unwrap();
    let mut result = id
        .to_be_bytes()
        .into_iter()
        .skip_while(|byte| *byte == 0)
        .collect::<Vec<_>>();
    let size = ((1u64 << (7 * width)) | bytes.len() as u64).to_be_bytes();
    result.extend_from_slice(&size[8 - width..]);
    result.extend_from_slice(bytes);
    result
}
fn unsigned(id: u32, value: u64) -> Vec<u8> {
    element(id, &value.to_be_bytes())
}
fn document(doc_type: &str, body: &[u8], unknown: bool) -> Vec<u8> {
    let mut bytes = element(0x1a45dfa3, &element(0x4282, doc_type.as_bytes()));
    if unknown {
        bytes.extend(0x18538067u32.to_be_bytes());
        bytes.extend([1, 255, 255, 255, 255, 255, 255, 255]);
        bytes.extend(body);
    } else {
        bytes.extend(element(0x18538067, body));
    }
    bytes
}
fn segment_info(scale: u64, ticks: Option<f64>) -> Vec<u8> {
    let mut bytes = unsigned(0x2ad7b1, scale);
    if let Some(ticks) = ticks {
        bytes.extend(element(0x4489, &ticks.to_be_bytes()));
    }
    bytes.extend(element(0x7ba9, b"Container Title"));
    bytes.extend(element(0x4461, &(-1i64).to_be_bytes()));
    element(0x1549a966, &bytes)
}
fn video_track(uid: u64, enabled: bool) -> Vec<u8> {
    element(
        0xae,
        &[
            unsigned(0x73c5, uid),
            unsigned(0x83, 1),
            unsigned(0xb9, u64::from(enabled)),
            unsigned(0x23e383, 40_000_000),
            element(0xe0, &[unsigned(0xb0, 640), unsigned(0xba, 360)].concat()),
        ]
        .concat(),
    )
}
fn simple_tag(name: &str, value: &[u8]) -> Vec<u8> {
    element(
        0x67c8,
        &[element(0x45a3, name.as_bytes()), element(0x4487, value)].concat(),
    )
}
fn tag(uid: u64, name: &str, value: &[u8]) -> Vec<u8> {
    element(
        0x7373,
        &[
            simple_tag(name, value),
            element(0x63c0, &unsigned(0x63c5, uid)),
        ]
        .concat(),
    )
}
fn body() -> Vec<u8> {
    [
        segment_info(500_000, Some(2500.5)),
        element(
            0x1654ae6b,
            &[video_track(6, false), video_track(7, true)].concat(),
        ),
    ]
    .concat()
}

#[test]
fn matroska_and_webm_share_track_selection_time_scale_and_metadata() {
    for doc_type in ["matroska", "webm"] {
        for unknown in [false, true] {
            let mut body = body();
            body.extend(element(
                0x1254c367,
                &[
                    tag(0, "TITLE", b"Global Title"),
                    tag(9, "ARTIST", b"Unrelated Audio Artist"),
                    tag(7, "ARTIST", b"Selected Artist"),
                    tag(0, "COMPOSER", b"Composer"),
                ]
                .concat(),
            ));
            let bytes = document(doc_type, &body, unknown);
            let info = super::super::video_info::read(&mut Cursor::new(&bytes)).unwrap();
            assert_eq!((info.width, info.height, info.rotation), (640, 360, 0));
            assert_eq!(info.duration_ms, 1250);
            assert_eq!(info.frame_rate, 25.0);
            assert_eq!(info.title, "Global Title");
            assert_eq!(info.artist, "Selected Artist");
            assert_eq!(info.composer, "Composer");
            assert_eq!(
                info.raw_taken_at.as_deref(),
                Some("2000-12-31T23:59:59.999999999+00:00")
            );
            assert_eq!(milliseconds(&mut Cursor::new(&bytes)), Some(1250));
            assert_eq!(
                super::super::probe::probe_duration_reader(&mut Cursor::new(&bytes)),
                Some(1250)
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
fn known_and_unknown_clusters_skip_payloads_and_preserve_tags_after_the_cluster() {
    for unknown in [false, true] {
        let mut cluster = unsigned(0xe7, 0);
        cluster.extend(element(0xa3, &vec![7; 4 * 1024 * 1024]));
        let cluster = if unknown {
            [0x1f43b675u32.to_be_bytes().to_vec(), vec![255], cluster].concat()
        } else {
            element(0x1f43b675, &cluster)
        };
        let mut body = body();
        body.extend(cluster);
        body.extend(element(0x1941a469, &vec![9; 2 * 1024 * 1024]));
        body.extend(element(0x1254c367, &tag(7, "TITLE", b"After Cluster")));
        let bytes = document("webm", &body, true);
        let length = bytes.len();
        let prefix = b"outside";
        let mut wrapped = prefix.to_vec();
        wrapped.extend(bytes);
        wrapped.extend(prefix);
        let mut counted = Counted {
            inner: Cursor::new(wrapped),
            bytes: 0,
        };
        let mut range = crate::utils::range_reader::RangeReader::new(
            &mut counted,
            prefix.len() as u64,
            length as u64,
        )
        .unwrap();
        let info = video(&mut range).unwrap();
        assert_eq!(info.title, "After Cluster");
        assert_eq!((info.width, info.height), (640, 360));
        drop(range);
        assert!(counted.bytes < 4096, "read {} bytes", counted.bytes);
    }
}
#[test]
fn duration_only_does_not_read_tags_or_tracks_and_unknown_duration_is_not_measured_zero() {
    let mut body = body();
    body.extend(element(
        0x1254c367,
        &tag(0, "TITLE", &vec![8; 2 * 1024 * 1024]),
    ));
    let mut counted = Counted {
        inner: Cursor::new(document("matroska", &body, false)),
        bytes: 0,
    };
    assert_eq!(milliseconds(&mut counted), Some(1250));
    assert!(counted.bytes < 256, "read {} bytes", counted.bytes);
    let body = [
        segment_info(1_000_000, None),
        element(0x1654ae6b, &video_track(1, true)),
    ]
    .concat();
    let bytes = document("webm", &body, false);
    assert_eq!(milliseconds(&mut Cursor::new(&bytes)), None);
    assert_eq!(video(&mut Cursor::new(&bytes)).unwrap().duration_ms, 0);
    let zero = document("matroska", &segment_info(1_000_000, Some(0.0)), false);
    assert_eq!(milliseconds(&mut Cursor::new(zero)), Some(0));
}
#[test]
fn malformed_headers_unknown_scalar_sizes_and_invalid_time_values_are_rejected() {
    assert!(video(&mut Cursor::new(document("other", &body(), false))).is_none());
    for ticks in [-1.0, f64::NAN, f64::INFINITY, i64::MAX as f64 * 2.0] {
        let bytes = document("webm", &segment_info(1_000_000, Some(ticks)), false);
        assert_eq!(milliseconds(&mut Cursor::new(bytes)), None);
    }
    let bytes = document("webm", &body(), false);
    for end in 0..bytes.len() {
        assert!(video(&mut Cursor::new(&bytes[..end])).is_none(), "{end}");
    }
    assert!(
        milliseconds(&mut Cursor::new(document(
            "webm",
            &segment_info(0, Some(1.0)),
            false
        )))
        .is_none()
    );
    let mut malformed = 0x1549a966u32.to_be_bytes().to_vec();
    malformed.push(255);
    assert!(video(&mut Cursor::new(document("webm", &malformed, false))).is_none());
}

#[test]
fn generated_vp8_containers_expose_real_metadata_and_time_without_decoding_frames() {
    for (bytes, title) in [
        (
            include_bytes!("../../../../plain-rs/testdata/video-vp8.webm").as_slice(),
            "WebMFixture",
        ),
        (
            include_bytes!("../../../../plain-rs/testdata/video-vp8.mkv").as_slice(),
            "MatroskaFixture",
        ),
    ] {
        let result = video(&mut Cursor::new(bytes)).unwrap();
        assert_eq!((result.width, result.height), (16, 16));
        assert_eq!(result.duration_ms, 120);
        assert_eq!(result.frame_rate, 25.0);
        assert_eq!(result.title, title);
        assert_eq!(result.artist, "FixtureArtist");
    }
}

#[test]
fn track_target_lists_match_any_selected_uid_and_exclude_chapter_metadata() {
    let target = element(0x63c0, &[unsigned(0x63c5, 8), unsigned(0x63c5, 7)].concat());
    let matching = element(
        0x7373,
        &[target, simple_tag("ARTIST", b"Shared Artist")].concat(),
    );
    let chapter = element(
        0x7373,
        &[
            element(0x63c0, &unsigned(0x63c4, 1)),
            simple_tag("TITLE", b"Chapter Title"),
        ]
        .concat(),
    );
    let bytes = document(
        "webm",
        &[body(), element(0x1254c367, &[matching, chapter].concat())].concat(),
        false,
    );
    let result = video(&mut Cursor::new(bytes)).unwrap();
    assert_eq!(result.artist, "Shared Artist");
    assert_eq!(result.title, "Container Title");
}

#[test]
fn hostile_element_counts_and_deep_tags_have_fixed_limits() {
    let mut many = body();
    for _ in 0..100_001 {
        many.extend(element(0xec, &[]));
    }
    assert!(video(&mut Cursor::new(document("webm", &many, true))).is_none());
    let mut nested = simple_tag("TITLE", b"Deep");
    for _ in 0..10 {
        nested = element(0x67c8, &nested);
    }
    let bytes = document(
        "webm",
        &[body(), element(0x1254c367, &element(0x7373, &nested))].concat(),
        false,
    );
    assert!(video(&mut Cursor::new(bytes)).is_none());
}
