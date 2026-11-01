mod riff;

use super::video_info::VideoInfo;
use riff::{Chunk, Parser};
use std::io::{Read, Seek, SeekFrom};

fn number(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}
#[derive(Default)]
struct Main {
    microseconds: u32,
    frames: u32,
    width: u32,
    height: u32,
}
#[derive(Default)]
struct Track {
    kind: [u8; 4],
    flags: u32,
    scale: u32,
    rate: u32,
    frames: u32,
    width: u32,
    height: u32,
}
impl Track {
    fn duration(&self, frames: u32) -> Option<i64> {
        if self.scale == 0 || self.rate == 0 {
            return None;
        }
        i64::try_from(u128::from(frames) * u128::from(self.scale) * 1000 / u128::from(self.rate))
            .ok()
    }
}
struct Parsed {
    video: VideoInfo,
    duration: Option<i64>,
}
fn track<R: Read + Seek>(
    parser: &mut Parser<'_, R>,
    parent: Chunk,
    details: bool,
) -> Option<Track> {
    let mut header = None;
    let mut format = None;
    parser.children(parent.data + 4, parent.end, |parser, chunk| {
        match &chunk.id {
            b"strh" => header = Some(parser.prefix(chunk, 48)?),
            b"strf" if details => format = Some(chunk),
            _ => {}
        }
        Some(())
    })?;
    let header = header?;
    let mut result = Track {
        kind: header[..4].try_into().ok()?,
        flags: number(&header, 8)?,
        scale: number(&header, 20)?,
        rate: number(&header, 24)?,
        frames: number(&header, 32)?,
        ..Default::default()
    };
    if &result.kind == b"vids"
        && result.flags & 1 == 0
        && let Some(format) = format
    {
        let format = parser.prefix(format, 40)?;
        if number(&format, 0)? < 40 {
            return None;
        }
        let width = number(&format, 4)? as i32;
        let height = number(&format, 8)? as i32;
        if width > 0 {
            result.width = width as u32;
            result.height = u32::try_from(height.checked_abs()?).ok()?;
        }
    }
    Some(result)
}
fn info<R: Read + Seek>(
    parser: &mut Parser<'_, R>,
    parent: Chunk,
    result: &mut VideoInfo,
) -> Option<()> {
    parser.children(parent.data + 4, parent.end, |parser, chunk| {
        let field = match &chunk.id {
            b"INAM" => &mut result.title,
            b"IART" => &mut result.artist,
            b"IPRD" => &mut result.album,
            b"IGNR" => &mut result.genre,
            b"IWRI" => &mut result.writer,
            b"IMUS" => &mut result.composer,
            b"ICRD" => {
                if let Some(value) = parser.text(chunk) {
                    result.raw_taken_at = Some(value);
                }
                return Some(());
            }
            _ => return Some(()),
        };
        if let Some(value) = parser.text(chunk) {
            *field = value;
        }
        Some(())
    })
}
fn parse(reader: &mut (impl Read + Seek), details: bool) -> Option<Parsed> {
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    let mut parser = Parser::new(reader);
    let root = parser.chunk(0, length)?;
    if &root.id != b"RIFF" || &parser.kind(root)? != b"AVI " {
        return None;
    }
    let mut main = None;
    let mut video_track = None;
    let mut total_frames = None;
    let mut result = VideoInfo::default();
    parser.children(root.data + 4, root.end, |parser, chunk| {
        if &chunk.id != b"LIST" {
            return Some(());
        }
        match &parser.kind(chunk)? {
            b"hdrl" => parser.children(chunk.data + 4, chunk.end, |parser, chunk| {
                match &chunk.id {
                    b"avih" => {
                        let header = parser.prefix(chunk, 56)?;
                        main = Some(Main {
                            microseconds: number(&header, 0)?,
                            frames: number(&header, 16)?,
                            width: number(&header, 32)?,
                            height: number(&header, 36)?,
                        });
                    }
                    b"LIST" => match &parser.kind(chunk)? {
                        b"strl" if video_track.is_none() => {
                            let candidate = track(parser, chunk, details)?;
                            if &candidate.kind == b"vids" && candidate.flags & 1 == 0 {
                                video_track = Some(candidate);
                            }
                        }
                        b"odml" => {
                            parser.children(chunk.data + 4, chunk.end, |parser, chunk| {
                                if &chunk.id == b"dmlh" {
                                    total_frames = Some(number(&parser.prefix(chunk, 4)?, 0)?);
                                }
                                Some(())
                            })?
                        }
                        _ => {}
                    },
                    _ => {}
                }
                Some(())
            })?,
            b"INFO" if details => info(parser, chunk, &mut result)?,
            _ => {}
        }
        Some(())
    })?;
    let main = main?;
    let duration = video_track
        .as_ref()
        .and_then(|track| {
            track.duration(total_frames.unwrap_or(if track.frames > 0 {
                track.frames
            } else {
                main.frames
            }))
        })
        .or_else(|| {
            (main.microseconds > 0).then(|| {
                (u128::from(total_frames.unwrap_or(main.frames)) * u128::from(main.microseconds)
                    / 1000) as i64
            })
        });
    if let Some(track) = video_track {
        result.width = if track.width > 0 {
            track.width
        } else {
            main.width
        };
        result.height = if track.height > 0 {
            track.height
        } else {
            main.height
        };
        if track.scale > 0 && track.rate > 0 {
            result.frame_rate = f64::from(track.rate) / f64::from(track.scale);
        }
    }
    if result.frame_rate == 0.0 && main.microseconds > 0 {
        result.frame_rate = 1_000_000.0 / f64::from(main.microseconds);
    }
    result.duration_ms = duration.unwrap_or_default();
    result.bitrate = duration
        .filter(|value| *value > 0)
        .and_then(|value| i64::try_from(u128::from(length) * 8000 / value as u128).ok())
        .unwrap_or_default();
    Some(Parsed {
        video: result,
        duration,
    })
}
pub(super) fn video(reader: &mut (impl Read + Seek)) -> Option<VideoInfo> {
    let result = parse(reader, true)?.video;
    (result.width > 0 && result.height > 0).then_some(result)
}
pub(super) fn milliseconds(reader: &mut (impl Read + Seek)) -> Option<i64> {
    parse(reader, false)?.duration
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/avi.rs"]
mod tests;
