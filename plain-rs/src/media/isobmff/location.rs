use super::{header, payload, u32_at};
use std::io::{Read, Seek, SeekFrom};

pub(super) fn read(reader: &mut (impl Read + Seek)) -> Option<String> {
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    walk(reader, 0, length, 0)
}

fn walk(reader: &mut (impl Read + Seek), mut offset: u64, end: u64, depth: u8) -> Option<String> {
    if depth > 3 {
        return None;
    }
    while offset < end {
        let (kind, start, next) = header(reader, offset, end)?;
        let value = match &kind {
            b"moov" | b"udta" => walk(reader, start, next, depth + 1),
            b"meta" => metadata(reader, start, next),
            b"\xa9xyz" if next - start <= 1028 => {
                let data = payload(reader, start, next)?;
                let length = usize::from(u16::from_be_bytes(data.get(..2)?.try_into().ok()?));
                text(data.get(4..4usize.checked_add(length)?)?)
            }
            _ => None,
        };
        if value.is_some() {
            return value;
        }
        offset = next;
    }
    None
}

fn text(bytes: &[u8]) -> Option<String> {
    if bytes.len() > 1024 {
        return None;
    }
    let value = std::str::from_utf8(bytes).ok()?.trim_end_matches('\0');
    (!value.is_empty()).then(|| value.to_owned())
}

fn metadata(reader: &mut (impl Read + Seek), start: u64, end: u64) -> Option<String> {
    if end.checked_sub(start)? < 4 {
        return None;
    }
    reader.seek(SeekFrom::Start(start)).ok()?;
    let mut version = [0; 4];
    reader.read_exact(&mut version).ok()?;
    let mut offset = start + if version == [0; 4] { 4 } else { 0 };
    let mut keys = None;
    let mut values = None;
    while offset < end {
        let (kind, start, next) = header(reader, offset, end)?;
        match &kind {
            b"keys" if next - start <= 64 * 1024 => keys = Some(payload(reader, start, next)?),
            b"ilst" => values = Some((start, next)),
            _ => {}
        }
        offset = next;
    }
    let keys = keys?;
    let count = u32_at(&keys, 4)? as usize;
    if count > keys.len().saturating_sub(8) / 8 {
        return None;
    }
    let mut offset = 8usize;
    let mut selected = None;
    for index in 1..=count {
        let size = usize::try_from(u32_at(&keys, offset)?).ok()?;
        if size < 8 {
            return None;
        }
        let end = offset.checked_add(size)?;
        let key = keys.get(offset + 8..end)?;
        if keys.get(offset + 4..offset + 8)? == b"mdta"
            && key == b"com.apple.quicktime.location.ISO6709"
        {
            selected = Some(u32::try_from(index).ok()?.to_be_bytes());
        }
        offset = end;
    }
    let selected = selected?;
    let (mut offset, end) = values?;
    while offset < end {
        let (kind, start, next) = header(reader, offset, end)?;
        if kind == selected {
            let mut offset = start;
            while offset < next {
                let (kind, start, end) = header(reader, offset, next)?;
                if &kind == b"data" && end - start <= 1032 {
                    let data = payload(reader, start, end)?;
                    if u32_at(&data, 0)? == 1 {
                        return text(data.get(8..)?);
                    }
                }
                offset = end;
            }
        }
        offset = next;
    }
    None
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/media/isobmff/location.rs"]
mod tests;
