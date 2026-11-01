use super::*;
use std::io::Cursor;

fn adts(rate: u8, blocks: u8, crc: bool, size: usize) -> Vec<u8> {
    let mut bytes = vec![0; size];
    bytes[..7].copy_from_slice(&[
        0xff,
        if crc { 0xf0 } else { 0xf1 },
        0x40 | (rate << 2),
        0x80 | ((size >> 11) as u8 & 3),
        (size >> 3) as u8,
        ((size & 7) as u8) << 5 | 0x1f,
        0xfc | blocks,
    ]);
    bytes
}
fn amr(wide: bool, kinds: &[(u8, usize)]) -> Vec<u8> {
    let mut bytes = if wide {
        b"#!AMR-WB\n".to_vec()
    } else {
        b"#!AMR\n".to_vec()
    };
    for (kind, size) in kinds {
        bytes.push((kind << 3) | 4);
        bytes.resize(bytes.len() + size - 1, 0);
    }
    bytes
}

#[test]
fn adts_counts_variable_frame_lengths_blocks_crc_and_sample_rates_without_rounding_per_frame() {
    let mut bytes = adts(4, 0, false, 37);
    bytes.extend(adts(3, 0, true, 29));
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), Some(44));
    let bytes = adts(3, 0, false, 2000).repeat(1000);
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), Some(21_333));
    assert_eq!(
        milliseconds(&mut Cursor::new(adts(3, 3, true, 91))),
        Some(85)
    );
}

#[test]
fn adts_supports_id3_prefixes_and_bounded_descriptors_without_reading_audio_payloads() {
    let mut audio = b"ID3\x04\0\0\0\0\0\x04".to_vec();
    audio.extend([0; 4]);
    audio.extend(adts(3, 0, false, 2000).repeat(1000));
    let mut bytes = b"outside-prefix".to_vec();
    let offset = bytes.len() as u64;
    bytes.extend_from_slice(&audio);
    bytes.extend(b"outside-suffix");
    let mut counted = Counted {
        reader: Cursor::new(bytes),
        read: 0,
    };
    let mut range =
        crate::utils::range_reader::RangeReader::new(&mut counted, offset, audio.len() as u64)
            .unwrap();
    assert_eq!(milliseconds(&mut range), Some(21_333));
    drop(range);
    assert!(counted.read < 10_000);
}

struct Counted {
    reader: Cursor<Vec<u8>>,
    read: usize,
}
impl Read for Counted {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = self.reader.read(bytes)?;
        self.read += count;
        Ok(count)
    }
}
impl Seek for Counted {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        self.reader.seek(from)
    }
}

#[test]
fn amr_counts_narrow_wide_silence_and_lost_frames_and_ignores_padding() {
    assert_eq!(
        milliseconds(&mut Cursor::new(amr(
            false,
            &[(0, 13), (7, 32), (8, 6), (15, 1)]
        ))),
        Some(80)
    );
    assert_eq!(
        milliseconds(&mut Cursor::new(amr(
            true,
            &[(0, 18), (1, 24), (9, 6), (14, 1), (15, 1)]
        ))),
        Some(100)
    );
    let mut bytes = amr(false, &[(7, 32)]);
    bytes[6] |= 0x83;
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), Some(20));
    let mut bytes = b"#!AMR_MC1.0\n".to_vec();
    bytes.extend(2u32.to_be_bytes());
    bytes.extend([15 << 3; 4]);
    assert_eq!(milliseconds(&mut Cursor::new(&bytes)), Some(40));
    bytes.pop();
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), None);
    assert_eq!(milliseconds(&mut Cursor::new(b"#!AMR\n")), Some(0));
}

#[test]
fn unsupported_reserved_and_truncated_frames_are_unknown_instead_of_a_partial_duration() {
    let mut truncated = adts(4, 0, false, 80);
    truncated.pop();
    for bytes in [
        truncated,
        adts(15, 0, false, 80),
        amr(false, &[(9, 6)]),
        b"unknown".to_vec(),
    ] {
        assert_eq!(milliseconds(&mut Cursor::new(bytes)), None);
    }
    let mut bytes = amr(true, &[(8, 61)]);
    bytes.pop();
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), None);
}
