//! Image header sniffing: format classification + dimensions from the first
//! bytes of a file, without decoding pixel data.
//!
//! This powers three things in the thumbnail engine:
//! 1. admission-control pricing (decoded size estimate before committing),
//! 2. the small-image passthrough decision,
//! 3. decompression-bomb rejection (absurd dimensions → refuse early).

/// Image formats the engine can classify from a header alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Jpeg,
    Png,
    Gif,
    WebP,
    Bmp,
    Tiff,
    Avif,
    Heif,
}

impl ImageKind {
    /// MIME type used when raw source bytes are served as-is.
    pub fn mime(self) -> &'static str {
        match self {
            ImageKind::Jpeg => "image/jpeg",
            ImageKind::Png => "image/png",
            ImageKind::Gif => "image/gif",
            ImageKind::WebP => "image/webp",
            ImageKind::Bmp => "image/bmp",
            ImageKind::Tiff => "image/tiff",
            ImageKind::Avif => "image/avif",
            ImageKind::Heif => "image/heic",
        }
    }

    pub fn can_passthrough(self) -> bool {
        !matches!(
            self,
            ImageKind::Bmp | ImageKind::Tiff | ImageKind::Avif | ImageKind::Heif
        )
    }
}

/// Header facts extracted without pixel decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sniffed {
    pub kind: ImageKind,
    pub width: u32,
    pub height: u32,
    /// JPEG-only: progressive DCT scan order.
    pub progressive: bool,
    pub preserve_alpha: bool,
    pub orientation: u8,
    pub work_bytes: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SniffError {
    /// Fewer bytes than needed for classification.
    Truncated,
    /// Not a recognized image header.
    Unknown,
}

impl std::fmt::Display for SniffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SniffError::Truncated => write!(f, "truncated image header"),
            SniffError::Unknown => write!(f, "unrecognized image header"),
        }
    }
}

impl std::error::Error for SniffError {}

