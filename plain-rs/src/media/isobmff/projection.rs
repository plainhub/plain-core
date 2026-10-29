use super::header;
use std::io::{Read, Seek, SeekFrom};

const LIMIT: usize = 4 * 1024 * 1024;

pub(super) fn movie(reader: &mut (impl Read + Seek), start: u64, end: u64) -> Option<Vec<u8>> {
    children(reader, start, end, b"moov", 0, false)
}
pub(super) fn sample_movie(
    reader: &mut (impl Read + Seek),
    start: u64,
    end: u64,
) -> Option<Vec<u8>> {
    children(reader, start, end, b"moov", 0, true)
}
fn bytes(
    reader: &mut (impl Read + Seek),
    start: u64,
    length: u64,
    limit: usize,
) -> Option<Vec<u8>> {
    let length = usize::try_from(length).ok()?;
    if length > limit {
        return None;
    }
    reader.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).ok()?;
    Some(bytes)
}
fn atom(output: &mut Vec<u8>, kind: [u8; 4], data: &[u8], limit: usize) -> Option<()> {
    let size = data.len().checked_add(8)?;
    if output.len().checked_add(size)? > limit {
        return None;
    }
    output.extend(u32::try_from(size).ok()?.to_be_bytes());
    output.extend(kind);
    output.extend(data);
    Some(())
}
fn children(
    reader: &mut (impl Read + Seek),
    mut start: u64,
    end: u64,
    parent: &[u8; 4],
    depth: u8,
    sample_tables: bool,
) -> Option<Vec<u8>> {
    if depth > 10 {
        return None;
    }
    let limit = if sample_tables {
        super::MAX_BOX_BYTES as usize
    } else {
        LIMIT
    };
    let mut output = Vec::new();
    while start < end {
        let (kind, body, next) = header(reader, start, end)?;
        start = next;
        let length = next - body;
        if sample_tables
            && parent != b"stsd"
            && !matches!(
                &kind,
                b"trak"
                    | b"mdia"
                    | b"minf"
                    | b"stbl"
                    | b"tkhd"
                    | b"mdhd"
                    | b"hdlr"
                    | b"stsd"
                    | b"stts"
                    | b"stsc"
                    | b"stsz"
                    | b"stss"
                    | b"stco"
                    | b"co64"
                    | b"mvex"
                    | b"trex"
                    | b"avcC"
            )
        {
            continue;
        }
        let data = if parent == b"stsd" {
            if sample_tables && &kind == b"avc1" {
                if length < 78 {
                    return None;
                }
                let mut data = bytes(reader, body, 78, limit)?;
                data.extend(children(
                    reader,
                    body + 78,
                    next,
                    &kind,
                    depth + 1,
                    sample_tables,
                )?);
                Some(data)
            } else {
                Some(bytes(reader, body, length.min(28), limit)?)
            }
        } else if parent == b"ilst" {
            let index = u32::from_be_bytes(kind);
            if (1..=8192).contains(&index)
                || matches!(
                    &kind,
                    b"\xa9nam"
                        | b"\xa9ART"
                        | b"\xa9alb"
                        | b"\xa9gen"
                        | b"gnre"
                        | b"\xa9wrt"
                        | b"\xa9day"
                )
            {
                Some(children(
                    reader,
                    body,
                    next,
                    &kind,
                    depth + 1,
                    sample_tables,
                )?)
            } else {
                None
            }
        } else {
            match &kind {
                b"trak" | b"mdia" | b"minf" | b"stbl" | b"udta" | b"ilst" => Some(children(
                    reader,
                    body,
                    next,
                    &kind,
                    depth + 1,
                    sample_tables,
                )?),
                b"meta" => {
                    if length < 4 {
                        return None;
                    }
                    let prefix = bytes(reader, body, 4, limit)?;
                    let mut data = Vec::new();
                    let offset = if prefix == [0; 4] {
                        data.extend([0; 4]);
                        4
                    } else {
                        0
                    };
                    data.extend(children(
                        reader,
                        body + offset,
                        next,
                        &kind,
                        depth + 1,
                        sample_tables,
                    )?);
                    Some(data)
                }
                b"stsd" => {
                    if length < 8 {
                        return None;
                    }
                    let mut data = bytes(reader, body, 8, limit)?;
                    data.extend(children(
                        reader,
                        body + 8,
                        next,
                        &kind,
                        depth + 1,
                        sample_tables,
                    )?);
                    Some(data)
                }
                b"tkhd" => Some(bytes(reader, body, length.min(96), limit)?),
                b"mvhd" | b"mdhd" => Some(bytes(reader, body, length.min(32), limit)?),
                b"hdlr" => Some(bytes(reader, body, length.min(12), limit)?),
                b"stts" => Some(bytes(reader, body, length, limit)?),
                b"mvex" if sample_tables => Some(children(
                    reader,
                    body,
                    next,
                    &kind,
                    depth + 1,
                    sample_tables,
                )?),
                b"stsc" | b"stsz" | b"stss" | b"stco" | b"co64" | b"trex" | b"avcC"
                    if sample_tables =>
                {
                    Some(bytes(reader, body, length, limit)?)
                }
                b"titl" | b"perf" | b"albm" | b"gnre" | b"auth" | b"keys" | b"data"
                | b"\xa9nam" | b"\xa9ART" | b"\xa9alb" | b"\xa9gen" | b"\xa9wrt" | b"\xa9day"
                    if length <= 64 * 1024 =>
                {
                    Some(bytes(reader, body, length, limit)?)
                }
                _ => None,
            }
        };
        if let Some(data) = data {
            atom(&mut output, kind, &data, limit)?;
        }
    }
    Some(output)
}
