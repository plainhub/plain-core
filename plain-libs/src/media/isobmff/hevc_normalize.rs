use super::{
    header,
    rewrite::{check_cancelled, copy},
    samples::VideoTrack,
};
use std::{
    io::{self, Read, Seek, SeekFrom, Write},
    sync::atomic::AtomicBool,
};

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid MP4 HEVC sample entry")
}
fn find(
    reader: &mut (impl Read + Seek),
    parent: (u64, u64),
    wanted: &[u8; 4],
) -> io::Result<(u64, u64)> {
    let mut offset = parent.0;
    while offset < parent.1 {
        let (kind, data, end) = header(reader, offset, parent.1).ok_or_else(invalid)?;
        if &kind == wanted {
            return Ok((data, end));
        }
        offset = end;
    }
    Err(invalid())
}

fn entry(reader: &mut (impl Read + Seek), length: u64, description: u32) -> io::Result<(u64, u64)> {
    let movie = find(reader, (0, length), b"moov")?;
    let mut offset = movie.0;
    while offset < movie.1 {
        let (kind, data, end) = header(reader, offset, movie.1).ok_or_else(invalid)?;
        offset = end;
        if &kind != b"trak" {
            continue;
        }
        let media = find(reader, (data, end), b"mdia")?;
        let handler = find(reader, media, b"hdlr")?;
        if handler.1 - handler.0 < 12 {
            return Err(invalid());
        }
        reader.seek(SeekFrom::Start(handler.0 + 8))?;
        let mut kind = [0; 4];
        reader.read_exact(&mut kind)?;
        if &kind != b"vide" {
            continue;
        }
        let info = find(reader, media, b"minf")?;
        let table = find(reader, info, b"stbl")?;
        let descriptions = find(reader, table, b"stsd")?;
        let mut offset = descriptions.0 + 8;
        for index in 1..=description {
            let (kind, data, end) = header(reader, offset, descriptions.1).ok_or_else(invalid)?;
            if index == description {
                if &kind != b"hev1" {
                    return Err(invalid());
                }
                let configuration = find(reader, (data + 78, end), b"hvcC")?;
                return Ok((offset + 4, configuration.0));
            }
            offset = end;
        }
    }
    Err(invalid())
}

/// The destination must be empty and privately owned until validation succeeds.
pub fn canonicalize_hev1(
    reader: &mut (impl Read + Seek),
    writer: &mut (impl Read + Write + Seek),
    cancelled: &AtomicBool,
) -> io::Result<()> {
    check_cancelled(cancelled)?;
    let length = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;
    writer.seek(SeekFrom::Start(0))?;
    copy(reader, writer, length, cancelled)?;
    let track = VideoTrack::read_hevc(writer)?;
    if &track.sample_entry != b"hev1" {
        return Err(invalid());
    }
    track.visit_samples(|range| {
        check_cancelled(cancelled)?;
        let mut offset = range.start;
        if offset == range.end {
            return Err(invalid());
        }
        while offset < range.end {
            check_cancelled(cancelled)?;
            if range.end - offset < track.nal_length_size as u64 + 2 {
                return Err(invalid());
            }
            writer.seek(SeekFrom::Start(offset))?;
            let mut size = [0; 4];
            writer.read_exact(&mut size[4 - track.nal_length_size..])?;
            let size = u64::from(u32::from_be_bytes(size));
            offset += track.nal_length_size as u64;
            if size < 2 || size > range.end - offset {
                return Err(invalid());
            }
            let mut nal = [0; 2];
            writer.read_exact(&mut nal)?;
            if nal[0] & 0x80 != 0 || nal[1] & 7 == 0 {
                return Err(invalid());
            }
            if (32..=34).contains(&((nal[0] >> 1) & 63)) {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "HEVC in-band parameter sets require bitstream remuxing",
                ));
            }
            offset += size;
        }
        Ok(())
    })?;
    check_cancelled(cancelled)?;
    let (kind, configuration) = entry(writer, length, track.description)?;
    writer.seek(SeekFrom::Start(kind))?;
    writer.write_all(b"hvc1")?;
    for (offset, value) in track.hevc_headers {
        writer.seek(SeekFrom::Start(configuration + offset as u64))?;
        writer.write_all(&[value])?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/isobmff/hevc_normalize.rs"]
mod tests;
