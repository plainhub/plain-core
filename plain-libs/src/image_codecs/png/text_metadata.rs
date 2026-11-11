use crate::image_codecs::deflate::BoundedDecompressionError;
use crate::image_codecs::png::{DecodingError, EncodingError, chunk, encoder};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use std::{convert::TryFrom, io::Write};
pub const DECOMPRESSION_LIMIT: usize = 2097152;
#[derive(Debug, Clone, Copy)]
pub(crate) enum TextEncodingError {
    Unrepresentable,
    InvalidKeywordSize,
    CompressionError,
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum TextDecodingError {
    Unrepresentable,
    InvalidKeywordSize,
    MissingNullSeparator,
    InflationError,
    OutOfDecompressionSpace,
    InvalidCompressionMethod,
    InvalidCompressionFlag,
    MissingCompressionFlag,
}
pub trait EncodableTextChunk {
    fn encode<W: Write>(&self, w: &mut W) -> Result<(), EncodingError>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TEXtChunk {
    pub keyword: String,
    pub text: String,
}
fn decode_iso_8859_1(text: &[u8]) -> String {
    text.iter().map(|&b| b as char).collect()
}
pub(crate) fn encode_iso_8859_1(text: &str) -> Result<Vec<u8>, TextEncodingError> {
    encode_iso_8859_1_iter(text).collect()
}
fn encode_iso_8859_1_into(buf: &mut Vec<u8>, text: &str) -> Result<(), TextEncodingError> {
    for b in encode_iso_8859_1_iter(text) {
        buf.push(b?);
    }
    Ok(())
}
fn encode_iso_8859_1_iter(text: &str) -> impl Iterator<Item = Result<u8, TextEncodingError>> + '_ {
    text.chars()
        .map(|c| u8::try_from(c as u32).map_err(|_| TextEncodingError::Unrepresentable))
}
fn decode_ascii(text: &[u8]) -> Result<&str, TextDecodingError> {
    if text.is_ascii() {
        Ok(std::str::from_utf8(text).expect("unreachable"))
    } else {
        Err(TextDecodingError::Unrepresentable)
    }
}
impl TEXtChunk {
    pub fn new(keyword: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            keyword: keyword.into(),
            text: text.into(),
        }
    }
    pub(crate) fn decode(
        keyword_slice: &[u8],
        text_slice: &[u8],
    ) -> Result<Self, TextDecodingError> {
        if keyword_slice.is_empty() || keyword_slice.len() > 79 {
            return Err(TextDecodingError::InvalidKeywordSize);
        }
        Ok(Self {
            keyword: decode_iso_8859_1(keyword_slice),
            text: decode_iso_8859_1(text_slice),
        })
    }
}
impl EncodableTextChunk for TEXtChunk {
    fn encode<W: Write>(&self, w: &mut W) -> Result<(), EncodingError> {
        let mut data = encode_iso_8859_1(&self.keyword)?;
        if data.is_empty() || data.len() > 79 {
            return Err(TextEncodingError::InvalidKeywordSize.into());
        }
        data.push(0);
        encode_iso_8859_1_into(&mut data, &self.text)?;
        encoder::write_chunk(w, chunk::tEXt, &data)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZTXtChunk {
    pub keyword: String,
    text: OptCompressed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum OptCompressed {
    Compressed(Vec<u8>),
    Uncompressed(String),
}
impl ZTXtChunk {
    pub fn new(keyword: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            keyword: keyword.into(),
            text: OptCompressed::Uncompressed(text.into()),
        }
    }
    pub(crate) fn decode(
        keyword_slice: &[u8],
        compression_method: u8,
        text_slice: &[u8],
    ) -> Result<Self, TextDecodingError> {
        if keyword_slice.is_empty() || keyword_slice.len() > 79 {
            return Err(TextDecodingError::InvalidKeywordSize);
        }
        if compression_method != 0 {
            return Err(TextDecodingError::InvalidCompressionMethod);
        }
        Ok(Self {
            keyword: decode_iso_8859_1(keyword_slice),
            text: OptCompressed::Compressed(text_slice.to_vec()),
        })
    }
    pub fn decompress_text(&mut self) -> Result<(), DecodingError> {
        self.decompress_text_with_limit(DECOMPRESSION_LIMIT)
    }
    pub fn decompress_text_with_limit(&mut self, limit: usize) -> Result<(), DecodingError> {
        match &self.text {
            OptCompressed::Compressed(v) => {
                let uncompressed_raw =
                    match crate::image_codecs::deflate::decompress_to_vec_bounded(&v[..], limit) {
                        Ok(s) => s,
                        Err(BoundedDecompressionError::OutputTooLarge { .. }) => {
                            return Err(DecodingError::from(
                                TextDecodingError::OutOfDecompressionSpace,
                            ));
                        }
                        Err(_) => {
                            return Err(DecodingError::from(TextDecodingError::InflationError));
                        }
                    };
                self.text = OptCompressed::Uncompressed(decode_iso_8859_1(&uncompressed_raw));
            }
            OptCompressed::Uncompressed(_) => {}
        };
        Ok(())
    }
    pub fn get_text(&self) -> Result<String, DecodingError> {
        match &self.text {
            OptCompressed::Compressed(v) => {
                let uncompressed_raw = crate::image_codecs::deflate::decompress_to_vec(v)
                    .map_err(|_| DecodingError::from(TextDecodingError::InflationError))?;
                Ok(decode_iso_8859_1(&uncompressed_raw))
            }
            OptCompressed::Uncompressed(s) => Ok(s.clone()),
        }
    }
    pub fn compress_text(&mut self) -> Result<(), EncodingError> {
        match &self.text {
            OptCompressed::Uncompressed(s) => {
                let uncompressed_raw = encode_iso_8859_1(s)?;
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
                encoder
                    .write_all(&uncompressed_raw)
                    .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                self.text = OptCompressed::Compressed(
                    encoder
                        .finish()
                        .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?,
                );
            }
            OptCompressed::Compressed(_) => {}
        }
        Ok(())
    }
}
impl EncodableTextChunk for ZTXtChunk {
    fn encode<W: Write>(&self, w: &mut W) -> Result<(), EncodingError> {
        let mut data = encode_iso_8859_1(&self.keyword)?;
        if data.is_empty() || data.len() > 79 {
            return Err(TextEncodingError::InvalidKeywordSize.into());
        }
        data.push(0);
        data.push(0);
        match &self.text {
            OptCompressed::Compressed(v) => {
                data.extend_from_slice(&v[..]);
            }
            OptCompressed::Uncompressed(s) => {
                let uncompressed_raw = encode_iso_8859_1(s)?;
                let mut encoder = ZlibEncoder::new(data, Compression::fast());
                encoder
                    .write_all(&uncompressed_raw)
                    .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                data = encoder
                    .finish()
                    .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
            }
        };
        encoder::write_chunk(w, chunk::zTXt, &data)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ITXtChunk {
    pub keyword: String,
    pub compressed: bool,
    pub language_tag: String,
    pub translated_keyword: String,
    text: OptCompressed,
}
impl ITXtChunk {
    pub fn new(keyword: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            keyword: keyword.into(),
            compressed: false,
            language_tag: "".to_string(),
            translated_keyword: "".to_string(),
            text: OptCompressed::Uncompressed(text.into()),
        }
    }
    pub(crate) fn decode(
        keyword_slice: &[u8],
        compression_flag: u8,
        compression_method: u8,
        language_tag_slice: &[u8],
        translated_keyword_slice: &[u8],
        text_slice: &[u8],
    ) -> Result<Self, TextDecodingError> {
        if keyword_slice.is_empty() || keyword_slice.len() > 79 {
            return Err(TextDecodingError::InvalidKeywordSize);
        }
        let keyword = decode_iso_8859_1(keyword_slice);
        let compressed = match compression_flag {
            0 => false,
            1 => true,
            _ => return Err(TextDecodingError::InvalidCompressionFlag),
        };
        if compressed && compression_method != 0 {
            return Err(TextDecodingError::InvalidCompressionMethod);
        }
        let language_tag = decode_ascii(language_tag_slice)?.to_owned();
        let translated_keyword = std::str::from_utf8(translated_keyword_slice)
            .map_err(|_| TextDecodingError::Unrepresentable)?
            .to_string();
        let text = if compressed {
            OptCompressed::Compressed(text_slice.to_vec())
        } else {
            OptCompressed::Uncompressed(
                String::from_utf8(text_slice.to_vec())
                    .map_err(|_| TextDecodingError::Unrepresentable)?,
            )
        };
        Ok(Self {
            keyword,
            compressed,
            language_tag,
            translated_keyword,
            text,
        })
    }
    pub fn decompress_text(&mut self) -> Result<(), DecodingError> {
        self.decompress_text_with_limit(DECOMPRESSION_LIMIT)
    }
    pub fn decompress_text_with_limit(&mut self, limit: usize) -> Result<(), DecodingError> {
        match &self.text {
            OptCompressed::Compressed(v) => {
                let uncompressed_raw =
                    match crate::image_codecs::deflate::decompress_to_vec_bounded(v, limit) {
                        Ok(s) => s,
                        Err(BoundedDecompressionError::OutputTooLarge { .. }) => {
                            return Err(DecodingError::from(
                                TextDecodingError::OutOfDecompressionSpace,
                            ));
                        }
                        Err(_) => {
                            return Err(DecodingError::from(TextDecodingError::InflationError));
                        }
                    };
                self.text = OptCompressed::Uncompressed(
                    String::from_utf8(uncompressed_raw)
                        .map_err(|_| TextDecodingError::Unrepresentable)?,
                );
            }
            OptCompressed::Uncompressed(_) => {}
        };
        Ok(())
    }
    pub fn get_text(&self) -> Result<String, DecodingError> {
        match &self.text {
            OptCompressed::Compressed(v) => {
                let uncompressed_raw = crate::image_codecs::deflate::decompress_to_vec(v)
                    .map_err(|_| DecodingError::from(TextDecodingError::InflationError))?;
                String::from_utf8(uncompressed_raw)
                    .map_err(|_| TextDecodingError::Unrepresentable.into())
            }
            OptCompressed::Uncompressed(s) => Ok(s.clone()),
        }
    }
    pub fn compress_text(&mut self) -> Result<(), EncodingError> {
        match &self.text {
            OptCompressed::Uncompressed(s) => {
                let uncompressed_raw = s.as_bytes();
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
                encoder
                    .write_all(uncompressed_raw)
                    .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                self.text = OptCompressed::Compressed(
                    encoder
                        .finish()
                        .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?,
                );
            }
            OptCompressed::Compressed(_) => {}
        }
        Ok(())
    }
}
impl EncodableTextChunk for ITXtChunk {
    fn encode<W: Write>(&self, w: &mut W) -> Result<(), EncodingError> {
        let mut data = encode_iso_8859_1(&self.keyword)?;
        if data.is_empty() || data.len() > 79 {
            return Err(TextEncodingError::InvalidKeywordSize.into());
        }
        data.push(0);
        if self.compressed {
            data.push(1);
        } else {
            data.push(0);
        }
        data.push(0);
        if !self.language_tag.is_ascii() {
            return Err(EncodingError::from(TextEncodingError::Unrepresentable));
        }
        data.extend(self.language_tag.as_bytes());
        data.push(0);
        data.extend_from_slice(self.translated_keyword.as_bytes());
        data.push(0);
        if self.compressed {
            match &self.text {
                OptCompressed::Compressed(v) => {
                    data.extend_from_slice(&v[..]);
                }
                OptCompressed::Uncompressed(s) => {
                    let uncompressed_raw = s.as_bytes();
                    let mut encoder = ZlibEncoder::new(data, Compression::fast());
                    encoder
                        .write_all(uncompressed_raw)
                        .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                    data = encoder
                        .finish()
                        .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                }
            }
        } else {
            match &self.text {
                OptCompressed::Compressed(v) => {
                    let uncompressed_raw = crate::image_codecs::deflate::decompress_to_vec(v)
                        .map_err(|_| EncodingError::from(TextEncodingError::CompressionError))?;
                    data.extend_from_slice(&uncompressed_raw[..]);
                }
                OptCompressed::Uncompressed(s) => {
                    data.extend_from_slice(s.as_bytes());
                }
            }
        }
        encoder::write_chunk(w, chunk::iTXt, &data)
    }
}
