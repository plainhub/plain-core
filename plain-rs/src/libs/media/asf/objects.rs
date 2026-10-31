use super::{HEADER, u16_at, u32_at, u64_at};
use std::io::{Read, Seek, SeekFrom};

pub(super) const HEADER_EXTENSION: [u8; 16] = [
    0xb5, 3, 0xbf, 0x5f, 0x2e, 0xa9, 0xcf, 0x11, 0x8e, 0xe3, 0, 0xc0, 0x0c, 0x20, 0x53, 0x65,
];
pub(super) const METADATA: [u8; 16] = [
    0xea, 0xcb, 0xf8, 0xc5, 0xaf, 0x5b, 0x77, 0x48, 0x84, 0x67, 0xaa, 0x8c, 0x44, 0xfa, 0x4c, 0xca,
];
pub(super) const METADATA_LIBRARY: [u8; 16] = [
    0x94, 0x1c, 0x23, 0x44, 0x98, 0x94, 0xd1, 0x49, 0xa1, 0x41, 0x1d, 0x13, 0x4e, 0x45, 0x70, 0x54,
];

pub(super) fn position(reader: &mut (impl Read + Seek), offset: u64) -> Option<()> {
    if reader.stream_position().ok()? != offset {
        reader.seek(SeekFrom::Start(offset)).ok()?;
    }
    Some(())
}

pub(super) struct Object {
    pub guid: [u8; 16],
    pub data: u64,
    pub end: u64,
    pub size: u64,
}

pub(super) fn visit<R: Read + Seek>(
    reader: &mut R,
    extensions: bool,
    mut visitor: impl FnMut(&mut R, Object) -> Option<bool>,
) -> Option<()> {
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    reader.seek(SeekFrom::Start(0)).ok()?;
    let mut header = [0; 30];
    reader.read_exact(&mut header).ok()?;
    let end = u64_at(&header, 16)?;
    let count = u32_at(&header, 24)?;
    if header[..16] != HEADER
        || end < 30
        || end > length
        || u64::from(count) > (end - 30) / 24
        || count > 100_000
    {
        return None;
    }
    let mut ranges = vec![(30u64, end, Some(count))];
    let mut remaining = 100_000usize;
    while let Some((offset, end, count)) = ranges.pop() {
        if count == Some(0) {
            continue;
        }
        if offset == end {
            if count.is_some_and(|count| count > 0) {
                return None;
            }
            continue;
        }
        if end.checked_sub(offset)? < 24 {
            return None;
        }
        remaining = remaining.checked_sub(1)?;
        position(reader, offset)?;
        let mut header = [0; 24];
        reader.read_exact(&mut header).ok()?;
        let size = u64_at(&header, 16)?;
        let next = offset.checked_add(size)?;
        if size < 24 || next > end {
            return None;
        }
        ranges.push((next, end, count.map(|value| value - 1)));
        let guid: [u8; 16] = header[..16].try_into().ok()?;
        if extensions && guid == HEADER_EXTENSION {
            if count.is_none() || size < 46 {
                return None;
            }
            let mut extension = [0; 22];
            reader.read_exact(&mut extension).ok()?;
            if u16_at(&extension, 16)? != 6 || u64::from(u32_at(&extension, 18)?) != size - 46 {
                return None;
            }
            ranges.push((offset + 46, next, None));
        } else if !visitor(
            reader,
            Object {
                guid,
                data: offset + 24,
                end: next,
                size,
            },
        )? {
            return Some(());
        }
    }
    Some(())
}
