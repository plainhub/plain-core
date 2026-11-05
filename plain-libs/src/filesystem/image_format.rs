use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const PHOTO_EXTENSIONS: [&str; 13] = [
    "jpg", "jpeg", "png", "bmp", "webp", "heic", "heif", "apng", "avif", "gif", "tiff", "tif",
    "svg",
];

pub fn is_animated_image_or_svg(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !PHOTO_EXTENSIONS.contains(&ext.as_str()) {
        return false;
    }
    if ext == "svg" {
        return true;
    }
    if ext == "jpg" || ext == "jpeg" {
        return false;
    }

    let mut header = [0u8; 256];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    if ext == "png" || ext == "apng" {
        return is_animated_png(&mut file);
    }
    let n = std::io::Read::read(&mut file, &mut header).unwrap_or(0);
    let header = &header[..n];

    if n >= 6 && matches!(&header[..6], b"GIF87a" | b"GIF89a") {
        return true;
    }

    let b0 = header.first().copied();
    let b1 = header.get(1).copied();
    if b0 == Some(0xFF) && b1 == Some(0xD8) || b0 == Some(0x89) && b1 == Some(b'P') {
        return false;
    }

    if n >= 17 && &header[0..4] == b"RIFF" && &header[8..12] == b"WEBP" {
        return &header[12..16] == b"VP8X" && n > 17 && (header[16] & 0b10) != 0;
    }

    if n >= 12 && &header[4..8] == b"ftyp" {
        return is_animated_isobmff(&mut file);
    }

    header.windows(4).any(|w| w == b"<svg")
}

fn is_animated_isobmff(file: &mut (impl Read + Seek)) -> bool {
    if file.seek(SeekFrom::Start(0)).is_err() {
        return false;
    }
    let mut header = [0; 8];
    if file.read_exact(&mut header).is_err() || &header[4..] != b"ftyp" {
        return false;
    }
    let size = u32::from_be_bytes(header[..4].try_into().unwrap());
    let (size, header_size) = if size == 1 {
        let mut extended = [0; 8];
        if file.read_exact(&mut extended).is_err() {
            return false;
        }
        (u64::from_be_bytes(extended), 16)
    } else {
        (u64::from(size), 8)
    };
    let sequence = |brand: &[u8]| matches!(brand, b"avis" | b"msf1" | b"hevc" | b"hevx");
    let mut brand = [0; 4];
    if file.read_exact(&mut brand).is_err() {
        return false;
    }
    if sequence(&brand) {
        return true;
    }
    let size = if size == 0 {
        let Ok(size) = file.seek(SeekFrom::End(0)) else {
            return false;
        };
        size
    } else {
        size
    };
    let Some(mut remaining) = size.checked_sub(header_size + 8) else {
        return false;
    };
    if remaining % 4 != 0 || file.seek(SeekFrom::Start(header_size + 8)).is_err() {
        return false;
    }
    let mut brands = [0; 512];
    while remaining != 0 {
        let length = remaining.min(brands.len() as u64) as usize;
        if file.read_exact(&mut brands[..length]).is_err() {
            return false;
        }
        if brands[..length].chunks_exact(4).any(sequence) {
            return true;
        }
        remaining -= length as u64;
    }
    false
}

fn is_animated_png(file: &mut (impl Read + Seek)) -> bool {
    let mut signature = [0; 8];
    if file.read_exact(&mut signature).is_err() || signature != *b"\x89PNG\r\n\x1a\n" {
        return false;
    }
    let mut chunk = [0; 8];
    while file.read_exact(&mut chunk).is_ok() {
        let length = u32::from_be_bytes(chunk[..4].try_into().unwrap());
        match &chunk[4..] {
            b"IDAT" | b"IEND" => return false,
            b"acTL" => {
                let mut animation = [0; 8];
                return length == 8
                    && file.read_exact(&mut animation).is_ok()
                    && u32::from_be_bytes(animation[..4].try_into().unwrap()) > 0;
            }
            _ => {
                if file.seek(SeekFrom::Current(i64::from(length) + 4)).is_err() {
                    return false;
                }
            }
        }
    }
    false
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/filesystem/image_format.rs"]
mod tests;
