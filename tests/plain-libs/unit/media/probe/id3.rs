use super::*;
use std::io::Cursor;

fn frame(version: u8, id: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut result = id.to_vec();
    if version == 2 {
        result.extend_from_slice(&(payload.len() as u32).to_be_bytes()[1..]);
    } else {
        let size = payload.len() as u32;
        if version == 4 {
            for shift in [21, 14, 7, 0] {
                result.push(((size >> shift) & 127) as u8);
            }
        } else {
            result.extend(size.to_be_bytes());
        }
        result.extend([0, 0]);
    }
    result.extend(payload);
    result
}

fn tag(version: u8, frames: &[u8], flags: u8) -> Vec<u8> {
    let mut result = vec![b'I', b'D', b'3', version, 0, flags];
    for shift in [21, 14, 7, 0] {
        result.push(((frames.len() >> shift) & 127) as u8);
    }
    result.extend(frames);
    result
}

struct Counted {
    cursor: Cursor<Vec<u8>>,
    reads: Vec<std::ops::Range<u64>>,
}
impl Read for Counted {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let start = self.cursor.position();
        let count = self.cursor.read(bytes)?;
        self.reads.push(start..start + count as u64);
        Ok(count)
    }
}
impl Seek for Counted {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(from)
    }
}

#[test]
fn selected_frames_keep_their_encoding_and_cover_bytes_are_never_read() {
    for version in [2, 3, 4] {
        let title_id = if version == 2 {
            b"TT2".as_slice()
        } else {
            b"TIT2".as_slice()
        };
        let cover_id = if version == 2 {
            b"PIC".as_slice()
        } else {
            b"APIC".as_slice()
        };
        let title = frame(version, title_id, b"\0Title");
        let cover = frame(version, cover_id, &vec![7; 2 * 1024 * 1024]);
        let cover_start = 10 + title.len() + if version == 2 { 6 } else { 10 };
        let cover_end = 10 + title.len() + cover.len();
        let mut frames = title.clone();
        frames.extend(cover);
        frames.extend(vec![0; 1024 * 1024]);
        let mut bytes = tag(version, &frames, 0);
        bytes.extend(b"audio");
        let mut source = Counted {
            cursor: Cursor::new(bytes),
            reads: Vec::new(),
        };
        let mut projected = reader(&mut source, true).unwrap();
        let mut actual = Vec::new();
        projected.read_to_end(&mut actual).unwrap();
        let mut expected = tag(version, &title, 0);
        expected.extend(b"audio");
        assert_eq!(actual, expected);
        assert_eq!(
            projected.seek(SeekFrom::End(-2)).unwrap(),
            expected.len() as u64 - 2
        );
        let mut last = [0; 2];
        projected.read_exact(&mut last).unwrap();
        assert_eq!(&last, b"io");
        projected.seek(SeekFrom::Start(0)).unwrap();
        let mut first = [0; 3];
        projected.read_exact(&mut first).unwrap();
        assert_eq!(&first, b"ID3");
        assert!(projected.seek(SeekFrom::End(1)).is_err());
        drop(projected);
        assert!(
            source
                .reads
                .iter()
                .all(|range| range.end <= cover_start as u64 || range.start >= cover_end as u64)
        );
        assert!(
            source
                .reads
                .iter()
                .map(|range| range.end - range.start)
                .sum::<u64>()
                < 128
        );
    }
}

#[test]
fn duration_input_discards_all_leading_tags_and_handles_tag_footers() {
    let mut tagged = tag(3, &frame(3, b"TIT2", b"\0Ignored"), 0);
    let mut second = tag(4, &[0; 20], 16);
    second.extend(b"3DI\x04\0\x10\0\0\0\x14");
    tagged.extend(second);
    tagged.extend(b"audio");
    let mut projected = reader(Cursor::new(tagged), false).unwrap();
    let mut bytes = Vec::new();
    projected.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"audio");
}

