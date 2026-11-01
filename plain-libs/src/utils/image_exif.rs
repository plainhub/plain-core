use super::image_dimensions::{parse_tiff_header, tiff_read_u16, tiff_read_u32};
use std::{
    collections::HashSet,
    io::{self, BufReader, Read, Seek, SeekFrom},
};

const MAX_METADATA_BYTES: usize = 4 * 1024 * 1024;

pub fn read(reader: &mut (impl Read + Seek)) -> io::Result<Option<Vec<u8>>> {
    reader.seek(SeekFrom::Start(0))?;
    let mut header = [0; 8];
    if let Err(error) = reader.read_exact(&mut header) {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            return Ok(None);
        }
        return Err(error);
    }
    if let Some((little, first)) = parse_tiff_header(&header) {
        let mut output = header.to_vec();
        let first = project(
            reader,
            &mut output,
            first,
            little,
            Directory::Primary,
            &mut HashSet::new(),
        )?;
        output[4..8].copy_from_slice(&encode_u32(first, little));
        return Ok(Some(output));
    }
    reader.seek(SeekFrom::Start(0))?;
    match exif::Reader::new().read_from_container(&mut BufReader::new(reader)) {
        Ok(exif) => Ok(Some(exif.buf().to_vec())),
        Err(exif::Error::NotFound(_)) => Ok(None),
        Err(error) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    }
}
#[derive(Clone, Copy)]
enum Directory {
    Primary,
    Exif,
    Gps,
}
fn selected(directory: Directory, tag: u16) -> bool {
    match directory {
        Directory::Primary => matches!(
            tag,
            0x0100
                | 0x0101
                | 0x010e
                | 0x010f
                | 0x0110
                | 0x0112
                | 0x011a
                | 0x011b
                | 0x013b
                | 0x8769
                | 0x8825
        ),
        Directory::Exif => matches!(
            tag,
            0x9003
                | 0x9011
                | 0xa001
                | 0x9202
                | 0x829a
                | 0x920a
                | 0x8827
                | 0x9209
                | 0x829d
                | 0x8822
                | 0x9207
                | 0xa403
        ),
        Directory::Gps => matches!(tag, 1..=4),
    }
}
fn encode_u32(value: u32, little: bool) -> [u8; 4] {
    if little {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    }
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid or oversized TIFF metadata",
    )
}
fn reserve(output: &mut Vec<u8>, bytes: usize) -> io::Result<usize> {
    let end = output
        .len()
        .checked_add(bytes)
        .filter(|end| *end <= MAX_METADATA_BYTES)
        .ok_or_else(invalid)?;
    let start = output.len();
    output.resize(end, 0);
    Ok(start)
}
fn project(
    reader: &mut (impl Read + Seek),
    output: &mut Vec<u8>,
    offset: u32,
    little: bool,
    directory: Directory,
    visited: &mut HashSet<u32>,
) -> io::Result<u32> {
    if !visited.insert(offset) {
        return Err(invalid());
    }
    reader.seek(SeekFrom::Start(u64::from(offset)))?;
    let mut count = [0; 2];
    reader.read_exact(&mut count)?;
    let count = usize::from(tiff_read_u16(&count, 0, little).ok_or_else(invalid)?);
    let mut table = vec![0; count * 12];
    reader.read_exact(&mut table)?;
    let entries = table
        .chunks_exact(12)
        .filter(|entry| selected(directory, tiff_read_u16(entry, 0, little).unwrap()))
        .collect::<Vec<_>>();
    let start = reserve(output, 2 + entries.len() * 12 + 4)?;
    let count = u16::try_from(entries.len()).map_err(|_| invalid())?;
    output[start..start + 2].copy_from_slice(&if little {
        count.to_le_bytes()
    } else {
        count.to_be_bytes()
    });
    for (index, entry) in entries.into_iter().enumerate() {
        let tag = tiff_read_u16(entry, 0, little).unwrap();
        let kind = tiff_read_u16(entry, 2, little).unwrap();
        let count = tiff_read_u32(entry, 4, little).unwrap();
        let target = start + 2 + index * 12;
        output[target..target + 12].copy_from_slice(entry);
        let pointer = match (directory, tag) {
            (Directory::Primary, 0x8769) => Some(Directory::Exif),
            (Directory::Primary, 0x8825) => Some(Directory::Gps),
            _ => None,
        };
        if let Some(directory) = pointer {
            if kind != 4 || count != 1 {
                return Err(invalid());
            }
            let offset = tiff_read_u32(entry, 8, little).unwrap();
            let projected = project(reader, output, offset, little, directory, visited)?;
            output[target + 8..target + 12].copy_from_slice(&encode_u32(projected, little));
            continue;
        }
        let unit = match kind {
            1 | 2 | 7 => 1,
            3 => 2,
            4 | 9 => 4,
            5 | 10 => 8,
            _ => return Err(invalid()),
        };
        let length = usize::try_from(count)
            .ok()
            .and_then(|count| count.checked_mul(unit))
            .ok_or_else(invalid)?;
        if length > 4 {
            let offset = tiff_read_u32(entry, 8, little).unwrap();
            let projected = reserve(output, length)?;
            reader.seek(SeekFrom::Start(u64::from(offset)))?;
            reader.read_exact(&mut output[projected..projected + length])?;
            output[target + 8..target + 12].copy_from_slice(&encode_u32(
                u32::try_from(projected).map_err(|_| invalid())?,
                little,
            ));
        }
    }
    u32::try_from(start).map_err(|_| invalid())
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/image_exif.rs"]
mod tests;
