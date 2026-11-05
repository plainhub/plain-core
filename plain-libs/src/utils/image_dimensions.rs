//! Hand-rolled image dimension parsing from raw bytes.
//!
//! Returns `(width, height)` for JPEG / PNG / GIF / BMP / WebP / ICO /
//! TIFF by reading the format's magic bytes and header fields directly —
//! mirrors `plain-app`'s `platform/getImageDimensions` (Android
//! `BitmapFactory` behaviour, including ICO where a `0` entry encodes
//! 256). Also provides [`exif_gps`], a hand-rolled EXIF GPS reader for
//! JPEG (APP1 `Exif\0\0`) and TIFF payloads. Anything undecodable yields
//! `None`.

/// Returns `(width, height)` for the image in `bytes`, or `None` when
/// `bytes` is not one of the supported formats.
pub fn dimensions(bytes: &[u8]) -> Option<(i32, i32)> {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        return jpeg(bytes);
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return png(bytes);
    }
    if bytes.len() >= 6 && (&bytes[..6] == b"GIF87a" || &bytes[..6] == b"GIF89a") {
        return gif(bytes);
    }
    if bytes.starts_with(b"BM") {
        return bmp(bytes);
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return webp_header(bytes)
            .ok()
            .map(|header| (header.width as i32, header.height as i32));
    }
    // TIFF (little- or big-endian).
    if bytes.starts_with(b"II") || bytes.starts_with(b"MM") {
        return tiff(bytes);
    }
    // ICO/CUR container: reserved(0,0) + type(1=icon) + image count (>0).
    if bytes.len() >= 8 && bytes[0] == 0 && bytes[1] == 0 && bytes[2] == 1 {
        return ico(bytes);
    }
    None
}

/// Extract GPS coordinates from the EXIF metadata of a JPEG
/// (APP1 `Exif\0\0` segment) or a TIFF payload. Returns
/// `(latitude, longitude)` in signed decimal degrees, or `None` when the
/// image carries no usable GPS IFD. Ported from plain-desktop
/// `file_query.rs` (used for photo locations in chat / media info).
pub fn exif_gps(bytes: &[u8]) -> Option<(f64, f64)> {
    let tiff: &[u8] = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        exif_tiff_from_jpeg(bytes)?
    } else {
        bytes
    };
    let (le, ifd0_off) = parse_tiff_header(tiff)?;
    let gps_off = tiff_find_sub_ifd(tiff, ifd0_off, le, 0x8825)?;
    let lat = tiff_read_dms(tiff, gps_off, le, 0x0002, 0x0001)?;
    let lon = tiff_read_dms(tiff, gps_off, le, 0x0004, 0x0003)?;
    Some((lat, lon))
}

/// JPEG dimensions from the first SOF marker. Returns `None` when no
/// SOF marker is found before SOS / EOI.
fn jpeg(b: &[u8]) -> Option<(i32, i32)> {
    let mut i = 2usize;
    while i + 1 < b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        // Markers can be repeated (0xFF 0xFF) — skip padding fills.
        while i < b.len() && b[i] == 0xFF {
            i += 1;
        }
        if i >= b.len() {
            return None;
        }
        let m = b[i];
        i += 1;
        // SOF markers (Start Of Frame), except 0xC4 (DHT) and 0xC8 (JPG).
        if (0xC0..=0xCF).contains(&m) && m != 0xC4 && m != 0xC8 {
            if i + 7 > b.len() {
                return None;
            }
            // Precision (1) + height (2) + width (2) follow the segment length.
            let h = u16::from_be_bytes([b[i + 3], b[i + 4]]) as i32;
            let w = u16::from_be_bytes([b[i + 5], b[i + 6]]) as i32;
            return Some((w, h));
        }
        // SOS (0xDA) or EOI (0xD9) — no more dimension markers ahead.
        if m == 0xDA || m == 0xD9 {
            return None;
        }
        if i + 2 > b.len() {
            return None;
        }
        let seg = u16::from_be_bytes([b[i], b[i + 1]]) as usize;
        if seg < 2 {
            return None;
        }
        i += 2 + (seg - 2);
    }
    None
}

