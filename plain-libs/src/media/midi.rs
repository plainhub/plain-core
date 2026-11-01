use std::io::{BufReader, Read, Seek, SeekFrom};

const MAX_TEMPO_EVENTS: usize = 4 * 1024 * 1024 / std::mem::size_of::<(u64, u32)>();

pub(super) fn milliseconds(reader: &mut (impl Read + Seek)) -> Option<i64> {
    parse(&mut BufReader::new(reader))
}

fn parse(reader: &mut (impl Read + Seek)) -> Option<i64> {
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    reader.seek(SeekFrom::Start(0)).ok()?;
    let mut header = [0; 14];
    reader.read_exact(&mut header).ok()?;
    if &header[..4] != b"MThd" {
        return None;
    }
    let size = u32::from_be_bytes(header[4..8].try_into().ok()?);
    let format = u16::from_be_bytes(header[8..10].try_into().ok()?);
    let tracks = u16::from_be_bytes(header[10..12].try_into().ok()?);
    let division = u16::from_be_bytes(header[12..14].try_into().ok()?);
    if size < 6 || format > 1 || tracks == 0 || (format == 0 && tracks != 1) || division == 0 {
        return None;
    }
    let mut offset = 8u64.checked_add(u64::from(size))?;
    let mut tempos = Vec::new();
    let mut end_tick = 0u64;
    let mut remaining_tracks = tracks;
    while remaining_tracks > 0 {
        if length.checked_sub(offset)? < 8 {
            return None;
        }
        reader.seek(SeekFrom::Start(offset)).ok()?;
        let mut chunk = [0; 8];
        reader.read_exact(&mut chunk).ok()?;
        let size = u64::from(u32::from_be_bytes(chunk[4..].try_into().ok()?));
        offset = offset.checked_add(8)?.checked_add(size)?;
        if offset > length {
            return None;
        }
        if &chunk[..4] == b"MTrk" {
            let mut track = Track {
                reader: &mut *reader,
                remaining: size,
            };
            end_tick = end_tick.max(track.read(&mut tempos)?);
            remaining_tracks -= 1;
        }
    }
    if division & 0x8000 != 0 {
        let frame = (division >> 8) as u8 as i8;
        let ticks_per_frame = u128::from(division & 255);
        if ticks_per_frame == 0 {
            return None;
        }
        let (numerator, denominator) = match frame {
            -24 => (1000, 24),
            -25 => (1000, 25),
            -29 => (1_001_000, 30000),
            -30 => (1000, 30),
            _ => return None,
        };
        return i64::try_from(
            u128::from(end_tick).checked_mul(numerator)? / (denominator * ticks_per_frame),
        )
        .ok();
    }
    tempos.sort_by_key(|(tick, _)| *tick);
    let mut previous = 0;
    let mut tempo = 500_000u128;
    let mut total = 0u128;
    for (tick, next_tempo) in tempos {
        total = total.checked_add(u128::from(tick.checked_sub(previous)?).checked_mul(tempo)?)?;
        previous = tick;
        tempo = u128::from(next_tempo);
    }
    total = total.checked_add(u128::from(end_tick.checked_sub(previous)?).checked_mul(tempo)?)?;
    i64::try_from(total / (u128::from(division) * 1000)).ok()
}

struct Track<'a, R> {
    reader: &'a mut R,
    remaining: u64,
}
impl<R: Read + Seek> Track<'_, R> {
    fn byte(&mut self) -> Option<u8> {
        self.remaining = self.remaining.checked_sub(1)?;
        let mut byte = [0];
        self.reader.read_exact(&mut byte).ok()?;
        Some(byte[0])
    }
    fn variable(&mut self) -> Option<u32> {
        let mut value = 0u32;
        for _ in 0..4 {
            let byte = self.byte()?;
            value = (value << 7) | u32::from(byte & 127);
            if byte < 128 {
                return Some(value);
            }
        }
        None
    }
    fn skip(&mut self, length: u32) -> Option<()> {
        self.remaining = self.remaining.checked_sub(u64::from(length))?;
        self.reader
            .seek(SeekFrom::Current(i64::from(length)))
            .ok()?;
        Some(())
    }
    fn read(&mut self, tempos: &mut Vec<(u64, u32)>) -> Option<u64> {
        let mut tick = 0u64;
        let mut running = 0u8;
        while self.remaining > 0 {
            tick = tick.checked_add(u64::from(self.variable()?))?;
            let byte = self.byte()?;
            let (status, first) = if byte < 128 {
                if running == 0 {
                    return None;
                }
                (running, true)
            } else {
                (byte, false)
            };
            match status {
                0x80..=0xef => {
                    running = status;
                    let count = if matches!(status >> 4, 12 | 13) { 1 } else { 2 };
                    for _ in usize::from(first)..count {
                        if self.byte()? >= 128 {
                            return None;
                        }
                    }
                }
                0xff => {
                    running = 0;
                    let kind = self.byte()?;
                    let length = self.variable()?;
                    match kind {
                        0x2f => {
                            if length != 0 {
                                return None;
                            }
                            return Some(tick);
                        }
                        0x51 => {
                            if length != 3 || tempos.len() >= MAX_TEMPO_EVENTS {
                                return None;
                            }
                            let tempo = (u32::from(self.byte()?) << 16)
                                | (u32::from(self.byte()?) << 8)
                                | u32::from(self.byte()?);
                            if tempo == 0 {
                                return None;
                            }
                            tempos.push((tick, tempo));
                        }
                        _ => self.skip(length)?,
                    }
                }
                0xf0 | 0xf7 => {
                    running = 0;
                    let length = self.variable()?;
                    self.skip(length)?;
                }
                _ => return None,
            }
        }
        Some(tick)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/midi.rs"]
mod tests;
