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
    if ext == "png" || ext == "jpg" || ext == "jpeg" {
        return false;
    }

    let mut header = [0u8; 256];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
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
        return matches!(&header[8..12], b"msf1" | b"hevc" | b"hevx");
    }

    header.windows(4).any(|w| w == b"<svg")
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/files/filesystem/image_format.rs"]
mod tests;
