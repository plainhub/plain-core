mod heif;
mod rows;
mod tiff;

use super::{
    scale::Bitmap,
    sniff::{ImageKind, Sniffed},
};
use crate::anyhow::Result;
use std::path::Path;

pub(super) fn kind(bytes: &[u8]) -> Option<ImageKind> {
    if bytes.starts_with(b"II\x2a\0")
        || bytes.starts_with(b"MM\0\x2a")
        || bytes.starts_with(b"II\x2b\0")
        || bytes.starts_with(b"MM\0\x2b")
    {
        return Some(ImageKind::Tiff);
    }
    if bytes.get(4..8) != Some(b"ftyp") {
        return None;
    }
    let length = u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?);
    let (offset, end) = match length {
        0 => (8, bytes.len()),
        1 => (
            16,
            usize::try_from(u64::from_be_bytes(bytes.get(8..16)?.try_into().ok()?))
                .unwrap_or(usize::MAX),
        ),
        length => (8, length as usize),
    };
    if end < offset + 8 {
        return None;
    }
    let brands = bytes.get(offset..end.min(bytes.len()))?;
    let major = brands.get(..4)?;
    let codes = std::iter::once(major).chain(brands.get(8..)?.chunks_exact(4));
    let mut heif = false;
    for code in codes {
        if matches!(code, b"avif" | b"avis") {
            return Some(ImageKind::Avif);
        }
        heif |= heic_rs::Brand::from_fourcc(code.try_into().ok()?).is_some();
    }
    heif.then_some(ImageKind::Heif)
}

pub(super) fn probe(path: &Path, kind: ImageKind) -> Result<Sniffed> {
    match kind {
        ImageKind::Tiff => tiff::probe(std::io::BufReader::new(std::fs::File::open(path)?)),
        _ => heif::probe(path, kind),
    }
}

pub(super) fn decode_file(path: &Path, kind: ImageKind) -> Result<Bitmap> {
    match kind {
        ImageKind::Tiff => tiff::decode(std::io::BufReader::new(std::fs::File::open(path)?)),
        _ => heif::decode(&std::fs::read(path)?),
    }
}

pub(super) fn decode_bytes(data: &[u8], kind: ImageKind) -> Result<Bitmap> {
    match kind {
        ImageKind::Tiff => tiff::decode(std::io::Cursor::new(data)),
        _ => heif::decode(data),
    }
}

pub(super) fn thumbnail(path: &Path, kind: ImageKind, tw: u32, th: u32) -> Result<Bitmap> {
    match kind {
        ImageKind::Tiff => {
            tiff::thumbnail(std::io::BufReader::new(std::fs::File::open(path)?), tw, th)
        }
        _ => heif::decode_unoriented(&std::fs::read(path)?),
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/media/thumbnails/still/benchmark.rs"]
mod benchmark;

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/media/thumbnails/still/mod.rs"]
mod tests;
