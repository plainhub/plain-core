//! Minimal EXIF orientation reader for JPEG (APP1 segment only).
//!
//! Full EXIF parsing is out of scope; we only need IFD0 tag `0x0112`
//! (Orientation) so generated thumbnails match how phones/cameras display
//! the photo. Runs on the already-read header buffer — zero extra I/O.

/// EXIF orientation value (1 = normal, 2..8 = mirrored/rotated).
/// `0` is treated as absent/normal by callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Orientation(pub u8);

impl Orientation {
    pub const NORMAL: Orientation = Orientation(1);

    /// Whether the stored image needs its axes swapped to display upright.
    pub fn swaps_axes(self) -> bool {
        matches!(self.0, 5..=8)
    }
}

impl Default for Orientation {
    fn default() -> Self {
        Self::NORMAL
    }
}

pub fn orientation(head: &[u8]) -> Option<u8> {
    plain_rs::utils::image_metadata::orientation(head).map(|value| value as u8)
}

/// Build a synthetic APP1 segment carrying the given orientation. Used by
/// tests to produce JPEGs with a known EXIF orientation.
#[cfg(test)]
pub(crate) fn test_app1_orientation(v: u8) -> Vec<u8> {
    let mut tiff: Vec<u8> = b"II".to_vec();
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 at offset 8
    tiff.extend_from_slice(&1u16.to_le_bytes()); // 1 entry
    tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // tag
    tiff.extend_from_slice(&3u16.to_le_bytes()); // SHORT
    tiff.extend_from_slice(&1u32.to_le_bytes()); // count
    tiff.extend_from_slice(&(v as u16).to_le_bytes());
    tiff.extend_from_slice(&[0, 0]); // value padding
    tiff.extend_from_slice(&0u32.to_be_bytes()); // next IFD = 0

    let mut app1 = vec![0xFF, 0xE1];
    let payload_len = 6 + tiff.len(); // "Exif\0\0" + TIFF
    app1.extend_from_slice(&((payload_len + 2) as u16).to_be_bytes());
    app1.extend_from_slice(b"Exif\0\0");
    app1.extend_from_slice(&tiff);
    app1
}

#[cfg(test)]
#[path = "../../../tests/unit/media/thumbnails/exif.rs"]
mod tests;
