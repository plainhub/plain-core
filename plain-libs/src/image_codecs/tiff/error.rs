use std::io;
use std::str;
use std::string;

use crate::image_codecs::tiff::ColorType;
use crate::image_codecs::tiff::decoder::ChunkType;
use crate::image_codecs::tiff::tags::Type;
use crate::image_codecs::tiff::tags::{
    CompressionMethod, PhotometricInterpretation, PlanarConfiguration, SampleFormat, Tag,
};
#[derive(Debug)]
pub enum TiffError {
    FormatError(TiffFormatError),
    UnsupportedError(TiffUnsupportedError),
    IoError(io::Error),
    LimitsExceeded,
    IntSizeError,
    UsageError(UsageError),
}
impl std::fmt::Display for TiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FormatError(err) => write!(f, "format error: {err}"),
            Self::UnsupportedError(err) => write!(f, "unsupported error: {err}"),
            Self::IoError(err) => write!(f, "{err}"),
            Self::LimitsExceeded => write!(f, "decoder limits exceeded"),
            Self::IntSizeError => write!(f, "platform or format size limits exceeded"),
            Self::UsageError(err) => write!(f, "usage error: {err}"),
        }
    }
}
impl std::error::Error for TiffError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FormatError(err) => Some(err),
            Self::UnsupportedError(err) => Some(err),
            Self::IoError(err) => Some(err),
            Self::UsageError(err) => Some(err),
            _ => None,
        }
    }
}
impl From<TiffFormatError> for TiffError {
    fn from(value: TiffFormatError) -> Self {
        Self::FormatError(value)
    }
}
impl From<TiffUnsupportedError> for TiffError {
    fn from(value: TiffUnsupportedError) -> Self {
        Self::UnsupportedError(value)
    }
}
impl From<io::Error> for TiffError {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}
impl From<UsageError> for TiffError {
    fn from(value: UsageError) -> Self {
        Self::UsageError(value)
    }
}
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TiffFormatError {
    TiffSignatureNotFound,
    TiffSignatureInvalid,
    ImageFileDirectoryNotFound,
    InconsistentSizesEncountered,
    InvalidDimensions(u32, u32),
    InvalidTag,
    InvalidTagValueType(Tag),
    RequiredTagNotFound(Tag),
    UnknownPredictor(u16),
    UnknownPlanarConfiguration(u16),
    InvalidTypeForTag,
    InvalidCountForTag(Tag, usize),
    StripTileTagConflict,
    CycleInOffsets,
    SamplesPerPixelIsZero,
    CompressedDataCorrupt(String),
}
impl std::fmt::Display for TiffFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TiffSignatureNotFound => write!(f, "TIFF signature not found"),
            Self::TiffSignatureInvalid => write!(f, "TIFF signature invalid"),
            Self::ImageFileDirectoryNotFound => write!(f, "image file directory not found"),
            Self::InconsistentSizesEncountered => write!(f, "inconsistent sizes encountered"),
            Self::InvalidDimensions(width, height) => {
                write!(f, "invalid dimensions: {width}x{height}")
            }
            Self::InvalidTag => write!(f, "image contains invalid tag"),
            Self::InvalidTagValueType(tag) => {
                write!(f, "tag `{tag:?}` did not have the expected value type")
            }
            Self::RequiredTagNotFound(tag) => write!(f, "required tag `{tag:?}` not found"),
            Self::UnknownPredictor(pred) => write!(f, "unknown predictor “{pred}” encountered"),
            Self::UnknownPlanarConfiguration(cfg) => {
                write!(f, "unknown planar configuration “{cfg}”")
            }
            Self::InvalidTypeForTag => write!(f, "tag has invalid type"),
            Self::InvalidCountForTag(tag, len) => write!(
                f,
                "tag `{tag:?}` with incorrect number of elements ({len}) encountered"
            ),
            Self::StripTileTagConflict => write!(
                f,
                "file should contain either (StripByteCounts and StripOffsets) or (TileByteCounts and TileOffsets), other combination was found"
            ),
            Self::CycleInOffsets => write!(f, "file contained a cycle in the list of IFDs"),
            Self::SamplesPerPixelIsZero => write!(f, "samples per pixel is zero"),
            Self::CompressedDataCorrupt(message) => {
                write!(f, "compressed data is corrupt: {message}")
            }
        }
    }
}
impl std::error::Error for TiffFormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            _ => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TiffUnsupportedError {
    FloatingPointPredictor(ColorType),
    HorizontalPredictor(ColorType),
    InconsistentBitsPerSample(Vec<u8>),
    InterpretationWithBits(PhotometricInterpretation, Vec<u8>),
    UnknownInterpretation,
    UnknownCompressionMethod,
    UnsupportedCompressionMethod(CompressionMethod),
    UnsupportedSampleDepth(u8),
    UnsupportedSampleFormat(Vec<SampleFormat>),
    UnsupportedColorType(ColorType),
    UnsupportedBitsPerChannel(u8),
    UnsupportedPlanarConfig(Option<PlanarConfiguration>),
    UnsupportedDataType,
    UnsupportedInterpretation(PhotometricInterpretation),
    ChromaSubsampling,
    MisalignedTileBoundaries,
}
impl std::fmt::Display for TiffUnsupportedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FloatingPointPredictor(color_type) => write!(
                f,
                "floating point predictor for {color_type:?} is unsupported"
            ),
            Self::HorizontalPredictor(color_type) => {
                write!(f, "horizontal predictor for {color_type:?} is unsupported")
            }
            Self::InconsistentBitsPerSample(bits_per_sample) => {
                write!(f, "inconsistent bits per sample: {bits_per_sample:?}")
            }
            Self::InterpretationWithBits(interpretation, bits_per_sample) => write!(
                f,
                "Photometric interpretation {interpretation:?} with bits per sample {bits_per_sample:?} is unsupported"
            ),
            Self::UnknownInterpretation => write!(f, "unknown photometric interpretation"),
            Self::UnknownCompressionMethod => write!(f, "unknown compression method"),
            Self::UnsupportedCompressionMethod(method) => {
                write!(f, "compression method {method:?} is unsupported")
            }
            Self::UnsupportedSampleDepth(depth) => {
                write!(f, "{depth} samples per pixel is unsupported")
            }
            Self::UnsupportedSampleFormat(sample_format) => {
                write!(f, "sample format {sample_format:?} is unsupported")
            }
            Self::UnsupportedColorType(color_type) => {
                write!(f, "color type {color_type:?} is unsupported")
            }
            Self::UnsupportedBitsPerChannel(bits_per_channel) => {
                write!(f, "{bits_per_channel} bits per channel not supported")
            }
            Self::UnsupportedPlanarConfig(planar) => {
                write!(f, "unsupported planar configuration “{planar:?}”")
            }
            Self::UnsupportedDataType => write!(f, "unsupported data type."),
            Self::UnsupportedInterpretation(interpretation) => write!(
                f,
                "unsupported photometric interpretation \"{interpretation:?}\""
            ),
            Self::ChromaSubsampling => {
                write!(f, "chroma subsampling of YCbCr color is unsupported")
            }
            Self::MisalignedTileBoundaries => {
                write!(f, "tile rows are not aligned to byte boundaries")
            }
        }
    }
}
impl std::error::Error for TiffUnsupportedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            _ => None,
        }
    }
}
#[derive(Debug)]
#[non_exhaustive]
pub enum UsageError {
    InvalidChunkType(ChunkType, ChunkType),
    InvalidChunkIndex(u32),
    InvalidPlaneIndex(u16),
    InvalidCodingUnit(u32, u32),
    PredictorCompressionMismatch,
    PredictorIncompatible,
    PredictorUnavailable,
    InsufficientOutputBufferSize { needed: usize, provided: usize },
    InsufficientOutputRowStride { needed: usize, requested: usize },
    ZeroIfdPointer,
    ReconfiguredAfterImageWrite,
    ByteOrderMismatch,
    MismatchedEntryLength { ty: Type, found: usize },
}
impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidChunkType(expected, actual) => write!(
                f,
                "requested operation is only valid for images with chunk encoding of type {expected:?} but got {actual:?}"
            ),
            Self::InvalidChunkIndex(index) => write!(f, "invalid chunk index ({index}) requested"),
            Self::InvalidPlaneIndex(index) => write!(f, "invalid plane index ({index}) requested"),
            Self::InvalidCodingUnit(index, have) => write!(
                f,
                "out of bounds coding unit ({index}) requested, only {have} available"
            ),
            Self::PredictorCompressionMismatch => write!(
                f,
                "requested predictor is not compatible with the requested compression"
            ),
            Self::PredictorIncompatible => write!(
                f,
                "the requested predictor is not compatible with the image's format"
            ),
            Self::PredictorUnavailable => write!(f, "the requested predictor is not available"),
            Self::InsufficientOutputBufferSize { needed, provided } => write!(
                f,
                "the borrowed output buffer is not large enough for the decoded data, needed {needed} but have {provided}"
            ),
            Self::InsufficientOutputRowStride { needed, requested } => write!(
                f,
                "the provided output row stride would alias rows of decoded data, needed {needed} but have {requested}"
            ),
            Self::ZeroIfdPointer => write!(f, "the offset 0 can not point to a valid IFD"),
            Self::ReconfiguredAfterImageWrite => write!(
                f,
                "attempted to reconfigure the encoder after image writing has started"
            ),
            Self::ByteOrderMismatch => write!(
                f,
                "attempted to use data with a byte order that does not match the required one"
            ),
            Self::MismatchedEntryLength { ty, found } => write!(
                f,
                "the length of data is not a multiple of the size of the type {ty:?}, found {found}"
            ),
        }
    }
}
impl std::error::Error for UsageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            _ => None,
        }
    }
}

impl From<str::Utf8Error> for TiffError {
    fn from(_err: str::Utf8Error) -> TiffError {
        TiffError::FormatError(TiffFormatError::InvalidTag)
    }
}

impl From<string::FromUtf8Error> for TiffError {
    fn from(_err: string::FromUtf8Error) -> TiffError {
        TiffError::FormatError(TiffFormatError::InvalidTag)
    }
}

impl From<std::num::TryFromIntError> for TiffError {
    fn from(_err: std::num::TryFromIntError) -> TiffError {
        TiffError::IntSizeError
    }
}

impl From<crate::image_codecs::lzw::LzwError> for TiffError {
    fn from(err: crate::image_codecs::lzw::LzwError) -> TiffError {
        TiffError::FormatError(TiffFormatError::CompressedDataCorrupt(err.to_string()))
    }
}

impl From<crate::image_codecs::jpeg::errors::DecodeErrors> for TiffError {
    fn from(err: crate::image_codecs::jpeg::errors::DecodeErrors) -> Self {
        TiffError::FormatError(TiffFormatError::CompressedDataCorrupt(err.to_string()))
    }
}
pub type TiffResult<T> = Result<T, TiffError>;
