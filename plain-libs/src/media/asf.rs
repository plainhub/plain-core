mod cover;
mod objects;
mod tags;
use objects::{METADATA, METADATA_LIBRARY};

use super::video_info::VideoInfo;
use std::io::{Read, Seek, SeekFrom};

pub(super) const HEADER: [u8; 16] = [
    0x30, 0x26, 0xb2, 0x75, 0x8e, 0x66, 0xcf, 0x11, 0xa6, 0xd9, 0, 0xaa, 0, 0x62, 0xce, 0x6c,
];
const FILE_PROPERTIES: [u8; 16] = [
    0xa1, 0xdc, 0xab, 0x8c, 0x47, 0xa9, 0xcf, 0x11, 0x8e, 0xe4, 0, 0xc0, 0x0c, 0x20, 0x53, 0x65,
];

const STREAM_PROPERTIES: [u8; 16] = [
    0x91, 7, 0xdc, 0xb7, 0xb7, 0xa9, 0xcf, 0x11, 0x8e, 0xe6, 0, 0xc0, 0x0c, 0x20, 0x53, 0x65,
];
const VIDEO_MEDIA: [u8; 16] = [
    0xc0, 0xef, 0x19, 0xbc, 0x4d, 0x5b, 0xcf, 0x11, 0xa8, 0xfd, 0, 0x80, 0x5f, 0x5c, 0x44, 0x2b,
];
const CONTENT: [u8; 16] = [
    0x33, 0x26, 0xb2, 0x75, 0x8e, 0x66, 0xcf, 0x11, 0xa6, 0xd9, 0, 0xaa, 0, 0x62, 0xce, 0x6c,
];
const EXTENDED_CONTENT: [u8; 16] = [
    0x40, 0xa4, 0xd0, 0xd2, 7, 0xe3, 0xd2, 0x11, 0x97, 0xf0, 0, 0xa0, 0xc9, 0x5e, 0xa8, 0x50,
];

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u64_at(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
}
fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn parse(reader: &mut (impl Read + Seek), details: bool) -> Option<(VideoInfo, Option<i64>)> {
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    let mut result = VideoInfo::default();
    let mut duration = None;
    let mut file_properties = false;
    let mut metadata_bytes = 4 * 1024 * 1024u64;
    objects::visit(reader, details, |reader, object| {
        let size = object.size;
        let offset = object.data - 24;
        let next = object.end;
        if object.guid == FILE_PROPERTIES {
            file_properties = true;
            if size < 104 {
                return None;
            }
            let mut data = [0; 80];
            reader.read_exact(&mut data).ok()?;
            if u32_at(&data, 64)? & 1 == 0 {
                duration = (u64_at(&data, 40)? / 10000)
                    .checked_sub(u64_at(&data, 56)?)
                    .and_then(|value| i64::try_from(value).ok());
            }
            if !details {
                return Some(false);
            }
        } else if details && object.guid == STREAM_PROPERTIES && result.width == 0 {
            if size < 78 {
                return None;
            }
            let mut data = [0; 54];
            reader.read_exact(&mut data).ok()?;
            let specific = u64::from(u32_at(&data, 40)?);
            let correction = u64::from(u32_at(&data, 44)?);
            if 78u64.checked_add(specific)?.checked_add(correction)? > size {
                return None;
            }
            if data[..16] == VIDEO_MEDIA && u16_at(&data, 48)? & 0x7f != 0 {
                if specific < 51 {
                    return None;
                }
                let mut format = [0; 51];
                reader.read_exact(&mut format).ok()?;
                let format_size = u64::from(u16_at(&format, 9)?);
                let bitmap_size = u64::from(u32_at(&format, 11)?);
                if format_size < 40
                    || 11u64.checked_add(format_size)? > specific
                    || bitmap_size < 40
                    || bitmap_size > format_size
                {
                    return None;
                }
                let width = u32_at(&format, 15)? as i32;
                let height = (u32_at(&format, 19)? as i32).checked_abs()?;
                if width > 0 && height > 0 {
                    result.width = width as u32;
                    result.height = height as u32;
                }
            }
        } else if details && (object.guid == CONTENT || object.guid == EXTENDED_CONTENT) {
            if object.guid == CONTENT {
                tags::content(reader, offset + 24, next, &mut result, &mut metadata_bytes)?;
            } else {
                tags::extended(reader, offset + 24, next, &mut result, &mut metadata_bytes)?;
            }
        } else if details && matches!(object.guid, METADATA | METADATA_LIBRARY) {
            tags::records(
                reader,
                object.data,
                object.end,
                &mut result,
                &mut metadata_bytes,
            )?;
        }
        Some(true)
    })?;
    if !file_properties {
        return None;
    }
    result.duration_ms = duration.unwrap_or_default();
    result.bitrate = duration
        .filter(|value| *value > 0)
        .and_then(|value| i64::try_from(u128::from(length) * 8000 / value as u128).ok())
        .unwrap_or_default();
    Some((result, duration))
}
pub(super) fn metadata(reader: &mut (impl Read + Seek)) -> Option<(VideoInfo, Option<i64>)> {
    parse(reader, true)
}
pub(super) fn milliseconds(reader: &mut (impl Read + Seek)) -> Option<i64> {
    parse(reader, false)?.1
}
pub(super) fn video(reader: &mut (impl Read + Seek)) -> Option<VideoInfo> {
    let (video, _) = parse(reader, true)?;
    (video.width > 0 && video.height > 0).then_some(video)
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/asf.rs"]
mod tests;

pub(super) fn cover(reader: &mut (impl Read + Seek)) -> Option<super::cover::CoverData> {
    cover::read(reader)
}
