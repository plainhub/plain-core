use super::{u16_at, u32_at};
use crate::media::video_info::VideoInfo;
use std::io::{Read, Seek};

pub(super) fn utf16(bytes: &[u8]) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units = bytes
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units)
        .ok()
        .map(|text| text.trim_end_matches('\0').to_owned())
}
pub(super) fn read(
    reader: &mut (impl Read + Seek),
    offset: u64,
    length: u64,
    end: u64,
    budget: &mut u64,
) -> Option<Vec<u8>> {
    if length > 64 * 1024 || offset.checked_add(length)? > end {
        return None;
    }
    *budget = budget.checked_sub(length)?;
    super::objects::position(reader, offset)?;
    let mut bytes = vec![0; length as usize];
    reader.read_exact(&mut bytes).ok()?;
    Some(bytes)
}
pub(super) fn content(
    reader: &mut (impl Read + Seek),
    start: u64,
    end: u64,
    info: &mut VideoInfo,
    budget: &mut u64,
) -> Option<()> {
    let sizes = read(reader, start, 10, end, budget)?;
    let mut offset = start + 10;
    for index in 0..5 {
        let size = u64::from(u16_at(&sizes, index * 2)?);
        if offset.checked_add(size)? > end {
            return None;
        }
        if index < 2
            && let Some(text) = utf16(&read(reader, offset, size, end, budget)?)
        {
            if index == 0 {
                info.title = text;
            } else {
                info.artist = text;
            }
        }
        offset += size;
    }
    Some(())
}
pub(super) struct Descriptor {
    pub name: Option<String>,
    pub stream: u16,
    pub kind: u16,
    pub data: u64,
    pub end: u64,
}
pub(super) fn descriptors<R: Read + Seek>(
    reader: &mut R,
    start: u64,
    end: u64,
    library: bool,
    budget: &mut u64,
    mut visitor: impl FnMut(&mut R, Descriptor, &mut u64) -> Option<bool>,
) -> Option<()> {
    let count = u16_at(&read(reader, start, 2, end, budget)?, 0)?;
    let mut offset = start + 2;
    for _ in 0..count {
        let (name_size, stream, properties) = if library {
            let header = read(reader, offset, 12, end, budget)?;
            offset += 12;
            (
                u64::from(u16_at(&header, 4)?),
                u16_at(&header, 2)?,
                Some((u16_at(&header, 6)?, u64::from(u32_at(&header, 8)?))),
            )
        } else {
            let size = u16_at(&read(reader, offset, 2, end, budget)?, 0)?;
            offset += 2;
            (u64::from(size), 0, None)
        };
        let name = utf16(&read(reader, offset, name_size, end, budget)?);
        offset = offset.checked_add(name_size)?;
        let (kind, size) = match properties {
            Some(properties) => properties,
            None => {
                let header = read(reader, offset, 4, end, budget)?;
                offset += 4;
                (u16_at(&header, 0)?, u64::from(u16_at(&header, 2)?))
            }
        };
        let next = offset.checked_add(size)?;
        if next > end {
            return None;
        }
        if !visitor(
            reader,
            Descriptor {
                name,
                stream,
                kind,
                data: offset,
                end: next,
            },
            budget,
        )? {
            return Some(());
        }
        offset = next;
    }
    Some(())
}
pub(super) fn extended(
    reader: &mut (impl Read + Seek),
    start: u64,
    end: u64,
    info: &mut VideoInfo,
    budget: &mut u64,
) -> Option<()> {
    values(reader, start, end, false, info, budget)
}
pub(super) fn records(
    reader: &mut (impl Read + Seek),
    start: u64,
    end: u64,
    info: &mut VideoInfo,
    budget: &mut u64,
) -> Option<()> {
    values(reader, start, end, true, info, budget)
}
fn values(
    reader: &mut (impl Read + Seek),
    start: u64,
    end: u64,
    library: bool,
    info: &mut VideoInfo,
    budget: &mut u64,
) -> Option<()> {
    descriptors(
        reader,
        start,
        end,
        library,
        budget,
        |reader, record, budget| {
            if record.stream == 0
                && let Some(name) = record.name
                && selected(&name)
            {
                let bytes = read(
                    reader,
                    record.data,
                    record.end - record.data,
                    record.end,
                    budget,
                )?;
                if let Some(value) = if record.kind == 0 {
                    utf16(&bytes)
                } else if record.kind == 3 && name == "WM/VideoFrameRate" {
                    Some(u32_at(&bytes, 0)?.to_string())
                } else {
                    None
                } {
                    apply(info, &name, value);
                }
            }
            Some(true)
        },
    )
}
fn selected(name: &str) -> bool {
    matches!(
        name,
        "Title"
            | "Author"
            | "WM/AlbumArtist"
            | "WM/AlbumTitle"
            | "WM/Genre"
            | "WM/Writer"
            | "WM/Composer"
            | "WM/VideoFrameRate"
            | "WM/MediaOriginalBroadcastDateTime"
    )
}
fn apply(info: &mut VideoInfo, name: &str, value: String) {
    match name {
        "Title" => info.title = value,
        "Author" => info.artist = value,
        "WM/AlbumArtist" => {
            info.album_artist = value.clone();
            if info.artist.is_empty() {
                info.artist = value;
            }
        }
        "WM/AlbumTitle" => info.album = value,
        "WM/Genre" => info.genre = value,
        "WM/Writer" => info.writer = value,
        "WM/Composer" => info.composer = value,
        "WM/MediaOriginalBroadcastDateTime" => info.raw_taken_at = Some(value),
        "WM/VideoFrameRate" => {
            if let Ok(rate) = value.parse::<f64>()
                && rate.is_finite()
                && rate > 0.0
            {
                info.frame_rate = rate;
            }
        }
        _ => {}
    }
}
