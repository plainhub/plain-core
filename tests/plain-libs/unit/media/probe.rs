use super::*;
use std::path::Path;
fn mp4_duration_secs(path: &str) -> Option<u32> {
    u32::try_from(crate::media::isobmff::mp4_milliseconds(Path::new(path))? / 1000).ok()
}

// ---------------------------------------------------------------------------
// probe_duration_secs (container walker + lofty)
// ---------------------------------------------------------------------------

#[test]
fn probe_duration_secs_reads_mvhd_for_video_only_mp4() {
    // lofty rejects video-only MP4 ("no audio tracks"); the container walker
    // must not. Covers head-moov and tail-moov (hvc1 remux) fixtures.
    for (name, secs) in [
        ("video-h264-baseline.mp4", 6),
        ("video-h264-65s.mp4", 65),
        ("video-h264-high.mp4", 6),
        ("video-h265.mp4", 3),
        ("video-h265-hvc1.mp4", 3),
    ] {
        let path = format!("{}/../plain-rs/testdata/{name}", env!("CARGO_MANIFEST_DIR"));
        assert_eq!(probe_duration_secs(&path), Some(secs), "{name}");
    }
}

#[test]
fn probe_duration_secs_garbage_and_missing_are_zero() {
    assert_eq!(probe_duration_secs("/nonexistent/x.mp4"), None);
    let dir = crate::temporary::tempdir().unwrap();
    let p = dir.path().join("x.mp4");
    std::fs::write(&p, b"not an mp4 at all").unwrap();
    assert_eq!(probe_duration_secs(p.to_str().unwrap()), None);
    // Truncated moov header (8 bytes claiming more) must not panic or loop.
    std::fs::write(&p, b"\x00\x00\x00\x40moov\x00\x00\x00\x10mvhd").unwrap();
    assert_eq!(probe_duration_secs(p.to_str().unwrap()), None);
}

// ---------------------------------------------------------------------------
// mvhd parsing (synthetic boxes — pure function, byte-level assertions)
// ---------------------------------------------------------------------------

/// One moov child box: header (size + kind) + payload.
fn box_bytes(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = (8 + payload.len()) as u32;
    [size.to_be_bytes().as_slice(), kind, payload].concat()
}

/// mvhd payload (everything past the box header) for version 0 / 1.
fn mvhd_payload_v0(timescale: u32, duration: u32) -> Vec<u8> {
    let mut b = vec![0u8; 4]; // version 0 + flags
    b.extend_from_slice(&[0u8; 8]); // creation + modification (4 + 4)
    b.extend_from_slice(&timescale.to_be_bytes());
    b.extend_from_slice(&duration.to_be_bytes());
    b
}

fn mvhd_payload_v1(timescale: u32, duration: u64) -> Vec<u8> {
    let mut b = vec![1u8, 0, 0, 0]; // version 1 + flags
    b.extend_from_slice(&[0u8; 16]); // creation + modification (8 + 8)
    b.extend_from_slice(&timescale.to_be_bytes());
    b.extend_from_slice(&duration.to_be_bytes());
    b
}

fn duration_from_moov(moov: &[u8]) -> Option<u32> {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("fixture.mp4");
    std::fs::write(&path, box_bytes(b"moov", moov)).unwrap();
    mp4_duration_secs(path.to_str().unwrap())
}

#[test]
fn mvhd_version_0_and_1_read_duration_over_timescale() {
    let moov = box_bytes(b"mvhd", &mvhd_payload_v0(10, 65));
    assert_eq!(duration_from_moov(&moov), Some(6)); // 65/10 → 6 whole secs
    let moov = box_bytes(b"mvhd", &mvhd_payload_v1(1000, 6_500));
    assert_eq!(duration_from_moov(&moov), Some(6));
    // Whole seconds only, truncated.
    let moov = box_bytes(b"mvhd", &mvhd_payload_v0(3, 10));
    assert_eq!(duration_from_moov(&moov), Some(3));
}

#[test]
fn mvhd_found_behind_sibling_boxes() {
    let moov = [
        box_bytes(b"trak", &[0u8; 12]),
        box_bytes(b"mvhd", &mvhd_payload_v0(2, 9)),
        box_bytes(b"trak", &[0u8; 12]),
    ]
    .concat();
    assert_eq!(duration_from_moov(&moov), Some(4));
}

