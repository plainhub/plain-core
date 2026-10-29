use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

const MAX_BOX_BYTES: u64 = 64 * 1024 * 1024;

struct Atom<'a> {
    kind: [u8; 4],
    data: &'a [u8],
}

fn u32_at(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        data.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn u64_at(data: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_be_bytes(
        data.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
}
fn atoms(mut data: &[u8]) -> Option<Vec<Atom<'_>>> {
    let mut result = Vec::new();
    while !data.is_empty() {
        let length = u64::from(u32_at(data, 0)?);
        let kind = data.get(4..8)?.try_into().ok()?;
        let (length, header) = match length {
            0 => (data.len() as u64, 8),
            1 => (u64_at(data, 8)?, 16),
            n => (n, 8),
        };
        let length = usize::try_from(length).ok()?;
        if length < header || length > data.len() {
            return None;
        }
        result.push(Atom {
            kind,
            data: &data[header..length],
        });
        data = &data[length..];
    }
    Some(result)
}
fn time_header(data: &[u8]) -> Option<(u32, u64)> {
    let (scale, ticks) = match data.first()? {
        0 => (u32_at(data, 12)?, u64::from(u32_at(data, 16)?)),
        1 => (u32_at(data, 20)?, u64_at(data, 24)?),
        _ => return None,
    };
    if scale == 0 {
        return None;
    }
    let ticks = if ticks == u64::MAX || (data.first() == Some(&0) && ticks == u64::from(u32::MAX)) {
        0
    } else {
        ticks
    };
    Some((scale, ticks))
}
fn milliseconds(scale: u32, ticks: u64) -> Option<i64> {
    if ticks == 0 || ticks == u64::MAX {
        return None;
    }
    let value = u128::from(ticks).checked_mul(1000)? / u128::from(scale);
    i64::try_from(value).ok().filter(|value| *value > 0)
}
struct Track {
    id: u32,
    scale: u32,
    ticks: u64,
    video: bool,
}
fn track(data: &[u8]) -> Option<Track> {
    let mut id = None;
    let mut time = None;
    let mut handler = None;
    for atom in atoms(data)? {
        match &atom.kind {
            b"tkhd" => {
                id = Some(u32_at(
                    atom.data,
                    match atom.data.first()? {
                        0 => 12,
                        1 => 20,
                        _ => return None,
                    },
                )?);
            }
            b"mdia" => {
                for child in atoms(atom.data)? {
                    match &child.kind {
                        b"mdhd" => time = time_header(child.data),
                        b"hdlr" => handler = child.data.get(8..12),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let handler = handler?;
    if handler != b"vide" && handler != b"soun" {
        return None;
    }
    let (scale, ticks) = time?;
    Some(Track {
        id: id?,
        scale,
        ticks,
        video: handler == b"vide",
    })
}
fn run_ticks(data: &[u8], default: u32) -> Option<u64> {
    let flags = u32_at(data, 0)? & 0xffffff;
    let count = u64::from(u32_at(data, 4)?);
    let mut offset = 8 + usize::from(flags & 1 != 0) * 4 + usize::from(flags & 4 != 0) * 4;
    if offset > data.len() {
        return None;
    }
    let stride = (8..=11).filter(|bit| flags & (1 << bit) != 0).count() * 4;
    let bytes = count.checked_mul(stride as u64)?;
    if bytes > (data.len() - offset) as u64 {
        return None;
    }
    if flags & 0x100 == 0 {
        return count.checked_mul(u64::from(default));
    }
    let mut total = 0u64;
    for _ in 0..count {
        total = total.checked_add(u64::from(u32_at(data, offset)?))?;
        offset += stride;
    }
    Some(total)
}
fn fragment_ticks(data: &[u8], id: u32, inherited: u32) -> Option<u64> {
    let mut total = 0u64;
    for atom in atoms(data)? {
        if &atom.kind != b"traf" {
            continue;
        }
        let children = atoms(atom.data)?;
        let header = children.iter().find(|child| &child.kind == b"tfhd")?;
        if u32_at(header.data, 4)? != id {
            continue;
        }
        let flags = u32_at(header.data, 0)? & 0xffffff;
        let offset = 8 + usize::from(flags & 1 != 0) * 8 + usize::from(flags & 2 != 0) * 4;
        let default = if flags & 8 != 0 {
            u32_at(header.data, offset)?
        } else {
            inherited
        };
        for run in children.iter().filter(|child| &child.kind == b"trun") {
            total = total.checked_add(run_ticks(run.data, default)?)?;
        }
    }
    Some(total)
}
fn header(file: &mut (impl Read + Seek), offset: u64, end: u64) -> Option<([u8; 4], u64, u64)> {
    if end.checked_sub(offset)? < 8 {
        return None;
    }
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut bytes = [0; 8];
    file.read_exact(&mut bytes).ok()?;
    let size = u32::from_be_bytes(bytes[..4].try_into().ok()?);
    let (length, header) = match size {
        0 => (end - offset, 8),
        1 => {
            if end.checked_sub(offset)? < 16 {
                return None;
            }
            let mut large = [0; 8];
            file.read_exact(&mut large).ok()?;
            (u64::from_be_bytes(large), 16)
        }
        n => (u64::from(n), 8),
    };
    let next = offset.checked_add(length)?;
    if length < header || next > end {
        return None;
    }
    Some((bytes[4..8].try_into().ok()?, offset + header, next))
}
fn payload(file: &mut (impl Read + Seek), start: u64, end: u64) -> Option<Vec<u8>> {
    let length = end.checked_sub(start)?;
    if length > MAX_BOX_BYTES {
        return None;
    }
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = vec![0; usize::try_from(length).ok()?];
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

pub fn mp4_milliseconds(path: &Path) -> Option<i64> {
    let mut file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    mp4_file_milliseconds(&mut file, 0, length)
}

pub fn mp4_file_milliseconds(
    file: &mut (impl Read + Seek),
    source_offset: u64,
    source_length: u64,
) -> Option<i64> {
    let length = source_offset.checked_add(source_length)?;
    let mut offset = source_offset;
    let mut selected = None;
    let mut movie_zero = false;
    let mut defaults = HashMap::new();
    while offset < length {
        let (kind, start, end) = header(file, offset, length)?;
        if &kind == b"moov" {
            let bytes = payload(file, start, end)?;
            let mut tracks = Vec::new();
            let mut movie = None;
            for atom in atoms(&bytes)? {
                match &atom.kind {
                    b"mvhd" => {
                        if let Some((scale, ticks)) = time_header(atom.data) {
                            movie = milliseconds(scale, ticks);
                            movie_zero = ticks == 0;
                        }
                    }
                    b"trak" => {
                        if let Some(track) = track(atom.data) {
                            tracks.push(track)
                        }
                    }
                    b"mvex" => {
                        for child in atoms(atom.data)? {
                            if &child.kind == b"trex" {
                                defaults.insert(u32_at(child.data, 4)?, u32_at(child.data, 12)?);
                            }
                        }
                    }
                    _ => {}
                }
            }
            if movie.is_some() {
                return movie;
            }
            selected = tracks.into_iter().max_by_key(|track| track.video);
            break;
        }
        offset = end;
    }
    let Some(track) = selected else {
        return movie_zero.then_some(0);
    };
    if let Some(duration) = milliseconds(track.scale, track.ticks) {
        return Some(duration);
    }
    let mut ticks = 0u64;
    offset = source_offset;
    while offset < length {
        let (kind, start, end) = header(file, offset, length)?;
        if &kind == b"moof" {
            let bytes = payload(file, start, end)?;
            ticks = ticks.checked_add(fragment_ticks(
                &bytes,
                track.id,
                defaults.get(&track.id).copied().unwrap_or(0),
            )?)?;
        }
        offset = end;
    }
    milliseconds(track.scale, ticks).or_else(|| movie_zero.then_some(0))
}

#[derive(Debug, Eq, PartialEq)]
pub struct Tracks {
    pub first_video_codec: Option<String>,
    pub needs_remux: bool,
}

fn child<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    atoms(data)?
        .into_iter()
        .find(|atom| &atom.kind == kind)
        .map(|atom| atom.data)
}

pub fn tracks(path: &Path) -> Option<Tracks> {
    let mut file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let mut offset = 0;
    while offset < length {
        let (kind, start, end) = header(&mut file, offset, length)?;
        offset = end;
        if &kind != b"moov" {
            continue;
        }
        let bytes = payload(&mut file, start, end)?;
        let mut result = Tracks {
            first_video_codec: None,
            needs_remux: false,
        };
        let mut saw_video = false;
        for track in atoms(&bytes)?
            .into_iter()
            .filter(|atom| &atom.kind == b"trak")
        {
            let Some(media) = child(track.data, b"mdia") else {
                continue;
            };
            let video = child(media, b"hdlr").and_then(|handler| handler.get(8..12))
                == Some(b"vide".as_slice());
            let description = child(media, b"minf")
                .and_then(|data| child(data, b"stbl"))
                .and_then(|data| child(data, b"stsd"));
            let entries = description.and_then(|data| {
                let count = usize::try_from(u32_at(data, 4)?).ok()?;
                let entries = atoms(data.get(8..)?)?;
                (count == entries.len()).then_some(entries)
            });
            if let Some(entries) = &entries {
                result.needs_remux |= entries.iter().any(|entry| &entry.kind == b"mebx");
            }
            if video && !saw_video {
                saw_video = true;
                result.first_video_codec = entries.and_then(|entries| {
                    entries
                        .first()
                        .map(|entry| String::from_utf8_lossy(&entry.kind).into_owned())
                });
            }
        }
        return Some(result);
    }
    None
}

#[cfg(test)]
#[path = "../../tests/unit/media/isobmff.rs"]
mod tests;

mod location;

pub fn location_reader(reader: &mut (impl Read + Seek)) -> Option<String> {
    location::read(reader)
}

mod metadata;
mod video;
pub use super::video_info::VideoInfo;

pub fn video_reader(reader: &mut (impl Read + Seek)) -> Option<VideoInfo> {
    video::read(reader)
}

mod projection;

mod samples;
pub use samples::AvcTrack;