/// PNG dimensions from the IHDR chunk.
fn png(b: &[u8]) -> Option<(i32, i32)> {
    if b.len() < 24 {
        return None;
    }
    if &b[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(b[16..20].try_into().ok()?) as i32;
    let h = u32::from_be_bytes(b[20..24].try_into().ok()?) as i32;
    Some((w, h))
}

/// GIF dimensions (logical screen descriptor width/height).
fn gif(b: &[u8]) -> Option<(i32, i32)> {
    if b.len() < 10 {
        return None;
    }
    let w = u16::from_le_bytes([b[6], b[7]]) as i32;
    let h = u16::from_le_bytes([b[8], b[9]]) as i32;
    Some((w, h))
}

/// BMP dimensions (width / height at the DIB header offsets 18 / 22).
fn bmp(b: &[u8]) -> Option<(i32, i32)> {
    if b.len() < 26 {
        return None;
    }
    let w = i32::from_le_bytes(b[18..22].try_into().ok()?);
    let h = i32::from_le_bytes(b[22..26].try_into().ok()?);
    Some((w.abs(), h.abs()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WebpHeader {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub animated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WebpHeaderError {
    Truncated,
    Invalid,
}

// https://developers.google.com/speed/webp/docs/riff_container
pub(crate) fn webp_header(buf: &[u8]) -> Result<WebpHeader, WebpHeaderError> {
    let signature = buf.get(..12).ok_or(WebpHeaderError::Truncated)?;
    if &signature[..4] != b"RIFF" || &signature[8..] != b"WEBP" {
        return Err(WebpHeaderError::Invalid);
    }
    let chunk_size = u32::from_le_bytes(
        buf.get(16..20)
            .ok_or(WebpHeaderError::Truncated)?
            .try_into()
            .unwrap(),
    );
    let file_size = u32::from_le_bytes(buf[4..8].try_into().unwrap());
    if u64::from(chunk_size) + 12 + u64::from(chunk_size & 1) > u64::from(file_size) {
        return Err(WebpHeaderError::Invalid);
    }
    let (width, height, has_alpha, animated) = match buf.get(12..16) {
        Some(b"VP8X") => {
            if chunk_size < 10 {
                return Err(WebpHeaderError::Invalid);
            }
            let header = buf.get(20..30).ok_or(WebpHeaderError::Truncated)?;
            let width = 1 + u32::from_le_bytes([header[4], header[5], header[6], 0]);
            let height = 1 + u32::from_le_bytes([header[7], header[8], header[9], 0]);
            if u64::from(width) * u64::from(height) > u64::from(u32::MAX) {
                return Err(WebpHeaderError::Invalid);
            }
            (width, height, header[0] & 0x10 != 0, header[0] & 0x02 != 0)
        }
        Some(b"VP8 ") => {
            if chunk_size < 10 {
                return Err(WebpHeaderError::Invalid);
            }
            let header = buf.get(20..30).ok_or(WebpHeaderError::Truncated)?;
            if header[0] & 1 != 0 || header[3..6] != [0x9d, 0x01, 0x2a] {
                return Err(WebpHeaderError::Invalid);
            }
            let width = u32::from(u16::from_le_bytes([header[6], header[7]]) & 0x3fff);
            let height = u32::from(u16::from_le_bytes([header[8], header[9]]) & 0x3fff);
            if width == 0 || height == 0 {
                return Err(WebpHeaderError::Invalid);
            }
            (width, height, false, false)
        }
        Some(b"VP8L") => {
            // https://developers.google.com/speed/webp/docs/webp_lossless_bitstream_specification#riff_header
            if chunk_size < 5 {
                return Err(WebpHeaderError::Invalid);
            }
            let header = buf.get(20..25).ok_or(WebpHeaderError::Truncated)?;
            if header[0] != 0x2f {
                return Err(WebpHeaderError::Invalid);
            }
            let packed = u32::from_le_bytes(header[1..].try_into().unwrap());
            if packed >> 29 != 0 {
                return Err(WebpHeaderError::Invalid);
            }
            (
                (packed & 0x3fff) + 1,
                ((packed >> 14) & 0x3fff) + 1,
                packed & (1 << 28) != 0,
                false,
            )
        }
        _ => return Err(WebpHeaderError::Invalid),
    };
    Ok(WebpHeader {
        width,
        height,
        has_alpha,
        animated,
    })
}

/// ICO dimensions from the first `ICONDIRENTRY` (`0` encodes 256).
/// Matches Android's `BitmapFactory` result for `.ico` favicons.
fn ico(b: &[u8]) -> Option<(i32, i32)> {
    let count = u16::from_le_bytes([b[4], b[5]]);
    if count == 0 {
        return None;
    }
    // First entry starts at offset 6; width / height are its first two bytes.
    let w = b[6] as i32;
    let h = b[7] as i32;
    Some((if w == 0 { 256 } else { w }, if h == 0 { 256 } else { h }))
}

// ── TIFF dimensions + EXIF GPS (ported from plain-desktop file_query.rs) ─────

fn tiff(b: &[u8]) -> Option<(i32, i32)> {
    let (le, ifd_off) = parse_tiff_header(b)?;
    let count = tiff_read_u16(b, ifd_off as usize, le)? as usize;
    let base = ifd_off as usize + 2;
    let mut width: Option<i32> = None;
    let mut height: Option<i32> = None;
    for i in 0..count {
        let entry = base + i * 12;
        if entry + 12 > b.len() {
            return None;
        }
        let tag = tiff_read_u16(b, entry, le)?;
        if tag == 0x0100 {
            width = tiff_ifd_value_i32(b, entry, le);
        } else if tag == 0x0101 {
            height = tiff_ifd_value_i32(b, entry, le);
        }
        if width.is_some() && height.is_some() {
            break;
        }
    }
    Some((width?, height?))
}

/// Parse the 8-byte TIFF header: byte order + magic 42 + first IFD offset.
pub(super) fn parse_tiff_header(tiff: &[u8]) -> Option<(bool, u32)> {
    if tiff.len() < 8 {
        return None;
    }
    let (le, magic) = match &tiff[0..2] {
        b"II" => (true, 0x002A),
        b"MM" => (false, 0x002A),
        _ => return None,
    };
    let expected = if le {
        u16::from_le_bytes([tiff[2], tiff[3]])
    } else {
        u16::from_be_bytes([tiff[2], tiff[3]])
    };
    if expected != magic {
        return None;
    }
    let off = if le {
        u32::from_le_bytes([tiff[4], tiff[5], tiff[6], tiff[7]])
    } else {
        u32::from_be_bytes([tiff[4], tiff[5], tiff[6], tiff[7]])
    };
    Some((le, off))
}

pub(super) fn tiff_read_u16(tiff: &[u8], off: usize, le: bool) -> Option<u16> {
    let b = tiff.get(off..off + 2)?;
    Some(if le {
        u16::from_le_bytes([b[0], b[1]])
    } else {
        u16::from_be_bytes([b[0], b[1]])
    })
}

pub(super) fn tiff_read_u32(tiff: &[u8], off: usize, le: bool) -> Option<u32> {
    let b = tiff.get(off..off + 4)?;
    Some(if le {
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    } else {
        u32::from_be_bytes([b[0], b[1], b[2], b[3]])
    })
}

/// Decode a SHORT (type 3) or LONG (type 4) IFD value that fits inline in
/// the entry's 4-byte value field.
fn tiff_ifd_value_i32(tiff: &[u8], entry: usize, le: bool) -> Option<i32> {
    let typ = tiff_read_u16(tiff, entry + 2, le)?;
    match typ {
        3 => Some(tiff_read_u16(tiff, entry + 8, le)? as i32),
        4 => Some(tiff_read_u32(tiff, entry + 8, le)? as i32),
        _ => None,
    }
}

/// Find a sub-IFD (e.g. GPS IFD) by tag id in the IFD at `ifd_off`. Returns
/// the offset of the sub-IFD's first entry.
fn tiff_find_sub_ifd(tiff: &[u8], ifd_off: u32, le: bool, sub_tag: u16) -> Option<u32> {
    let count = tiff_read_u16(tiff, ifd_off as usize, le)? as usize;
    let base = ifd_off as usize + 2;
    for i in 0..count {
        let entry = base + i * 12;
        if entry + 12 > tiff.len() {
            return None;
        }
        let tag = tiff_read_u16(tiff, entry, le)?;
        if tag == sub_tag {
            // Type 4 (LONG) value is stored inline in the 4-byte value field.
            return tiff_read_u32(tiff, entry + 8, le);
        }
    }
    None
}

/// Read a GPS DMS coordinate (RATIONAL × 3) and apply the matching
/// `*Ref` (ASCII "N"/"S"/"E"/"W") tag to derive a signed decimal.
fn tiff_read_dms(tiff: &[u8], ifd_off: u32, le: bool, dms_tag: u16, ref_tag: u16) -> Option<f64> {
    let (dms, ref_marker) = tiff_find_dms_and_ref(tiff, ifd_off, le, dms_tag, ref_tag)?;
    let mut iter = dms.iter();
    let d = iter.next()?.to_f64();
    let m = iter.next()?.to_f64();
    let s = iter.next()?.to_f64();
    let mut decimal = d + m / 60.0 + s / 3600.0;
    if ref_marker.contains('S') || ref_marker.contains('W') {
        decimal = -decimal;
    }
    Some(decimal)
}

fn tiff_find_dms_and_ref(
    tiff: &[u8],
    ifd_off: u32,
    le: bool,
    dms_tag: u16,
    ref_tag: u16,
) -> Option<(Vec<TiffRational>, String)> {
    let count = tiff_read_u16(tiff, ifd_off as usize, le)? as usize;
    let base = ifd_off as usize + 2;
    let mut dms: Option<Vec<TiffRational>> = None;
    let mut ref_marker: Option<String> = None;
    for i in 0..count {
        let entry = base + i * 12;
        if entry + 12 > tiff.len() {
            return None;
        }
        let tag = tiff_read_u16(tiff, entry, le)?;
        let typ = tiff_read_u16(tiff, entry + 2, le)?;
        if tag == dms_tag && typ == 5 {
            let off = tiff_read_u32(tiff, entry + 8, le)? as usize;
            dms = Some(tiff_read_rationals(tiff, off, le, 3)?);
        } else if tag == ref_tag && typ == 2 {
            // ASCII: value is N bytes starting at `off` (or inline if
            // count ≤ 4). Use whichever is shorter.
            let count_bytes = tiff_read_u32(tiff, entry + 4, le)? as usize;
            let buf: &[u8] = if count_bytes <= 4 {
                &tiff[entry + 8..entry + 8 + count_bytes]
            } else {
                let off = tiff_read_u32(tiff, entry + 8, le)? as usize;
                tiff.get(off..off + count_bytes)?
            };
            ref_marker = Some(
                String::from_utf8_lossy(buf)
                    .trim_end_matches('\0')
                    .to_string(),
            );
        }
        if dms.is_some() && ref_marker.is_some() {
            break;
        }
    }
    Some((dms?, ref_marker.unwrap_or_default()))
}

fn tiff_read_rationals(
    tiff: &[u8],
    off: usize,
    le: bool,
    count: usize,
) -> Option<Vec<TiffRational>> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let p = off + i * 8;
        if p + 8 > tiff.len() {
            return None;
        }
        let n = tiff_read_u32(tiff, p, le)?;
        let d = tiff_read_u32(tiff, p + 4, le)?;
        out.push(TiffRational { n, d });
    }
    Some(out)
}

/// Hand-rolled RATIONAL: 32-bit unsigned numerator / denominator.
#[derive(Clone, Copy)]
struct TiffRational {
    n: u32,
    d: u32,
}

impl TiffRational {
    fn to_f64(self) -> f64 {
        if self.d == 0 {
            0.0
        } else {
            (self.n as f64) / (self.d as f64)
        }
    }
}

/// Find the `Exif\0\0` marker in a JPEG's APP1 segment and return the
/// TIFF payload that follows it. Returns `None` if there is no EXIF.
pub(super) fn exif_tiff_from_jpeg(jpeg: &[u8]) -> Option<&[u8]> {
    if jpeg.len() < 4 || jpeg[0..2] != [0xFF, 0xD8] {
        return None;
    }
    let mut i = 2;
    while i + 4 <= jpeg.len() {
        if jpeg[i] != 0xFF {
            return None;
        }
        // Skip 0xFF padding.
        let mut marker = jpeg[i + 1];
        while marker == 0xFF && i + 2 < jpeg.len() {
            i += 1;
            marker = jpeg[i + 1];
        }
        i += 2;
        if marker == 0xD9 || marker == 0xDA {
            return None;
        }
        if i + 2 > jpeg.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([jpeg[i], jpeg[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > jpeg.len() {
            return None;
        }
        let payload = &jpeg[i + 2..i + seg_len];
        if marker == 0xE1 && payload.starts_with(b"Exif\0\0") {
            return Some(&payload[6..]);
        }
        i += seg_len;
    }
    None
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/image_dimensions.rs"]
mod tests;