#[test]
fn mvhd_malformed_is_none_never_panic() {
    // Payloadless mvhd (header-only box) must not index out of bounds.
    assert_eq!(duration_from_moov(&box_bytes(b"mvhd", &[])), None);
    // Truncated version-0 payload (timescale present, duration missing).
    assert_eq!(duration_from_moov(&box_bytes(b"mvhd", &[0u8; 16])), None);
    // Zero timescale would divide by zero.
    assert_eq!(
        duration_from_moov(&box_bytes(b"mvhd", &mvhd_payload_v0(0, 10))),
        None
    );
    // No mvhd at all.
    assert_eq!(duration_from_moov(&box_bytes(b"trak", &[0u8; 8])), None);
}

// ---------------------------------------------------------------------------
// probe_media (single-parse audio probe)
// ---------------------------------------------------------------------------

#[test]
fn probe_media_audio_gets_duration_and_tags() {
    let path = format!(
        "{}/../plain-rs/testdata/audio-tagged.mp3",
        env!("CARGO_MANIFEST_DIR")
    );
    let m = probe_media(&path, "audio");
    assert!(m.duration_secs > 0);
    assert_eq!(m.artist, "Hydrate Artist");
    assert_eq!(m.title, "Hydrate Title");
}

#[test]
fn id3v1_gbk_requires_filename_confirmation_and_preserves_western_tags() {
    let directory = crate::temporary::tempdir().unwrap();
    let audio = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-untagged.mp3"),
    )
    .unwrap();
    for (name, title, artist, expected_title, expected_artist) in [
        (
            "歌手-歌曲.mp3",
            &b"\xb8\xe8\xc7\xfa"[..],
            &b"\xb8\xe8\xca\xd6"[..],
            "歌曲",
            "歌手",
        ),
        (
            "western.mp3",
            &b"Caf\xe9"[..],
            &b"Andr\xe9"[..],
            "Café",
            "André",
        ),
        (
            "unrelated.mp3",
            &b"\xb8\xe8\xc7\xfa"[..],
            &b"\xb8\xe8\xca\xd6"[..],
            "¸èÇú",
            "¸èÊÖ",
        ),
        (
            "歌手-其他.mp3",
            &b"\xb8\xe8\xc7\xfa"[..],
            &b"\xb8\xe8\xca\xd6"[..],
            "¸èÇú",
            "¸èÊÖ",
        ),
    ] {
        let mut tag = [0u8; 128];
        tag[..3].copy_from_slice(b"TAG");
        tag[3..3 + title.len()].copy_from_slice(title);
        tag[33..33 + artist.len()].copy_from_slice(artist);
        let path = directory.path().join(name);
        let mut bytes = audio.clone();
        bytes.extend_from_slice(&tag);
        std::fs::write(&path, bytes).unwrap();
        let result = probe_media(path.to_str().unwrap(), "audio");
        assert_eq!(
            (result.title.as_str(), result.artist.as_str()),
            (expected_title, expected_artist),
            "{name}"
        );
    }
}

#[test]
fn legacy_id3v1_encodings_require_matching_filename() {
    let directory = crate::temporary::tempdir().unwrap();
    let audio = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-untagged.mp3"),
    )
    .unwrap();
    for (encoding, artist, title) in [
        (encoding_rs::SHIFT_JIS, "歌手", "日本の歌"),
        (encoding_rs::EUC_KR, "가수", "노래"),
        (encoding_rs::BIG5, "歌手", "繁體歌曲"),
        (encoding_rs::UTF_8, "Björk", "é"),
    ] {
        let (artist_bytes, _, artist_errors) = encoding.encode(artist);
        let (title_bytes, _, title_errors) = encoding.encode(title);
        assert!(!artist_errors && !title_errors);
        let mut tag = [0u8; 128];
        tag[..3].copy_from_slice(b"TAG");
        tag[3..3 + title_bytes.len()].copy_from_slice(&title_bytes);
        tag[33..33 + artist_bytes.len()].copy_from_slice(&artist_bytes);
        let mut bytes = audio.clone();
        bytes.extend_from_slice(&tag);
        let path = directory.path().join(format!("{artist}-{title}.mp3"));
        std::fs::write(&path, &bytes).unwrap();
        let result = probe_media(path.to_str().unwrap(), "audio");
        assert_eq!(
            (result.artist.as_str(), result.title.as_str()),
            (artist, title)
        );
        let path = directory.path().join("unrelated.mp3");
        std::fs::write(&path, &bytes).unwrap();
        let result = probe_media(path.to_str().unwrap(), "audio");
        if encoding != encoding_rs::UTF_8 {
            assert_ne!(
                (result.artist.as_str(), result.title.as_str()),
                (artist, title)
            );
        }
    }
}

