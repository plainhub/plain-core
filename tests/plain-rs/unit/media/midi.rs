use super::*;
use std::io::Cursor;

fn variable(mut value: u32) -> Vec<u8> {
    let mut bytes = vec![(value & 127) as u8];
    while value > 127 {
        value >>= 7;
        bytes.push((value & 127) as u8 | 128);
    }
    bytes.reverse();
    bytes
}
fn file(format: u16, division: u16, tracks: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"MThd\0\0\0\x06".to_vec();
    bytes.extend(format.to_be_bytes());
    bytes.extend((tracks.len() as u16).to_be_bytes());
    bytes.extend(division.to_be_bytes());
    for track in tracks {
        bytes.extend(b"MTrk");
        bytes.extend((track.len() as u32).to_be_bytes());
        bytes.extend(track);
    }
    bytes
}
fn end_after(ticks: u32) -> Vec<u8> {
    [variable(ticks), vec![0xff, 0x2f, 0]].concat()
}

#[test]
fn ticks_tempo_changes_and_running_status_produce_one_continuous_duration() {
    let track = [
        vec![0, 0x90, 60, 100],
        variable(480),
        vec![61, 0],
        end_after(480),
    ]
    .concat();
    assert_eq!(
        milliseconds(&mut Cursor::new(file(0, 480, &[track]))),
        Some(1000)
    );
    let tempos = [
        vec![0, 0xff, 0x51, 3, 7, 0xa1, 0x20],
        variable(480),
        vec![0xff, 0x51, 3, 0x0f, 0x42, 0x40],
        end_after(0),
    ]
    .concat();
    let notes = [
        vec![0, 0x90, 60, 100],
        variable(960),
        vec![0x80, 60, 0],
        end_after(0),
    ]
    .concat();
    let bytes = file(1, 480, &[tempos, notes]);
    assert_eq!(milliseconds(&mut Cursor::new(&bytes)), Some(1500));
    assert_eq!(
        crate::media::probe::probe_duration_reader(&mut Cursor::new(&bytes)),
        Some(1500)
    );
    assert_eq!(
        crate::media::probe::probe_audio_reader(&mut Cursor::new(bytes)).duration_ms,
        1500
    );
}

#[test]
fn smpte_and_large_tick_counts_preserve_units_and_drop_frame_precision() {
    assert_eq!(
        milliseconds(&mut Cursor::new(file(0, 0xe728, &[end_after(1000)]))),
        Some(1000)
    );
    assert_eq!(
        milliseconds(&mut Cursor::new(file(0, 0xe350, &[end_after(2400)]))),
        Some(1001)
    );
    assert_eq!(
        milliseconds(&mut Cursor::new(file(0, 1, &[end_after(0x0fff_ffff)]))),
        Some(134_217_727_500)
    );
}

#[test]
fn broken_chunks_events_running_status_and_undefined_asynchronous_duration_remain_unknown() {
    for bytes in [
        file(2, 480, &[end_after(480)]),
        file(0, 0, &[end_after(480)]),
        file(0, 480, &[vec![0, 60, 100]]),
        file(0, 480, &[vec![0, 0x90, 60]]),
        file(0, 480, &[vec![0x81, 0x80, 0x80, 0x80, 0]]),
        file(0, 480, &[vec![0, 0x90, 60, 100, 0, 0xff, 6, 0, 0, 61, 0]]),
        file(0, 480, &[vec![0, 0x90, 60, 100, 0, 0xf0, 0, 0, 61, 0]]),
        file(0, 480, &[vec![0, 0xff, 0x51, 3, 0, 0, 0]]),
    ] {
        assert_eq!(milliseconds(&mut Cursor::new(bytes)), None);
    }
    let mut bytes = file(0, 480, &[end_after(480)]);
    bytes.pop();
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), None);
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
fn sysex_payloads_are_skipped_and_owned_descriptor_limits_are_preserved() {
    let size = 1024 * 1024usize;
    let mut track = vec![0, 0xf0];
    track.extend(variable(size as u32));
    track.resize(track.len() + size, 1);
    track.extend(end_after(480));
    let midi = file(0, 480, &[track]);
    let mut bytes = b"outside-prefix".to_vec();
    let offset = bytes.len() as u64;
    bytes.extend_from_slice(&midi);
    bytes.extend(b"outside-suffix");
    let mut counted = Counted {
        reader: Cursor::new(bytes),
        read: 0,
    };
    let mut range =
        crate::utils::range_reader::RangeReader::new(&mut counted, offset, midi.len() as u64)
            .unwrap();
    assert_eq!(milliseconds(&mut range), Some(500));
    drop(range);
    assert!(counted.read < 32 * 1024, "read {} bytes", counted.read);
}
