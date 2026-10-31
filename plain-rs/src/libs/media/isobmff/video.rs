use super::{atoms, child, header, milliseconds, time_header, u32_at, u64_at};
use std::io::{Read, Seek, SeekFrom};

use crate::libs::media::video_info::VideoInfo;

pub(super) fn read(reader: &mut (impl Read + Seek)) -> Option<VideoInfo> {
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    let mut offset = 0;
    while offset < length {
        let (kind, start, end) = header(reader, offset, length)?;
        offset = end;
        if &kind != b"moov" {
            continue;
        }
        let movie = super::projection::movie(reader, start, end)?;
        let mut fields = super::metadata::read(&movie);
        let mut result = None;
        for atom in atoms(&movie)? {
            if &atom.kind != b"trak" {
                continue;
            }
            let Some(track) = super::track(atom.data).filter(|track| track.video) else {
                continue;
            };
            let track_header = child(atom.data, b"tkhd")?;
            let matrix = match track_header.first()? {
                0 => 40,
                1 => 52,
                _ => return None,
            };
            let (mut width, mut height) = (
                u32_at(track_header, matrix + 36)? >> 16,
                u32_at(track_header, matrix + 40)? >> 16,
            );
            let sample_table = child(atom.data, b"mdia")
                .and_then(|data| child(data, b"minf"))
                .and_then(|data| child(data, b"stbl"));
            if let Some(description) = sample_table.and_then(|data| child(data, b"stsd")) {
                let entries = atoms(description.get(8..)?)?;
                if usize::try_from(u32_at(description, 4)?).ok()? != entries.len() {
                    return None;
                }
                if let Some(entry) = entries.first() {
                    if let (Some(w), Some(h)) = (entry.data.get(24..26), entry.data.get(26..28)) {
                        let w = u16::from_be_bytes(w.try_into().ok()?);
                        let h = u16::from_be_bytes(h.try_into().ok()?);
                        if w > 0 && h > 0 {
                            (width, height) = (u32::from(w), u32::from(h));
                        }
                    }
                }
            }
            if width == 0 || height == 0 {
                continue;
            }
            let a = u32_at(track_header, matrix)? as i32;
            let b = u32_at(track_header, matrix + 4)? as i32;
            let rotation = if a == 0 && b == 0 {
                0
            } else {
                (f64::from(b).atan2(f64::from(a)).to_degrees().round() as i32).rem_euclid(360)
            };
            let duration = child(&movie, b"mvhd")
                .and_then(time_header)
                .and_then(|(scale, ticks)| milliseconds(scale, ticks))
                .or_else(|| milliseconds(track.scale, track.ticks))
                .or_else(|| super::mp4_file_milliseconds(reader, 0, length))?;
            let frame_rate = fields
                .remove("frameRate")
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite() && *value > 0.0)
                .or_else(|| {
                    sample_table
                        .and_then(|data| child(data, b"stts"))
                        .and_then(|data| rate(data, track.scale))
                })
                .unwrap_or(0.0);
            let bitrate = if duration > 0 {
                i64::try_from(u128::from(length).checked_mul(8000)? / duration as u128).ok()?
            } else {
                0
            };
            let creation = child(&movie, b"mvhd").and_then(creation_time);
            result = Some(VideoInfo {
                album_artist: String::new(),

                width,
                height,
                rotation,
                duration_ms: duration,
                bitrate,
                frame_rate,
                title: fields.remove("title").unwrap_or_default(),
                artist: fields.remove("artist").unwrap_or_default(),
                album: fields.remove("album").unwrap_or_default(),
                genre: fields.remove("genre").unwrap_or_default(),
                writer: fields.remove("writer").unwrap_or_default(),
                composer: fields.remove("composer").unwrap_or_default(),
                raw_taken_at: fields.remove("date").or(creation),
                raw_location: super::location_reader(reader),
            });
            break;
        }
        return result;
    }
    None
}

fn rate(data: &[u8], scale: u32) -> Option<f64> {
    let count = usize::try_from(u32_at(data, 4)?).ok()?;
    if count > data.len().checked_sub(8)? / 8 {
        return None;
    }
    let mut samples = 0u128;
    let mut ticks = 0u128;
    for index in 0..count {
        let n = u128::from(u32_at(data, 8 + index * 8)?);
        let delta = u128::from(u32_at(data, 12 + index * 8)?);
        samples = samples.checked_add(n)?;
        ticks = ticks.checked_add(n.checked_mul(delta)?)?;
    }
    if ticks == 0 {
        return None;
    }
    let rate = samples as f64 * f64::from(scale) / ticks as f64;
    rate.is_finite().then_some(rate)
}
fn creation_time(data: &[u8]) -> Option<String> {
    let time = match data.first()? {
        0 => u64::from(u32_at(data, 4)?),
        1 => u64_at(data, 4)?,
        _ => return None,
    };
    if time == 0 {
        return None;
    }
    let seconds = i64::try_from(time).ok()?.checked_sub(2_082_844_800)?;
    Some(
        chrono::DateTime::from_timestamp(seconds, 0)?
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    )
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/libs/media/isobmff/video.rs"]
mod tests;
