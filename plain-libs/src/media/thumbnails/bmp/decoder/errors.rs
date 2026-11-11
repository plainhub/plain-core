// Adapted from image 0.25.10 BMP decoder; see ../LICENSE-MIT and ../decoder.rs for provenance.
use super::{ImageError, ImageFormat, MAX_WIDTH_HEIGHT};
use crate::image::error::DecodingError;
use std::{error, fmt};
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum DecoderError {
    CorruptRleData,

    BitfieldMaskNonContiguous,
    BitfieldMaskInvalid,
    BitfieldMaskMissing(u32),
    BitfieldMasksMissing(u32),

    BmpSignatureInvalid,
    MoreThanOnePlane,
    InvalidChannelWidth(ChannelWidthError, u16),

    NegativeWidth(i32),
    ImageTooLarge(i32, i32),
    InvalidHeight,

    ImageTypeInvalidForTopDown(u32),
    ImageTypeUnknown(u32),

    HeaderTooSmall(u32),

    PaletteSizeExceeded { colors_used: u32, bit_count: u16 },
}

impl fmt::Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecoderError::CorruptRleData => f.write_str("Corrupt RLE data"),
            DecoderError::BitfieldMaskNonContiguous => f.write_str("Non-contiguous bitfield mask"),
            DecoderError::BitfieldMaskInvalid => f.write_str("Invalid bitfield mask"),
            DecoderError::BitfieldMaskMissing(bb) => {
                f.write_fmt(format_args!("Missing {bb}-bit bitfield mask"))
            }
            DecoderError::BitfieldMasksMissing(bb) => {
                f.write_fmt(format_args!("Missing {bb}-bit bitfield masks"))
            }
            DecoderError::BmpSignatureInvalid => f.write_str("BMP signature not found"),
            DecoderError::MoreThanOnePlane => f.write_str("More than one plane"),
            DecoderError::InvalidChannelWidth(tp, n) => {
                f.write_fmt(format_args!("Invalid channel bit count for {tp}: {n}"))
            }
            DecoderError::NegativeWidth(w) => f.write_fmt(format_args!("Negative width ({w})")),
            DecoderError::ImageTooLarge(w, h) => f.write_fmt(format_args!(
                "Image too large (one of ({w}, {h}) > soft limit of {MAX_WIDTH_HEIGHT})"
            )),
            DecoderError::InvalidHeight => f.write_str("Invalid height"),
            DecoderError::ImageTypeInvalidForTopDown(tp) => f.write_fmt(format_args!(
                "Invalid image type {tp} for top-down image."
            )),
            DecoderError::ImageTypeUnknown(tp) => {
                f.write_fmt(format_args!("Unknown image compression type {tp}"))
            }
            DecoderError::HeaderTooSmall(s) => {
                f.write_fmt(format_args!("Bitmap header too small ({s} bytes)"))
            }
            DecoderError::PaletteSizeExceeded {
                colors_used,
                bit_count,
            } => f.write_fmt(format_args!(
                "Palette size {colors_used} exceeds maximum size for BMP with bit count of {bit_count}"
            )),
        }
    }
}

impl From<DecoderError> for ImageError {
    fn from(e: DecoderError) -> ImageError {
        ImageError::Decoding(DecodingError::new(ImageFormat::Bmp.into(), e))
    }
}

impl error::Error for DecoderError {}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ChannelWidthError {
    Rgb,
    Rle8,
    Rle4,
    Bitfields,
}

impl fmt::Display for ChannelWidthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ChannelWidthError::Rgb => "RGB",
            ChannelWidthError::Rle8 => "RLE8",
            ChannelWidthError::Rle4 => "RLE4",
            ChannelWidthError::Bitfields => "bitfields",
        })
    }
}