#[test]
fn id3v1_utf8_recovers_non_latin_text_without_reinterpreting_western_or_invalid_bytes() {
    let audio = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-untagged.mp3"),
    )
    .unwrap();
    for (title, artist, expected_title, expected_artist) in [
        (
            "サンプルのタイトル".as_bytes(),
            "サンプルのアーティス".as_bytes(),
            "サンプルのタイトル",
            "サンプルのアーティス",
        ),
        (
            "サンプルのタイトル".as_bytes(),
            &b"Andr\xe9"[..],
            "サンプルのタイトル",
            "André",
        ),
        (&b"Caf\xe9"[..], &b"Andr\xe9"[..], "Café", "André"),
        (&b"\xc3\xa9"[..], &b"\xc3\xb1"[..], "Ã©", "Ã±"),
        (
            &b"\xe3\x82"[..],
            &b"\xe3\x82\xff"[..],
            "ã\u{82}",
            "ã\u{82}ÿ",
        ),
    ] {
        let mut tag = [0u8; 128];
        tag[..3].copy_from_slice(b"TAG");
        tag[3..3 + title.len()].copy_from_slice(title);
        tag[33..33 + artist.len()].copy_from_slice(artist);
        tag[63..63 + title.len()].copy_from_slice(title);
        let mut bytes = audio.clone();
        bytes.extend_from_slice(&tag);
        let result = probe_audio_reader(&mut std::io::Cursor::new(bytes));
        assert_eq!(
            (
                result.title.as_str(),
                result.artist.as_str(),
                result.album.as_str()
            ),
            (expected_title, expected_artist, expected_title)
        );
    }
}

#[test]
fn id3v2_tags_keep_priority_over_filename_confirmed_id3v1() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("歌手-歌曲.mp3");
    let mut audio = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-tagged.mp3"),
    )
    .unwrap();
    let mut tag = [0u8; 128];
    tag[..3].copy_from_slice(b"TAG");
    tag[3..7].copy_from_slice(b"\xb8\xe8\xc7\xfa");
    tag[33..37].copy_from_slice(b"\xb8\xe8\xca\xd6");
    audio.extend_from_slice(&tag);
    std::fs::write(&path, audio).unwrap();
    let result = probe_media(path.to_str().unwrap(), "audio");
    assert_eq!(
        (result.title.as_str(), result.artist.as_str()),
        ("Hydrate Title", "Hydrate Artist")
    );
}

#[test]
fn id3v2_gbk_requires_single_byte_frames_and_filename_confirmation() {
    let directory = crate::temporary::tempdir().unwrap();
    let audio = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-untagged.mp3"),
    )
    .unwrap();
    let frame = |id: &[u8; 4], encoding: u8, text: &[u8]| {
        let mut frame = id.to_vec();
        frame.extend_from_slice(&((text.len() + 1) as u32).to_be_bytes());
        frame.extend_from_slice(&[0, 0, encoding]);
        frame.extend_from_slice(text);
        frame
    };
    let title = b"\xb8\xe8\xc7\xfa";
    let artist = b"\xb8\xe8\xca\xd6";
    for (name, encoding, title_bytes, artist_bytes, expected_title, expected_artist) in [
        (
            "歌手 - 歌曲.mp3",
            0,
            title.as_slice(),
            artist.as_slice(),
            "歌曲",
            "歌手",
        ),
        (
            "unrelated.mp3",
            0,
            title.as_slice(),
            artist.as_slice(),
            "¸èÇú",
            "¸èÊÖ",
        ),
        (
            "western.mp3",
            0,
            b"Caf\xe9".as_slice(),
            b"Andr\xe9".as_slice(),
            "Café",
            "André",
        ),
        (
            "歌手 - 歌曲.mp3",
            3,
            "¸èÇú".as_bytes(),
            "¸èÊÖ".as_bytes(),
            "¸èÇú",
            "¸èÊÖ",
        ),
        (
            "歌手 - 歌曲.mp3",
            1,
            b"\xff\xfe\xb8\x00\xe8\x00\xc7\x00\xfa\x00".as_slice(),
            b"\xff\xfe\xb8\x00\xe8\x00\xca\x00\xd6\x00".as_slice(),
            "¸èÇú",
            "¸èÊÖ",
        ),
    ] {
        let frames = [
            frame(b"TIT2", encoding, title_bytes),
            frame(b"TPE1", encoding, artist_bytes),
        ]
        .concat();
        let size = frames.len();
        let mut bytes = vec![b'I', b'D', b'3', 3, 0, 0];
        bytes.extend([
            ((size >> 21) & 127) as u8,
            ((size >> 14) & 127) as u8,
            ((size >> 7) & 127) as u8,
            (size & 127) as u8,
        ]);
        bytes.extend(frames);
        bytes.extend(&audio);
        let mut legacy_tag = [0u8; 128];
        legacy_tag[..3].copy_from_slice(b"TAG");
        legacy_tag[3..7].copy_from_slice(title);
        legacy_tag[33..37].copy_from_slice(artist);
        bytes.extend(legacy_tag);
        let path = directory.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let result = probe_media(path.to_str().unwrap(), "audio");
        assert_eq!(
            (result.title.as_str(), result.artist.as_str()),
            (expected_title, expected_artist),
            "{name}, encoding {encoding}"
        );
    }
}

