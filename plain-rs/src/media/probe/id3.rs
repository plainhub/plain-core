use crate::utils::range_reader::RangeReader;
use std::io::{self, Read, Seek, SeekFrom};

const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_FRAMES: usize = 100_000;

pub(super) struct View<R> {
    prefix: Vec<u8>,
    pub(super) tags_complete: bool,
    tail: RangeReader<R>,
    position: u64,
    length: u64,
}

pub(super) fn reader<R: Read + Seek>(mut source: R, text: bool) -> io::Result<View<R>> {
    let length = source.seek(SeekFrom::End(0))?;
    let (prefix, offset, tags_complete) =
        project(&mut source, length, text)?.unwrap_or_else(|| (Vec::new(), 0, true));
    let tail = RangeReader::new(source, offset, length - offset)?;
    Ok(View {
        length: prefix.len() as u64 + length - offset,
        prefix,
        tags_complete,
        tail,
        position: 0,
    })
}

fn synchsafe(bytes: &[u8]) -> Option<u64> {
    bytes.iter().try_fold(0u64, |value, byte| {
        (byte & 128 == 0).then_some((value << 7) | u64::from(*byte))
    })
}

fn project(
    reader: &mut (impl Read + Seek),
    length: u64,
    text: bool,
) -> io::Result<Option<(Vec<u8>, u64, bool)>> {
    let mut prefix = Vec::new();
    let mut tags_complete = true;
    let mut offset = 0u64;
    for _ in 0..32 {
        if length - offset < 10 {
            break;
        }
        reader.seek(SeekFrom::Start(offset))?;
        let mut header = [0; 10];
        reader.read_exact(&mut header)?;
        if &header[..3] != b"ID3" {
            break;
        }
        if !matches!(header[3], 2..=4) {
            return Ok(None);
        }
        let Some(size) = synchsafe(&header[6..]) else {
            return Ok(None);
        };
        let Some(end) = offset
            .checked_add(10)
            .and_then(|offset| offset.checked_add(size))
        else {
            return Ok(None);
        };
        let footer = u64::from(header[3] == 4 && header[5] & 16 != 0) * 10;
        if end.checked_add(footer).is_none_or(|end| end > length) {
            return Ok(None);
        }
        if text {
            // Global unsynchronization prevents seeking by the decoded frame lengths.
            if header[5] != 0 {
                return Ok(None);
            }
            let Some((frames, complete)) = text_frames(
                reader,
                offset + 10,
                end,
                header[3],
                MAX_TEXT_BYTES.saturating_sub(prefix.len() + 10),
            )?
            else {
                return Ok(None);
            };
            tags_complete &= complete;
            if prefix.len() + frames.len() + 10 > MAX_TEXT_BYTES {
                tags_complete = false;
                offset = end + footer;
                continue;
            }
            for (index, shift) in [21, 14, 7, 0].into_iter().enumerate() {
                header[6 + index] = ((frames.len() >> shift) & 127) as u8;
            }
            prefix.extend(header);
            prefix.extend(frames);
        }
        offset = end + footer;
    }
    Ok(Some((prefix, offset, tags_complete)))
}

fn text_frames(
    reader: &mut (impl Read + Seek),
    mut offset: u64,
    end: u64,
    version: u8,
    budget: usize,
) -> io::Result<Option<(Vec<u8>, bool)>> {
    let mut selected = Vec::new();
    let mut complete = true;
    let header_len = if version == 2 { 6 } else { 10 };
    let id_len = if version == 2 { 3 } else { 4 };
    for _ in 0..MAX_FRAMES {
        if end - offset < header_len as u64 {
            return Ok(Some((selected, complete)));
        }
        reader.seek(SeekFrom::Start(offset))?;
        let mut header = [0; 10];
        reader.read_exact(&mut header[..header_len])?;
        let id = &header[..id_len];
        if id.iter().all(|byte| *byte == 0) {
            return Ok(Some((selected, complete)));
        }
        if !id
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            return Ok(None);
        }
        let size = if version == 2 {
            u64::from(u32::from_be_bytes([0, header[3], header[4], header[5]]))
        } else if version == 4 {
            let Some(size) = synchsafe(&header[4..8]) else {
                return Ok(None);
            };
            size
        } else {
            u64::from(u32::from_be_bytes(header[4..8].try_into().unwrap()))
        };
        let Some(next) = offset
            .checked_add(header_len as u64)
            .and_then(|offset| offset.checked_add(size))
        else {
            return Ok(None);
        };
        if next > end {
            return Ok(None);
        }
        if matches!(
            id,
            b"TIT2" | b"TPE1" | b"TALB" | b"TPE2" | b"TT2" | b"TP1" | b"TAL" | b"TP2"
        ) {
            if size > MAX_TEXT_BYTES as u64 || selected.len() + header_len + size as usize > budget
            {
                complete = false;
                offset = next;
                continue;
            }
            selected.extend_from_slice(&header[..header_len]);
            let start = selected.len();
            selected.resize(start + size as usize, 0);
            reader.read_exact(&mut selected[start..])?;
        }
        offset = next;
    }
    Ok(None)
}

impl<R: Read> Read for View<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = if self.position < self.prefix.len() as u64 {
            let start = self.position as usize;
            let count = bytes.len().min(self.prefix.len() - start);
            bytes[..count].copy_from_slice(&self.prefix[start..start + count]);
            count
        } else {
            self.tail.read(bytes)?
        };
        self.position += count as u64;
        Ok(count)
    }
}

impl<R: Seek> Seek for View<R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(value) => i128::from(value),
            SeekFrom::Current(value) => i128::from(self.position) + i128::from(value),
            SeekFrom::End(value) => i128::from(self.length) + i128::from(value),
        };
        if position < 0 || position > i128::from(self.length) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Seek outside audio input",
            ));
        }
        let position = position as u64;
        self.tail.seek(SeekFrom::Start(
            position.saturating_sub(self.prefix.len() as u64),
        ))?;
        self.position = position;
        Ok(position)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/media/probe/id3.rs"]
mod tests;
