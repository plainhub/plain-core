//! The `ftyp` box and the brands this crate recognises.

use crate::boxes::BoxIter;
use crate::error::{Error, Result};

/// A file's declared family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Brand {
    /// `heic`: HEVC-coded still image, the ordinary iPhone photograph.
    Heic,
    /// `heix`: HEVC still image with extended profiles (10-bit and up).
    Heix,
    /// `heim`: HEVC multiview still image.
    Heim,
    /// `heis`: HEVC scalable still image.
    Heis,
    /// `hevc`: HEVC image sequence.
    Hevc,
    /// `mif1`: the generic HEIF image file brand.
    Mif1,
    /// `msf1`: the generic HEIF image sequence brand.
    Msf1,
    /// `avif` and `avis`: AV1 in the HEIF container.
    Avif,
}

impl Brand {
    /// Map a four-character code onto a known brand.
    pub fn from_fourcc(code: &[u8; 4]) -> Option<Brand> {
        Some(match code {
            b"heic" => Brand::Heic,
            b"heix" => Brand::Heix,
            b"heim" => Brand::Heim,
            b"heis" => Brand::Heis,
            b"hevc" => Brand::Hevc,
            b"mif1" => Brand::Mif1,
            b"msf1" => Brand::Msf1,
            b"avif" | b"avis" => Brand::Avif,
            _ => return None,
        })
    }

    /// The four-character code for this brand.
    pub const fn fourcc(self) -> &'static [u8; 4] {
        match self {
            Brand::Heic => b"heic",
            Brand::Heix => b"heix",
            Brand::Heim => b"heim",
            Brand::Heis => b"heis",
            Brand::Hevc => b"hevc",
            Brand::Mif1 => b"mif1",
            Brand::Msf1 => b"msf1",
            Brand::Avif => b"avif",
        }
    }
}

/// The parsed `ftyp` box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileType {
    /// The brand the writer considers definitive.
    pub major: Brand,
    /// The brand this crate chose to work with, drawn from the major brand or
    /// from the compatible list.
    pub effective: Brand,
    /// Version of the major brand, as written.
    pub minor_version: u32,
    /// How many compatible brands were listed, recognised or not.
    pub compatible_count: usize,
}

/// Parse the `ftyp` box at the head of the file.
pub fn parse(data: &[u8]) -> Result<FileType> {
    let ftyp = BoxIter::new(data).require(b"ftyp", "ftyp")?;
    let mut r = ftyp.reader();
    let major_code = r.fourcc("ftyp major brand")?;
    let minor_version = r.u32("ftyp minor version")?;
    let major = Brand::from_fourcc(&major_code);

    let mut compatible_count = 0usize;
    let mut fallback: Option<Brand> = None;
    let mut saw_avif = false;
    while r.remaining() >= 4 {
        let code = r.fourcc("ftyp compatible brand")?;
        compatible_count += 1;
        if let Some(b) = Brand::from_fourcc(&code) {
            saw_avif |= b == Brand::Avif;
            if fallback.is_none() {
                fallback = Some(b);
            }
        }
    }

    let major = match major {
        Some(b) => b,
        None => fallback.ok_or(Error::Unsupported(
            "the file's brand is not a HEIF brand this crate recognises",
        ))?,
    };
    // AVIF may use `mif1` as its major brand and identify AV1 in the compatible list.
    let effective = if major == Brand::Avif || saw_avif {
        Brand::Avif
    } else {
        major
    };
    Ok(FileType {
        major,
        effective,
        minor_version,
        compatible_count,
    })
}