use std::io::{BufRead, Cursor, Seek, SeekFrom};
fn le16(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

/// Classify a header buffer. Only the leading bytes are required; passing the
/// whole file is fine but wasteful (`read_head` caps at 256 KiB).
pub fn sniff_header(buf: &[u8]) -> Result<Sniffed, SniffError> {
    if buf.len() < 12 {
        return Err(SniffError::Truncated);
    }
    if buf.starts_with(&[0xFF, 0xD8]) {
        return sniff_jpeg(&mut Cursor::new(buf), false);
    }
    if buf.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        // IHDR must be the first chunk: length(4) + "IHDR" then BE w/h.
        let dimensions = buf.get(16..24).ok_or(SniffError::Truncated)?;
        let w = u32::from_be_bytes(dimensions[..4].try_into().unwrap());
        let h = u32::from_be_bytes(dimensions[4..].try_into().unwrap());
        if w == 0 || h == 0 {
            return Err(SniffError::Unknown);
        }
        return Ok(Sniffed {
            kind: ImageKind::Png,
            width: w,
            height: h,
            progressive: false,
            preserve_alpha: true,
            orientation: 1,
            work_bytes: 0,
        });
    }
    if buf.starts_with(b"GIF87a") || buf.starts_with(b"GIF89a") {
        let w = u32::from(le16(buf, 6).ok_or(SniffError::Truncated)?);
        let h = u32::from(le16(buf, 8).ok_or(SniffError::Truncated)?);
        if w == 0 || h == 0 {
            return Err(SniffError::Unknown);
        }
        return Ok(Sniffed {
            kind: ImageKind::Gif,
            width: w,
            height: h,
            progressive: false,
            preserve_alpha: false,
            orientation: 1,
            work_bytes: 0,
        });
    }
    if buf.starts_with(b"RIFF") && buf.get(8..12) == Some(b"WEBP") {
        return sniff_webp(buf);
    }
    if buf.starts_with(b"BM") {
        let rd = |i: usize| -> Option<i32> {
            let b = buf.get(i..i + 4)?;
            Some(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let (w, h) = if rd(14) == Some(12) {
            (
                u32::from(le16(buf, 18).ok_or(SniffError::Truncated)?),
                u32::from(le16(buf, 20).ok_or(SniffError::Truncated)?),
            )
        } else {
            let w = rd(18).ok_or(SniffError::Truncated)?;
            let h = rd(22).ok_or(SniffError::Truncated)?;
            if w <= 0 || h == i32::MIN {
                return Err(SniffError::Unknown);
            }
            (w as u32, h.unsigned_abs())
        };
        if w == 0 || h == 0 || w > 65_535 || h > 65_535 {
            return Err(SniffError::Unknown);
        }
        return Ok(Sniffed {
            kind: ImageKind::Bmp,
            width: w,
            height: h,
            progressive: false,
            preserve_alpha: super::bmp::has_alpha(buf),
            orientation: 1,
            work_bytes: 0,
        });
    }
    Err(SniffError::Unknown)
}

pub(super) fn sniff_jpeg(
    reader: &mut (impl BufRead + Seek),
    read_orientation: bool,
) -> Result<Sniffed, SniffError> {
    use crate::binary::{BigEndian, ReadBytesExt};
    use crate::image::metadata::Orientation;
    use crate::utils::range_reader::RangeReader;

    reader.seek(SeekFrom::Start(2)).map_err(|_| SniffError::Truncated)?;
    let mut source = None;
    let mut orientation = None;
    while reader.skip_until(0xff).map_err(|_| SniffError::Truncated)? != 0 {
        let mut marker = reader.read_u8().map_err(|_| SniffError::Truncated)?;
        while marker == 0xff {
            marker = reader.read_u8().map_err(|_| SniffError::Truncated)?;
        }
        match marker {
            0xda | 0xd9 => break,
            0xd8 | 0x01 | 0xd0..=0xd7 => continue,
            _ => {}
        }
        let length = reader.read_u16::<BigEndian>().map_err(|_| SniffError::Truncated)?
            .checked_sub(2).ok_or(SniffError::Unknown)?;
        let start = reader.stream_position().map_err(|_| SniffError::Truncated)?;
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            if length < 5 {
                return Err(SniffError::Truncated);
            }
            let mut shape = [0; 5];
            reader.read_exact(&mut shape).map_err(|_| SniffError::Truncated)?;
            let width = u32::from(u16::from_be_bytes([shape[3], shape[4]]));
            let height = u32::from(u16::from_be_bytes([shape[1], shape[2]]));
            if width == 0 || height == 0 {
                return Err(SniffError::Unknown);
            }
            source = Some(Sniffed {
                kind: ImageKind::Jpeg,
                width,
                height,
                progressive: marker == 0xc2,
                preserve_alpha: false,
                orientation: 1,
                work_bytes: 0,
            });
        } else if read_orientation && marker == 0xe1 && length >= 6 {
            let mut exif = [0; 6];
            reader.read_exact(&mut exif).map_err(|_| SniffError::Truncated)?;
            if &exif == b"Exif\0\0" {
                let mut tiff = RangeReader::new(&mut *reader, start + 6, u64::from(length - 6))
                    .map_err(|_| SniffError::Truncated)?;
                orientation = Orientation::from_exif_reader(&mut tiff);
            }
        }
        if let Some(mut source) = source
            && (!read_orientation || orientation.is_some())
        {
            source.orientation = orientation.map_or(1, Orientation::to_exif);
            return Ok(source);
        }
        reader.seek(SeekFrom::Start(start + u64::from(length)))
            .map_err(|_| SniffError::Truncated)?;
    }
    source.ok_or(SniffError::Truncated)
}

fn sniff_webp(buf: &[u8]) -> Result<Sniffed, SniffError> {
    use crate::utils::image_dimensions::{WebpHeaderError, webp_header};
    let header = webp_header(buf).map_err(|error| match error {
        WebpHeaderError::Truncated => SniffError::Truncated,
        WebpHeaderError::Invalid => SniffError::Unknown,
    })?;
    Ok(Sniffed {
        kind: ImageKind::WebP,
        width: header.width,
        height: header.height,
        progressive: false,
        preserve_alpha: header.has_alpha,
        orientation: 1,
        work_bytes: 0,
    })
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/thumbnails/sniff.rs"]
mod tests;