#[test]
fn probe_media_tagless_audio_is_duration_only() {
    let path = format!(
        "{}/../plain-rs/testdata/audio-untagged.mp3",
        env!("CARGO_MANIFEST_DIR")
    );
    let m = probe_media(&path, "audio");
    assert_eq!(m.duration_secs, 4);
    assert_eq!(m.artist, "");
    assert_eq!(m.title, "");
}

#[test]
fn probe_media_video_only_mp4_gets_duration_not_tags() {
    let path = format!(
        "{}/../plain-rs/testdata/video-h264-baseline.mp4",
        env!("CARGO_MANIFEST_DIR")
    );
    let m = probe_media(&path, "video");
    assert_eq!(m.duration_secs, 6);
    assert_eq!(m.artist, "");
    assert_eq!(m.title, "");
}

#[test]
fn probe_media_missing_file_is_all_zeros() {
    let m = probe_media("/nonexistent/x.mp3", "audio");
    assert_eq!(
        (m.duration_secs, m.artist.as_str(), m.title.as_str()),
        (0, "", "")
    );
}

#[test]
fn container_zero_duration_is_available_while_probe_failure_is_unknown() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("zero.m4a");
    let moov = box_bytes(b"moov", &box_bytes(b"mvhd", &mvhd_payload_v0(1000, 0)));
    std::fs::write(&path, moov).unwrap();
    let zero = probe_media(path.to_str().unwrap(), "audio");
    assert_eq!(zero.duration_secs, 0);
    assert!(zero.duration_available);
    let missing = probe_media(
        directory.path().join("missing.m4a").to_str().unwrap(),
        "audio",
    );
    assert_eq!(missing.duration_secs, 0);
    assert!(!missing.duration_available);
}

#[test]
fn duration_reader_identifies_content_and_keeps_descriptor_ranges_isolated() {
    let fixture = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-untagged.mp3"),
    )
    .unwrap();
    let mut bytes = b"prefix".to_vec();
    let offset = bytes.len() as u64;
    bytes.extend_from_slice(&fixture);
    bytes.extend_from_slice(b"suffix");
    let mut source = std::io::Cursor::new(bytes);
    let mut reader =
        crate::utils::range_reader::RangeReader::new(&mut source, offset, fixture.len() as u64)
            .unwrap();
    let duration = probe_duration_reader(&mut reader).unwrap();
    assert_eq!(duration / 1000, 4);
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("audio-without-extension");
    std::fs::write(&path, fixture).unwrap();
    assert_eq!(
        probe_duration_milliseconds(path.to_str().unwrap()),
        Some(duration)
    );
}

struct CountedAudio {
    reader: std::io::Cursor<Vec<u8>>,
    bytes_read: usize,
}
impl Read for CountedAudio {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = self.reader.read(bytes)?;
        self.bytes_read += count;
        Ok(count)
    }
}
impl Seek for CountedAudio {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        self.reader.seek(from)
    }
}

#[test]
fn duration_only_probing_skips_large_tag_bodies_instead_of_decoding_unused_text() {
    let padding = 1024 * 1024usize;
    let mut bytes = b"ID3\x04\0\0".to_vec();
    for shift in [21, 14, 7, 0] {
        bytes.push(((padding >> shift) & 0x7f) as u8);
    }
    bytes.resize(10 + padding, 0);
    bytes.extend(
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-untagged.mp3"),
        )
        .unwrap(),
    );
    let mut reader = CountedAudio {
        reader: std::io::Cursor::new(bytes),
        bytes_read: 0,
    };
    assert_eq!(probe_duration_reader(&mut reader).unwrap() / 1000, 4);
    assert!(
        reader.bytes_read < 128 * 1024,
        "{} bytes read",
        reader.bytes_read
    );
}

