mod amf;
mod video;

use super::video_info::VideoInfo;
use amf::Scalar;
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
};

fn unsigned24(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes([
        0,
        *bytes.first()?,
        *bytes.get(1)?,
        *bytes.get(2)?,
    ]))
}
struct Tag {
    kind: u8,
    data: u64,
    size: u32,
    end: u64,
}
fn tag(reader: &mut (impl Read + Seek), offset: u64, length: u64) -> Option<Tag> {
    if length.checked_sub(offset)? < 15 {
        return None;
    }
    reader.seek(SeekFrom::Start(offset)).ok()?;
    let mut header = [0; 11];
    reader.read_exact(&mut header).ok()?;
    if header[0] & 0xe0 != 0 || header[8..] != [0; 3] {
        return None;
    }
    let size = unsigned24(&header[1..4])?;
    let data = offset.checked_add(11)?;
    let end = data.checked_add(u64::from(size))?.checked_add(4)?;
    if end > length {
        return None;
    }
    reader.seek(SeekFrom::Start(end - 4)).ok()?;
    let mut trailer = [0; 4];
    reader.read_exact(&mut trailer).ok()?;
    if u32::from_be_bytes(trailer) != size.checked_add(11)? {
        return None;
    }
    Some(Tag {
        kind: header[0],
        data,
        size,
        end,
    })
}
fn numeric(fields: &HashMap<String, Owned>, key: &str) -> Option<f64> {
    match fields.get(key)? {
        Owned::Number(value) if value.is_finite() && *value >= 0.0 => Some(*value),
        _ => None,
    }
}
fn dimension(fields: &HashMap<String, Owned>, key: &str) -> u32 {
    numeric(fields, key)
        .filter(|value| *value > 0.0 && *value <= u32::MAX as f64 && value.fract() == 0.0)
        .map(|value| value as u32)
        .unwrap_or_default()
}
fn text(fields: &HashMap<String, Owned>, key: &str) -> String {
    match fields.get(key) {
        Some(Owned::Text(value)) => value.clone(),
        _ => String::new(),
    }
}
fn parse(reader: &mut (impl Read + Seek)) -> Option<(VideoInfo, Option<i64>)> {
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    reader.seek(SeekFrom::Start(0)).ok()?;
    let mut header = [0; 9];
    reader.read_exact(&mut header).ok()?;
    if &header[..3] != b"FLV" || header[3] != 1 || header[4] & !5 != 0 {
        return None;
    }
    let offset = u64::from(u32::from_be_bytes(header[5..].try_into().ok()?));
    if offset < 9 || offset.checked_add(4)? > length {
        return None;
    }
    reader.seek(SeekFrom::Start(offset)).ok()?;
    let mut previous = [0; 4];
    reader.read_exact(&mut previous).ok()?;
    if previous != [0; 4] {
        return None;
    }
    let first = offset + 4;
    let mut offset = first;
    let mut fields = HashMap::new();
    let mut stream = None;
    let mut remaining = 4 * 1024 * 1024u32;
    for _ in 0..128 {
        if offset >= length {
            break;
        }
        let tag = tag(reader, offset, length)?;
        offset = tag.end;
        if tag.kind == 18 && tag.size <= 1024 * 1024 && tag.size <= remaining {
            remaining -= tag.size;
            let mut bytes = vec![0; tag.size as usize];
            reader.seek(SeekFrom::Start(tag.data)).ok()?;
            reader.read_exact(&mut bytes).ok()?;
            if let Some(metadata) = amf::metadata(&bytes) {
                let owned = metadata
                    .into_iter()
                    .map(|(key, value)| {
                        (
                            key.to_owned(),
                            match value {
                                Scalar::Number(value) => Owned::Number(value),
                                Scalar::Text(value) => Owned::Text(value.to_owned()),
                                Scalar::Other => Owned::Other,
                            },
                        )
                    })
                    .collect::<HashMap<_, _>>();
                fields = owned;
                if dimension(&fields, "width") > 0
                    && dimension(&fields, "height") > 0
                    && numeric(&fields, "framerate").is_some_and(|value| value > 0.0)
                {
                    break;
                }
            }
        }
        if tag.kind == 9 && stream.is_none() && tag.size > 1 {
            reader.seek(SeekFrom::Start(tag.data)).ok()?;
            let mut prefix = [0; 1];
            reader.read_exact(&mut prefix).ok()?;
            let codec = prefix[0] & 15;
            if prefix[0] & 0x80 != 0 && prefix[0] & 15 == 0 && tag.size >= 28 {
                let size = (tag.size - 1).min(64 * 1024);
                if size > remaining {
                    break;
                }
                remaining -= size;
                let mut bytes = vec![0; size as usize];
                reader.read_exact(&mut bytes).ok()?;
                if &bytes[..4] == b"hvc1" {
                    stream = super::hevc::hvcc(&bytes[4..]);
                }
            } else if matches!(codec, 7 | 12) && prefix[0] & 0x80 == 0 && tag.size >= 11 {
                let mut packet = [0; 1];
                reader.read_exact(&mut packet).ok()?;
                if packet[0] == 0 {
                    let size = tag.size.min(64 * 1024) - 2;
                    if size > remaining {
                        break;
                    }
                    remaining -= size;
                    let mut bytes = vec![0; size as usize];
                    reader.read_exact(&mut bytes).ok()?;
                    stream = if codec == 7 {
                        super::h264::avcc(&bytes[3..])
                    } else {
                        super::hevc::hvcc(&bytes[3..])
                    };
                }
            } else if prefix[0] >> 4 == 1 && matches!(codec, 1 | 2 | 3 | 6) {
                let maximum = match codec {
                    1 => 64 * 1024,
                    2 => 9,
                    _ => 4,
                };
                let size = (tag.size - 1).min(maximum);
                if size > remaining {
                    break;
                }
                remaining -= size;
                let mut bytes = vec![0; size as usize];
                reader.read_exact(&mut bytes).ok()?;
                stream = video::dimensions(codec, &bytes).map(|(width, height)| {
                    super::video_info::StreamInfo {
                        width,
                        height,
                        frame_rate: None,
                    }
                });
            }
        }
    }
    let duration = numeric(&fields, "duration").and_then(|value| {
        let ms = value * 1000.0;
        (ms.is_finite() && ms < i64::MAX as f64).then_some(ms as i64)
    });
    let date = text(&fields, "creationdate");
    let video = VideoInfo {
        width: match dimension(&fields, "width") {
            0 => stream.as_ref().map(|value| value.width).unwrap_or_default(),
            value => value,
        },
        height: match dimension(&fields, "height") {
            0 => stream
                .as_ref()
                .map(|value| value.height)
                .unwrap_or_default(),
            value => value,
        },
        duration_ms: duration.unwrap_or_default(),
        frame_rate: numeric(&fields, "framerate")
            .filter(|value| *value > 0.0)
            .or_else(|| stream.as_ref().and_then(|value| value.frame_rate))
            .unwrap_or_default(),
        bitrate: duration
            .filter(|value| *value > 0)
            .and_then(|value| i64::try_from(u128::from(length) * 8000 / value as u128).ok())
            .unwrap_or_default(),
        title: text(&fields, "title"),
        artist: text(&fields, "artist"),
        album: text(&fields, "album"),
        genre: text(&fields, "genre"),
        writer: text(&fields, "author"),
        composer: text(&fields, "composer"),
        raw_taken_at: (!date.is_empty()).then_some(date),
        ..Default::default()
    };
    Some((video, duration))
}
enum Owned {
    Number(f64),
    Text(String),
    Other,
}
pub(super) fn video(reader: &mut (impl Read + Seek)) -> Option<VideoInfo> {
    let (video, _) = parse(reader)?;
    (video.width > 0 && video.height > 0).then_some(video)
}
pub(super) fn milliseconds(reader: &mut (impl Read + Seek)) -> Option<i64> {
    parse(reader)?.1
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/flv.rs"]
mod tests;
