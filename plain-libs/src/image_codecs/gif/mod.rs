#![forbid(unsafe_code)]
#[macro_use]
mod common;
mod encoder;
mod reader;
mod traits;
pub use crate::image_codecs::gif::common::{AnyExtension, DisposalMethod, Extension, Frame};
pub use crate::image_codecs::gif::encoder::{
    Encoder, EncodingError, EncodingFormatError, ExtensionData, Repeat,
};
pub use crate::image_codecs::gif::reader::{ColorOutput, MemoryLimit};
pub use crate::image_codecs::gif::reader::{DecodeOptions, Decoder, Version};
pub use crate::image_codecs::gif::reader::{DecodingError, DecodingFormatError};
pub mod streaming_decoder {
    pub use crate::image_codecs::gif::common::Block;
    pub use crate::image_codecs::gif::reader::{
        Decoded, FrameDataType, FrameDecoder, OutputBuffer, StreamingDecoder,
    };
}
