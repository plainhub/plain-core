use crate::image_codecs::jpeg::misc::{
    START_OF_FRAME_EXT_AR, START_OF_FRAME_EXT_SEQ, START_OF_FRAME_LOS_SEQ,
    START_OF_FRAME_LOS_SEQ_AR, START_OF_FRAME_PROG_DCT_AR,
};
use crate::image_codecs::zune::bytestream::ZByteIoError;
use core::fmt::{Debug, Display, Formatter};
use std::string::String;
pub enum DecodeErrors {
    Format(String),
    FormatStatic(&'static str),
    IllegalMagicBytes(u16),
    HuffmanDecode(String),
    ZeroError,
    DqtError(String),
    SosError(String),
    SofError(String),
    Unsupported(UnsupportedSchemes),
    MCUError(String),
    ExhaustedData,
    LargeDimensions(usize),
    TooSmallOutput(usize, usize),
    IoErrors(ZByteIoError),
}
impl std::error::Error for DecodeErrors {}
impl From<&'static str> for DecodeErrors {
    fn from(data: &'static str) -> Self {
        return Self::FormatStatic(data);
    }
}
impl From<ZByteIoError> for DecodeErrors {
    fn from(data: ZByteIoError) -> Self {
        return Self::IoErrors(data);
    }
}
impl Debug for DecodeErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match &self {
            Self::Format(a) => write!(f, "{a:?}"),
            Self::FormatStatic(a) => write!(f, "{:?}", &a),
            Self::HuffmanDecode(reason) => {
                write!(f, "Error decoding huffman values: {reason}")
            }
            Self::ZeroError => write!(f, "Image width or height is set to zero, cannot continue"),
            Self::DqtError(reason) => write!(f, "Error parsing DQT segment. Reason:{reason}"),
            Self::SosError(reason) => write!(f, "Error parsing SOS Segment. Reason:{reason}"),
            Self::SofError(reason) => write!(f, "Error parsing SOF segment. Reason:{reason}"),
            Self::IllegalMagicBytes(bytes) => {
                write!(f, "Error parsing image. Illegal start bytes:{bytes:X}")
            }
            Self::MCUError(reason) => write!(f, "Error in decoding MCU. Reason {reason}"),
            Self::Unsupported(image_type) => {
                write!(f, "{image_type:?}")
            }
            Self::ExhaustedData => write!(f, "Exhausted data in the image"),
            Self::LargeDimensions(dimensions) => write!(
                f,
                "Too large dimensions {dimensions},library supports up to {}",
                crate::image_codecs::jpeg::decoder::MAX_DIMENSIONS
            ),
            Self::TooSmallOutput(expected, found) => write!(
                f,
                "Too small output, expected buffer with at least {expected} bytes but got one with {found} bytes"
            ),
            Self::IoErrors(error) => write!(f, "I/O errors {error:?}"),
        }
    }
}
impl Display for DecodeErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
#[derive(Eq, PartialEq, Copy, Clone)]
pub enum UnsupportedSchemes {
    ExtendedSequentialHuffman,
    LosslessHuffman,
    ExtendedSequentialDctArithmetic,
    ProgressiveDctArithmetic,
    LosslessArithmetic,
}
impl Debug for UnsupportedSchemes {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match &self {
            Self::ExtendedSequentialHuffman => {
                write!(
                    f,
                    "The library cannot yet decode images encoded using Extended Sequential Huffman  encoding scheme yet."
                )
            }
            Self::LosslessHuffman => {
                write!(
                    f,
                    "The library cannot yet decode images encoded with Lossless Huffman encoding scheme"
                )
            }
            Self::ExtendedSequentialDctArithmetic => {
                write!(
                    f,
                    "The library cannot yet decode Images Encoded with Extended Sequential DCT Arithmetic scheme"
                )
            }
            Self::ProgressiveDctArithmetic => {
                write!(
                    f,
                    "The library cannot yet decode images encoded with Progressive DCT Arithmetic scheme"
                )
            }
            Self::LosslessArithmetic => {
                write!(
                    f,
                    "The library cannot yet decode images encoded with Lossless Arithmetic encoding scheme"
                )
            }
        }
    }
}
impl UnsupportedSchemes {
    #[must_use]
    pub fn from_int(int: u8) -> Option<UnsupportedSchemes> {
        let int = u16::from_be_bytes([0xff, int]);
        match int {
            START_OF_FRAME_PROG_DCT_AR => Some(Self::ProgressiveDctArithmetic),
            START_OF_FRAME_LOS_SEQ => Some(Self::LosslessHuffman),
            START_OF_FRAME_LOS_SEQ_AR => Some(Self::LosslessArithmetic),
            START_OF_FRAME_EXT_SEQ => Some(Self::ExtendedSequentialHuffman),
            START_OF_FRAME_EXT_AR => Some(Self::ExtendedSequentialDctArithmetic),
            _ => None,
        }
    }
}
