use super::image_dimensions::{
    exif_tiff_from_jpeg, parse_tiff_header, tiff_read_u16, tiff_read_u32,
};
use serde::Serialize;

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMetadata {
    pub make: String,
    pub model: String,
    pub rotation: i32,
    #[serde(skip)]
    pub orientation: Option<u16>,
    pub color_space: String,
    pub aperture_value: f64,
    pub exposure_time: String,
    pub focal_length: String,
    pub iso_speed: i32,
    pub raw_taken_at: String,
    pub offset_time: String,
    pub flash: i32,
    pub f_number: f64,
    pub exposure_program: i32,
    pub metering_mode: i32,
    pub white_balance: i32,
    pub creator: String,
    pub resolution_x: i32,
    pub resolution_y: i32,
    pub description: String,
}
#[cfg(feature = "time")]
impl ImageMetadata {
    pub fn taken_at(&self) -> Option<String> {
        let offset = if self.offset_time.is_empty() {
            "+00:00"
        } else {
            &self.offset_time
        };
        crate::chrono::DateTime::parse_from_str(
            &format!("{} {offset}", self.raw_taken_at),
            "%Y:%m:%d %H:%M:%S %:z",
        )
        .ok()
        .map(|date| {
            date.with_timezone(&crate::chrono::Utc)
                .to_rfc3339_opts(crate::chrono::SecondsFormat::Millis, true)
        })
    }
}
struct Field<'a> {
    bytes: &'a [u8],
    kind: u16,
    little: bool,
}
impl Field<'_> {
    fn text(&self) -> String {
        if self.kind != 2 {
            return String::new();
        }
        String::from_utf8_lossy(self.bytes)
            .trim_end_matches('\0')
            .trim()
            .to_owned()
    }
    fn number(&self) -> Option<f64> {
        let read = |offset| tiff_read_u32(self.bytes, offset, self.little);
        match self.kind {
            3 => Some(f64::from(tiff_read_u16(self.bytes, 0, self.little)?)),
            4 => Some(f64::from(read(0)?)),
            5 | 10 => {
                let (a, b) = (read(0)?, read(4)?);
                let (a, b) = if self.kind == 10 {
                    (f64::from(a as i32), f64::from(b as i32))
                } else {
                    (f64::from(a), f64::from(b))
                };
                (b != 0.0).then_some(a / b)
            }
            _ => None,
        }
    }
    fn integer(&self) -> i32 {
        self.number()
            .filter(|v| *v >= 0.0 && *v <= i32::MAX as f64)
            .unwrap_or(0.0) as i32
    }
}
fn visit(tiff: &[u8], offset: u32, little: bool, mut field: impl FnMut(u16, Field<'_>)) {
    let offset = offset as usize;
    let Some(count) = tiff_read_u16(tiff, offset, little) else {
        return;
    };
    for index in 0..usize::from(count) {
        let Some(entry) = offset
            .checked_add(2)
            .and_then(|v| v.checked_add(index * 12))
        else {
            break;
        };
        let Some(raw) = tiff.get(entry..).and_then(|v| v.get(..12)) else {
            break;
        };
        let tag = tiff_read_u16(raw, 0, little).unwrap();
        let kind = tiff_read_u16(raw, 2, little).unwrap();
        let unit = match kind {
            1 | 2 | 7 => 1,
            3 => 2,
            4 | 9 => 4,
            5 | 10 => 8,
            _ => continue,
        };
        let Some(length) = (tiff_read_u32(raw, 4, little).unwrap() as usize).checked_mul(unit)
        else {
            continue;
        };
        let start = if length <= 4 {
            entry + 8
        } else {
            tiff_read_u32(raw, 8, little).unwrap() as usize
        };
        let Some(bytes) = tiff.get(start..).and_then(|v| v.get(..length)) else {
            continue;
        };
        field(
            tag,
            Field {
                bytes,
                kind,
                little,
            },
        );
    }
}
fn tiff_payload(bytes: &[u8]) -> Option<&[u8]> {
    if bytes.starts_with(b"\xff\xd8") {
        return exif_tiff_from_jpeg(bytes);
    }
    if bytes.starts_with(b"II") || bytes.starts_with(b"MM") {
        return Some(bytes);
    }
    let png = bytes.starts_with(b"\x89PNG\r\n\x1a\n");
    let webp = bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP");
    let mut offset: usize = if png {
        8
    } else if webp {
        12
    } else {
        return None;
    };
    loop {
        let header = bytes.get(offset..)?.get(..8)?;
        let (tag, length) = if png {
            (
                &header[4..8],
                u32::from_be_bytes(header[..4].try_into().ok()?) as usize,
            )
        } else {
            (
                &header[..4],
                u32::from_le_bytes(header[4..8].try_into().ok()?) as usize,
            )
        };
        let start = offset.checked_add(8)?;
        let data = bytes.get(start..)?.get(..length)?;
        if tag == b"eXIf" || tag == b"EXIF" {
            return Some(data.strip_prefix(b"Exif\0\0").unwrap_or(data));
        }
        offset = start
            .checked_add(length)?
            .checked_add(if png { 4 } else { length % 2 })?;
    }
}
pub fn gps(bytes: &[u8]) -> Option<(f64, f64)> {
    super::image_dimensions::exif_gps(tiff_payload(bytes)?)
}
fn orientation_value(value: Field<'_>) -> Option<u16> {
    if value.kind != 3 || value.bytes.len() != 2 {
        return None;
    }
    let value = tiff_read_u16(value.bytes, 0, value.little)?;
    (1..=8).contains(&value).then_some(value)
}

pub fn orientation(bytes: &[u8]) -> Option<u16> {
    let tiff = tiff_payload(bytes)?;
    let (little, first) = parse_tiff_header(tiff)?;
    let mut result = None;
    visit(tiff, first, little, |tag, field| {
        if tag == 0x0112 {
            result = orientation_value(field);
        }
    });
    result
}

pub fn read(bytes: &[u8]) -> ImageMetadata {
    let mut result = ImageMetadata::default();
    let Some(tiff) = tiff_payload(bytes) else {
        return result;
    };
    let Some((little, first)) = parse_tiff_header(tiff) else {
        return result;
    };
    let mut exif = None;
    visit(tiff, first, little, |tag, value| match tag {
        0x8769 if value.kind == 4 => exif = tiff_read_u32(value.bytes, 0, little),
        0x010f => result.make = value.text(),
        0x0110 => result.model = value.text(),
        0x0112 => {
            result.orientation = orientation_value(value);
            result.rotation = match result.orientation {
                Some(3) => 180,
                Some(6) => 90,
                Some(8) => 270,
                _ => 0,
            }
        }
        0x011a => result.resolution_x = value.integer(),
        0x011b => result.resolution_y = value.integer(),
        0x013b => result.creator = value.text(),
        0x010e => result.description = value.text(),
        _ => {}
    });
    if let Some(exif) = exif {
        visit(tiff, exif, little, |tag, value| match tag {
            0x9003 => result.raw_taken_at = value.text(),
            0x9011 => result.offset_time = value.text(),
            0xa001 => {
                result.color_space = if value.integer() == 1 { "sRGB" } else { "RGB" }.to_owned()
            }
            0x9202 => result.aperture_value = value.number().unwrap_or(0.0),
            0x829a => {
                if let Some(seconds) = value.number().filter(|v| *v > 0.0) {
                    result.exposure_time = if seconds < 1.0 {
                        format!("1/{:.0}", 1.0 / seconds)
                    } else {
                        seconds.to_string()
                    };
                }
            }
            0x920a => {
                if let Some(number) = value.number() {
                    result.focal_length = number.to_string();
                }
            }
            0x8827 => result.iso_speed = value.integer(),
            0x9209 => result.flash = value.integer(),
            0x829d => result.f_number = value.number().unwrap_or(0.0),
            0x8822 => result.exposure_program = value.integer(),
            0x9207 => result.metering_mode = value.integer(),
            0xa403 => result.white_balance = value.integer(),
            _ => {}
        });
    }
    result
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/image_metadata.rs"]
mod tests;

pub fn thumbnail(jpeg: &[u8]) -> Option<&[u8]> {
    let tiff = exif_tiff_from_jpeg(jpeg)?;
    let (little, first) = parse_tiff_header(tiff)?;
    let count = usize::from(tiff_read_u16(tiff, first as usize, little)?);
    let next = (first as usize)
        .checked_add(2)?
        .checked_add(count.checked_mul(12)?)?;
    let thumbnail = tiff_read_u32(tiff, next, little)?;
    if thumbnail == 0 {
        return None;
    }
    let mut offset = None;
    let mut length = None;
    visit(tiff, thumbnail, little, |tag, field| {
        if field.kind != 4 {
            return;
        }
        if tag == 0x0201 {
            offset = tiff_read_u32(field.bytes, 0, little);
        }
        if tag == 0x0202 {
            length = tiff_read_u32(field.bytes, 0, little);
        }
    });
    let offset = offset? as usize;
    let end = offset.checked_add(length? as usize)?;
    let bytes = tiff.get(offset..end)?;
    bytes.starts_with(&[0xff, 0xd8]).then_some(bytes)
}