#[test]
fn uncommon_tag_flags_and_malformed_lengths_keep_the_original_input_for_the_decoder() {
    let mut malformed = frame(3, b"TIT2", b"\0Title");
    malformed[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
    for mut original in [
        tag(3, &frame(3, b"TIT2", b"\0Title"), 128),
        tag(3, &malformed, 0),
    ] {
        original.extend(b"audio");
        let mut projected = reader(Cursor::new(original.clone()), true).unwrap();
        let mut bytes = Vec::new();
        projected.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, original);
    }
}

#[test]
fn lofty_still_decodes_original_text_encodings_and_duration_after_sparse_projection() {
    let audio = include_bytes!("../../../../../plain-rs/testdata/audio-untagged.mp3");
    let header_size = synchsafe(&audio[6..10]).unwrap() as usize;
    for version in [3, 4] {
        let mut title = vec![1, 255, 254];
        for unit in "Studio 曲".encode_utf16().chain([0]) {
            title.extend(unit.to_le_bytes());
        }
        let mut frames = frame(version, b"TIT2", &title);
        frames.extend(frame(version, b"APIC", &vec![1; 1024 * 1024]));
        frames.extend(frame(version, b"TPE1", b"\0Composer"));
        let mut bytes = tag(version, &frames, 0);
        bytes.extend(&audio[10 + header_size..]);
        let metadata = super::super::probe_audio_reader(&mut Cursor::new(bytes));
        assert!(metadata.parsed);
        assert_eq!(metadata.title, "Studio 曲");
        assert_eq!(metadata.artist, "Composer");
        assert!(metadata.duration_available);
        assert!(metadata.duration_ms > 0);
    }
}

#[test]
fn album_frames_survive_projection_in_all_supported_id3_versions() {
    for version in [2, 3, 4] {
        let album_id = if version == 2 {
            b"TAL".as_slice()
        } else {
            b"TALB".as_slice()
        };
        let artist_id = if version == 2 {
            b"TP2".as_slice()
        } else {
            b"TPE2".as_slice()
        };
        let mut frames = frame(version, album_id, b"\0Album");
        frames.extend(frame(version, artist_id, b"\0Album Artist"));
        let mut expected = tag(version, &frames, 0);
        let picture_id = if version == 2 {
            b"PIC".as_slice()
        } else {
            b"APIC".as_slice()
        };
        let picture = frame(version, picture_id, &vec![7; 2 * 1024 * 1024]);
        let start = 10 + frames.len() + if version == 2 { 6 } else { 10 };
        let end = 10 + frames.len() + picture.len();
        frames.extend(picture);
        let mut bytes = tag(version, &frames, 0);
        bytes.extend(b"audio");
        expected.extend(b"audio");
        let mut source = Counted {
            cursor: Cursor::new(bytes),
            reads: vec![],
        };
        let mut projected = reader(&mut source, true).unwrap();
        assert!(projected.tags_complete);
        let mut actual = vec![];
        projected.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, expected);
        drop(projected);
        assert!(
            source
                .reads
                .iter()
                .all(|range| range.end <= start as u64 || range.start >= end as u64)
        );
    }
}

#[test]
fn oversized_album_text_is_skipped_without_falling_back_to_the_whole_tag() {
    let oversized = frame(3, b"TALB", &vec![7; MAX_TEXT_BYTES + 1]);
    let artist = frame(3, b"TPE2", b"\0Album Artist");
    let mut frames = oversized.clone();
    frames.extend(&artist);
    let mut bytes = tag(3, &frames, 0);
    bytes.extend(b"audio");
    let mut source = Counted {
        cursor: Cursor::new(bytes),
        reads: vec![],
    };
    let mut projected = reader(&mut source, true).unwrap();
    assert!(!projected.tags_complete);
    let mut actual = vec![];
    projected.read_to_end(&mut actual).unwrap();
    let mut expected = tag(3, &artist, 0);
    expected.extend(b"audio");
    assert_eq!(actual, expected);
    drop(projected);
    assert!(
        source
            .reads
            .iter()
            .all(|range| range.end <= 20 || range.start >= (10 + oversized.len()) as u64)
    );
    assert!(
        source
            .reads
            .iter()
            .map(|range| range.end - range.start)
            .sum::<u64>()
            < 200
    );
}
