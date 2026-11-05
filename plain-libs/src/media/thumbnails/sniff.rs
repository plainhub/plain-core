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

fn be16(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]))
}
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
        return sniff_jpeg(buf);
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

/// Walk JPEG marker segments until a SOF frame header yields the dimensions.
fn sniff_jpeg(buf: &[u8]) -> Result<Sniffed, SniffError> {
    let sof =
        |m: u8| -> bool { matches!(m, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) };
    let mut i = 2usize;
    while i < buf.len() {
        if buf[i] != 0xFF {
            // Tolerate garbage between segments by scanning to the next marker.
            i += 1;
            continue;
        }
        // Skip fill bytes (repeated 0xFF padding before the marker code).
        let mut m_pos = i + 1;
        while m_pos < buf.len() && buf[m_pos] == 0xFF {
            m_pos += 1;
        }
        let Some(&m) = buf.get(m_pos) else {
            return Err(SniffError::Truncated);
        };
        match m {
            // Standalone markers without a length payload.
            0xD8 | 0x01 | 0xD0..=0xD7 => {
                i = m_pos + 1;
                continue;
            }
            _ if sof(m) => {
                // Layout after the marker code: len(2) precision(1)
                // height(2) width(2).
                let h = u32::from(be16(buf, m_pos + 4).ok_or(SniffError::Truncated)?);
                let w = u32::from(be16(buf, m_pos + 6).ok_or(SniffError::Truncated)?);
                if w == 0 || h == 0 {
                    return Err(SniffError::Unknown);
                }
                return Ok(Sniffed {
                    kind: ImageKind::Jpeg,
                    width: w,
                    height: h,
                    progressive: m == 0xC2,
                    preserve_alpha: false,
                    orientation: 1,
                    work_bytes: 0,
                });
            }
            _ => {
                // Length-prefixed segment (APPn, DQT, DHT, ...): the 2-byte
                // BE length counts itself; skip to the next marker.
                let len = usize::from(be16(buf, m_pos + 1).ok_or(SniffError::Truncated)?);
                if len < 2 {
                    return Err(SniffError::Unknown);
                }
                i = m_pos + len + 1;
            }
        }
    }
    Err(SniffError::Truncated)
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