#[test]
fn image_and_document_rows_never_receive_audio_properties_from_their_contents() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-tagged.mp3");
    for kind in ["image", "doc"] {
        let metadata = probe_media(path.to_str().unwrap(), kind);
        assert_eq!(metadata.duration_ms, 0);
        assert!(!metadata.duration_available);
        assert!(!metadata.parsed);
        assert!(metadata.artist.is_empty());
        assert!(metadata.title.is_empty());
    }
}

#[test]
fn audio_tag_reader_sniffs_content_within_the_owned_descriptor_range() {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-tagged.mp3"),
    )
    .unwrap();
    let mut wrapped = b"outside-prefix".to_vec();
    let offset = wrapped.len() as u64;
    wrapped.extend_from_slice(&bytes);
    wrapped.extend_from_slice(b"outside-suffix");
    let mut source = std::io::Cursor::new(wrapped);
    let mut reader =
        crate::utils::range_reader::RangeReader::new(&mut source, offset, bytes.len() as u64)
            .unwrap();
    let facts = probe_audio_reader(&mut reader);
    assert!(facts.parsed);
    assert!(facts.duration_available);
    assert_eq!(facts.title, "Hydrate Title");
    assert_eq!(facts.artist, "Hydrate Artist");
    assert!(facts.duration_ms > 0);
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("wrong-extension.bin");
    std::fs::write(&path, bytes).unwrap();
    let local = probe_media(path.to_str().unwrap(), "audio");
    assert_eq!(local.title, facts.title);
    assert_eq!(local.artist, facts.artist);
    assert_eq!(local.duration_ms, facts.duration_ms);
}

#[test]
fn metadata_reader_skips_cover_payloads_while_preserving_text_and_duration() {
    fn frame(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = kind.to_vec();
        bytes.extend((payload.len() as u32).to_be_bytes());
        bytes.extend([0, 0]);
        bytes.extend(payload);
        bytes
    }
    let mut picture = b"\0image/jpeg\0\x03\0".to_vec();
    picture.resize(1024 * 1024, 7);
    let mut frames = frame(b"TIT2", b"\0Cover Fixture");
    frames.extend(frame(b"APIC", &picture));
    let mut bytes = b"ID3\x03\0\0".to_vec();
    for shift in [21, 14, 7, 0] {
        bytes.push(((frames.len() >> shift) & 127) as u8);
    }
    bytes.extend(frames);
    let audio = include_bytes!("../../../../plain-rs/testdata/audio-untagged.mp3");
    let size = audio[6..10]
        .iter()
        .fold(0usize, |size, byte| (size << 7) | usize::from(*byte));
    bytes.extend(&audio[10 + size..]);
    let mut reader = CountedAudio {
        reader: std::io::Cursor::new(bytes),
        bytes_read: 0,
    };
    let metadata = probe_audio_reader(&mut reader);
    assert!(metadata.parsed);
    assert_eq!(metadata.title, "Cover Fixture");
    assert!(metadata.duration_ms > 0);
    assert!(
        reader.bytes_read < 128 * 1024,
        "read {} bytes",
        reader.bytes_read
    );
}

#[test]
fn audio_probe_preserves_album_and_album_artist_from_the_shared_reader() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-album-tagged.mp3");
    let facts = probe_media(path.to_str().unwrap(), "audio");
    assert!(facts.parsed);
    assert_eq!(facts.title, "Album Fixture");
    assert_eq!(facts.artist, "Track Artist");
    assert_eq!(facts.album, "Shared Album");
    assert_eq!(facts.album_artist, "Album Artist");
    assert!(facts.duration_ms > 0);
}

#[test]
fn oversized_album_tags_keep_duration_but_are_unavailable_for_replacing_stored_tags() {
    let payload = vec![7; 2 * 1024 * 1024];
    let mut frame = b"TALB".to_vec();
    frame.extend((payload.len() as u32).to_be_bytes());
    frame.extend([0, 0]);
    frame.extend(payload);
    let mut bytes = b"ID3\x03\0\0".to_vec();
    for shift in [21, 14, 7, 0] {
        bytes.push(((frame.len() >> shift) & 127) as u8);
    }
    bytes.extend(frame);
    bytes.extend(
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-untagged.mp3"),
        )
        .unwrap(),
    );
    let mut source = CountedAudio {
        reader: std::io::Cursor::new(bytes),
        bytes_read: 0,
    };
    let facts = probe_audio_reader(&mut source);
    assert!(!facts.parsed);
    assert!(facts.duration_available);
    assert!(facts.duration_ms > 0);
    assert!(facts.album.is_empty());
    assert!(
        source.bytes_read < 128 * 1024,
        "{} bytes read",
        source.bytes_read
    );
}
