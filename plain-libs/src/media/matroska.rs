mod ebml;
mod tags;

use super::video_info::VideoInfo;
use ebml::{Element, Parser};
use std::io::{Read, Seek, SeekFrom};

#[derive(Default)]
struct Info {
    scale: Option<u64>,
    ticks: Option<f64>,
    title: String,
    date: Option<String>,
}
impl Info {
    fn duration(&self) -> Option<i64> {
        let ms = self.ticks? * self.scale.unwrap_or(1_000_000) as f64 / 1_000_000.0;
        (ms.is_finite() && ms >= 0.0 && ms < i64::MAX as f64).then_some(ms as i64)
    }
}
#[derive(Default)]
struct Track {
    uid: u64,
    kind: u64,
    enabled: Option<bool>,
    width: u32,
    height: u32,
    frame_duration: Option<u64>,
    frame_rate: Option<f64>,
}
struct Parsed {
    duration: Option<i64>,
    video: Option<VideoInfo>,
}

fn segment<R: Read + Seek>(parser: &mut Parser<'_, R>, length: u64) -> Option<Element> {
    let header = parser.element(0, length)?;
    if header.id != 0x1a45dfa3 {
        return None;
    }
    let mut valid = false;
    parser.children(header, |parser, element| {
        match element.id {
            0x4282 => valid = matches!(parser.text(element)?.as_str(), "matroska" | "webm"),
            0x42f7 if parser.unsigned(element)? != 1 => return None,
            0x4285 if parser.unsigned(element)? > 4 => return None,
            0x42f2 if !(1..=4).contains(&parser.unsigned(element)?) => return None,
            0x42f3 if !(1..=8).contains(&parser.unsigned(element)?) => return None,
            _ => {}
        }
        Some(())
    })?;
    if !valid {
        return None;
    }
    let mut offset = header.end;
    while offset < length {
        let element = parser.element(offset, length)?;
        if element.id == 0x18538067 {
            return Some(element);
        }
        if !matches!(element.id, 0xec | 0xbf) {
            return None;
        }
        offset = element.end;
    }
    None
}
fn info<R: Read + Seek>(
    parser: &mut Parser<'_, R>,
    parent: Element,
    details: bool,
) -> Option<Info> {
    let mut result = Info::default();
    parser.children(parent, |parser, element| {
        match element.id {
            0x2ad7b1 => {
                let scale = parser.unsigned(element)?;
                if scale == 0 {
                    return None;
                }
                result.scale = Some(scale);
            }
            0x4489 => {
                let ticks = parser.float(element)?;
                if ticks < 0.0 {
                    return None;
                }
                result.ticks = Some(ticks);
            }
            0x7ba9 if details => result.title = parser.text(element)?,
            0x4461 if details => {
                let bytes = parser.bytes(element, 8)?;
                let nanos = if bytes.is_empty() {
                    0
                } else {
                    i64::from_be_bytes(bytes.try_into().ok()?)
                };
                let seconds = 978_307_200i64.checked_add(nanos.div_euclid(1_000_000_000))?;
                result.date = chrono::DateTime::from_timestamp(
                    seconds,
                    nanos.rem_euclid(1_000_000_000) as u32,
                )
                .map(|date| date.to_rfc3339());
            }
            _ => {}
        }
        Some(())
    })?;
    Some(result)
}
fn track<R: Read + Seek>(parser: &mut Parser<'_, R>, parent: Element) -> Option<Track> {
    let mut result = Track::default();
    parser.children(parent, |parser, element| {
        match element.id {
            0x73c5 => result.uid = parser.unsigned(element)?,
            0x83 => result.kind = parser.unsigned(element)?,
            0xb9 => result.enabled = Some(parser.unsigned(element)? != 0),
            0x23e383 => result.frame_duration = Some(parser.unsigned(element)?),
            0xe0 => parser.children(element, |parser, field| {
                match field.id {
                    0xb0 => result.width = u32::try_from(parser.unsigned(field)?).ok()?,
                    0xba => result.height = u32::try_from(parser.unsigned(field)?).ok()?,
                    0x2383e3 => result.frame_rate = Some(parser.float(field)?),
                    _ => {}
                }
                Some(())
            })?,
            _ => {}
        }
        Some(())
    })?;
    Some(result)
}
fn parse(reader: &mut (impl Read + Seek), details: bool) -> Option<Parsed> {
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    let mut parser = Parser::new(reader);
    let segment = segment(&mut parser, length)?;
    let mut offset = segment.data;
    let mut metadata = None;
    let mut video = None;
    let mut tag_elements = Vec::new();
    while offset < segment.end {
        let element = parser.element(offset, segment.end)?;
        offset = if element.id == 0x1f43b675 {
            parser.next_segment_element(element)?
        } else {
            element.end
        };
        match element.id {
            0x1549a966 => {
                metadata = Some(info(&mut parser, element, details)?);
                if !details {
                    return Some(Parsed {
                        duration: metadata?.duration(),
                        video: None,
                    });
                }
            }
            0x1654ae6b if details => parser.children(element, |parser, element| {
                if element.id == 0xae && video.is_none() {
                    let track = track(parser, element)?;
                    if track.kind == 1
                        && track.enabled != Some(false)
                        && track.width > 0
                        && track.height > 0
                    {
                        video = Some(track);
                    }
                }
                Some(())
            })?,
            0x1254c367 if details => {
                if tag_elements.len() >= 128 {
                    return None;
                }
                tag_elements.push(element);
            }
            _ => {}
        }
    }
    let metadata = metadata?;
    let duration = metadata.duration();
    let Some(track) = video else {
        return Some(Parsed {
            duration,
            video: None,
        });
    };
    let mut result = VideoInfo {
        width: track.width,
        height: track.height,
        duration_ms: duration.unwrap_or_default(),
        frame_rate: track
            .frame_duration
            .filter(|value| *value > 0)
            .map(|value| 1_000_000_000.0 / value as f64)
            .or(track.frame_rate.filter(|value| *value > 0.0))
            .unwrap_or_default(),
        bitrate: duration
            .filter(|value| *value > 0)
            .and_then(|value| i64::try_from(u128::from(length) * 8000 / value as u128).ok())
            .unwrap_or_default(),
        title: metadata.title,
        raw_taken_at: metadata.date,
        ..Default::default()
    };
    for element in tag_elements {
        tags::apply(&mut parser, element, track.uid, &mut result)?;
    }
    Some(Parsed {
        duration,
        video: Some(result),
    })
}
pub(super) fn video(reader: &mut (impl Read + Seek)) -> Option<VideoInfo> {
    parse(reader, true)?.video
}
pub(super) fn milliseconds(reader: &mut (impl Read + Seek)) -> Option<i64> {
    parse(reader, false)?.duration
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/matroska.rs"]
mod tests;
