use super::{atoms, child, header, payload, u32_at, rewrite::{check_cancelled, copy}};
use std::{
    collections::HashSet,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    sync::atomic::AtomicBool,
};

struct Span {
    kind: [u8; 4],
    start: usize,
    data: usize,
    end: usize,
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid MP4 metadata track")
}

fn required<T>(value: Option<T>) -> io::Result<T> {
    value.ok_or_else(invalid)
}

fn span(bytes: &[u8], offset: usize) -> io::Result<Span> {
    let mut reader = Cursor::new(bytes);
    let (kind, data, end) = required(header(&mut reader, offset as u64, bytes.len() as u64))?;
    Ok(Span {
        kind,
        start: offset,
        data: data as usize,
        end: end as usize,
    })
}

fn retire(bytes: &mut [u8], span: &Span) {
    // Free boxes preserve every media offset, including offsets inside fragmented MP4 files.
    bytes[span.start + 4..span.start + 8].copy_from_slice(b"free");
    bytes[span.data..span.end].fill(0);
}

fn metadata_tracks(movie: &mut [u8]) -> io::Result<HashSet<u32>> {
    let mut dropped = HashSet::new();
    let mut ids = HashSet::new();
    let mut offset = 0;
    while offset < movie.len() {
        let span = span(movie, offset)?;
        offset = span.end;
        if &span.kind != b"trak" {
            continue;
        }
        let track = &movie[span.data..span.end];
        let track_header = required(child(track, b"tkhd"))?;
        let id_offset = match track_header.first() {
            Some(0) => 12,
            Some(1) => 20,
            _ => return Err(invalid()),
        };
        let id = required(u32_at(track_header, id_offset))?;
        if id == 0 || !ids.insert(id) {
            return Err(invalid());
        }
        let Some(description) = child(track, b"mdia")
            .and_then(|bytes| child(bytes, b"minf"))
            .and_then(|bytes| child(bytes, b"stbl"))
            .and_then(|bytes| child(bytes, b"stsd"))
        else {
            continue;
        };
        let entries = required(atoms(required(description.get(8..))?))?;
        if required(u32_at(description, 4))? as usize != entries.len() {
            return Err(invalid());
        }
        if !entries.iter().any(|entry| &entry.kind == b"mebx") {
            continue;
        }
        dropped.insert(id);
        retire(movie, &span);
    }
    let mut offset = 0;
    while offset < movie.len() {
        let container = span(movie, offset)?;
        offset = container.end;
        if &container.kind != b"mvex" {
            continue;
        }
        let bytes = &mut movie[container.data..container.end];
        let mut offset = 0;
        while offset < bytes.len() {
            let entry = span(bytes, offset)?;
            offset = entry.end;
            if &entry.kind == b"trex"
                && dropped.contains(&required(u32_at(&bytes[entry.data..entry.end], 4))?)
            {
                retire(bytes, &entry);
            }
        }
    }
    Ok(dropped)
}

fn retire_fragment_tracks(bytes: &mut [u8], dropped: &HashSet<u32>) -> io::Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let span = span(bytes, offset)?;
        offset = span.end;
        if &span.kind != b"traf" {
            continue;
        }
        let track = required(child(&bytes[span.data..span.end], b"tfhd"))?;
        if dropped.contains(&required(u32_at(track, 4))?) {
            retire(bytes, &span);
        }
    }
    Ok(())
}

fn retire_random_access_tracks(bytes: &mut [u8], dropped: &HashSet<u32>) -> io::Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let span = span(bytes, offset)?;
        offset = span.end;
        if &span.kind == b"tfra"
            && dropped.contains(&required(u32_at(&bytes[span.data..span.end], 4))?)
        {
            retire(bytes, &span);
        }
    }
    Ok(())
}

pub fn remove_mebx_tracks(
    reader: &mut (impl Read + Seek),
    writer: &mut (impl Write + Seek),
    cancelled: &AtomicBool,
) -> io::Result<bool> {
    let length = reader.seek(SeekFrom::End(0))?;
    let mut offset = 0;
    let mut movie = None;
    while offset < length {
        check_cancelled(cancelled)?;
        let (kind, start, end) = required(header(reader, offset, length))?;
        if &kind == b"moov" {
            if movie.is_some() {
                return Err(invalid());
            }
            movie = Some(required(payload(reader, start, end))?);
        }
        offset = end;
    }
    let mut movie = required(movie)?;
    let dropped = metadata_tracks(&mut movie)?;
    if dropped.is_empty() {
        return Ok(false);
    }
    writer.seek(SeekFrom::Start(0))?;
    offset = 0;
    while offset < length {
        check_cancelled(cancelled)?;
        let (kind, start, end) = required(header(reader, offset, length))?;
        let rewritten = match &kind {
            b"moov" => Some(std::mem::take(&mut movie)),
            b"moof" => {
                let mut bytes = required(payload(reader, start, end))?;
                retire_fragment_tracks(&mut bytes, &dropped)?;
                Some(bytes)
            }
            b"mfra" => {
                let mut bytes = required(payload(reader, start, end))?;
                retire_random_access_tracks(&mut bytes, &dropped)?;
                Some(bytes)
            }
            b"sidx" => {
                let bytes = required(payload(reader, start, end))?;
                if dropped.contains(&required(u32_at(&bytes, 4))?) {
                    reader.seek(SeekFrom::Start(offset))?;
                    let mut bytes = vec![0; (end - offset) as usize];
                    reader.read_exact(&mut bytes)?;
                    let span = Span {
                        kind,
                        start: 0,
                        data: (start - offset) as usize,
                        end: bytes.len(),
                    };
                    retire(&mut bytes, &span);
                    check_cancelled(cancelled)?;
                    writer.write_all(&bytes)?;
                    offset = end;
                    continue;
                }
                None
            }
            _ => None,
        };
        reader.seek(SeekFrom::Start(offset))?;
        let copy_length = if rewritten.is_some() {
            start - offset
        } else {
            end - offset
        };
        copy(reader, writer, copy_length, cancelled)?;
        if let Some(bytes) = rewritten {
            check_cancelled(cancelled)?;
            writer.write_all(&bytes)?;
        }
        offset = end;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/isobmff/metadata_tracks.rs"]
mod tests;
