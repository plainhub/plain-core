use crate::audio_tags::error::{ErrorKind, LoftyError, Result};
use crate::audio_tags::macros::err;

use std::io::Read;

use crate::utils::binary::ReadBytesExt;

#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash)]
#[repr(u8)]
pub enum TextEncoding {
    Latin1 = 0,

    UTF16 = 1,

    UTF16BE = 2,

    UTF8 = 3,
}

impl TextEncoding {
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Latin1),
            1 => Some(Self::UTF16),
            2 => Some(Self::UTF16BE),
            3 => Some(Self::UTF8),
            _ => None,
        }
    }

    pub(crate) fn verify_latin1(text: &str) -> bool {
        text.chars().all(|c| c as u32 <= 255)
    }

    pub(crate) fn to_id3v23(self) -> Self {
        match self {
            Self::UTF8 | Self::UTF16BE => {
                log::warn!(
                    "Text encoding {:?} is not supported in ID3v2.3, substituting with UTF-16",
                    self
                );
                Self::UTF16
            }
            _ => self,
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub(crate) struct DecodeTextResult {
    pub(crate) content: String,
    pub(crate) bytes_read: usize,
    pub(crate) bom: [u8; 2],
}

impl DecodeTextResult {
    pub(crate) fn text_or_none(self) -> Option<String> {
        if self.content.is_empty() {
            return None;
        }

        Some(self.content)
    }
}

const EMPTY_DECODED_TEXT: DecodeTextResult = DecodeTextResult {
    content: String::new(),
    bytes_read: 0,
    bom: [0, 0],
};

#[derive(Copy, Clone, Debug)]
pub(crate) struct TextDecodeOptions {
    pub encoding: TextEncoding,
    pub terminated: bool,
    pub bom: [u8; 2],
}

impl TextDecodeOptions {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn encoding(mut self, encoding: TextEncoding) -> Self {
        self.encoding = encoding;
        self
    }

    pub(crate) fn terminated(mut self, terminated: bool) -> Self {
        self.terminated = terminated;
        self
    }

    pub(crate) fn bom(mut self, bom: [u8; 2]) -> Self {
        self.bom = bom;
        self
    }
}

impl Default for TextDecodeOptions {
    fn default() -> Self {
        Self {
            encoding: TextEncoding::UTF8,
            terminated: false,
            bom: [0, 0],
        }
    }
}

pub(crate) fn decode_text<R>(reader: &mut R, options: TextDecodeOptions) -> Result<DecodeTextResult>
where
    R: Read,
{
    let raw_bytes;
    let bytes_read;

    if options.terminated {
        let (bytes, terminator_len) = read_to_terminator(reader, options.encoding);

        if bytes.is_empty() {
            return Ok(EMPTY_DECODED_TEXT);
        }

        bytes_read = bytes.len() + terminator_len;
        raw_bytes = bytes;
    } else {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;

        if bytes.is_empty() {
            return Ok(EMPTY_DECODED_TEXT);
        }

        bytes_read = bytes.len();
        raw_bytes = bytes;
    }

    let mut bom = [0, 0];
    let read_string = match options.encoding {
        TextEncoding::Latin1 => latin1_decode(&raw_bytes),
        TextEncoding::UTF16 => {
            if raw_bytes.len() < 2 {
                err!(TextDecode("UTF-16 string has an invalid length (< 2)"));
            }

            if raw_bytes.len() % 2 != 0 {
                err!(TextDecode("UTF-16 string has an odd length"));
            }

            let bom_to_check;
            if options.bom == [0, 0] {
                bom_to_check = [raw_bytes[0], raw_bytes[1]];
            } else {
                bom_to_check = options.bom;
            }

            match bom_to_check {
                [0xFE, 0xFF] => {
                    bom = [0xFE, 0xFF];
                    utf16_decode_bytes(&raw_bytes[2..], u16::from_be_bytes)?
                }
                [0xFF, 0xFE] => {
                    bom = [0xFF, 0xFE];
                    utf16_decode_bytes(&raw_bytes[2..], u16::from_le_bytes)?
                }
                _ => err!(TextDecode("UTF-16 string has an invalid byte order mark")),
            }
        }
        TextEncoding::UTF16BE => utf16_decode_bytes(raw_bytes.as_slice(), u16::from_be_bytes)?,
        TextEncoding::UTF8 => utf8_decode(raw_bytes)
            .map_err(|_| LoftyError::new(ErrorKind::TextDecode("Expected a UTF-8 string")))?,
    };

    if read_string.is_empty() {
        return Ok(EMPTY_DECODED_TEXT);
    }

    Ok(DecodeTextResult {
        content: read_string,
        bytes_read,
        bom,
    })
}

pub(crate) fn read_to_terminator<R>(reader: &mut R, encoding: TextEncoding) -> (Vec<u8>, usize)
where
    R: Read,
{
    let mut text_bytes = Vec::new();
    let mut terminator_len = 0;

    match encoding {
        TextEncoding::Latin1 | TextEncoding::UTF8 => {
            while let Ok(byte) = reader.read_u8() {
                if byte == 0 {
                    terminator_len = 1;
                    break;
                }

                text_bytes.push(byte)
            }
        }
        TextEncoding::UTF16 | TextEncoding::UTF16BE => {
            while let (Ok(b1), Ok(b2)) = (reader.read_u8(), reader.read_u8()) {
                if b1 == 0 && b2 == 0 {
                    terminator_len = 2;
                    break;
                }

                text_bytes.push(b1);
                text_bytes.push(b2)
            }
        }
    }

    (text_bytes, terminator_len)
}

pub(crate) fn latin1_decode(bytes: &[u8]) -> String {
    let mut text = bytes.iter().map(|c| *c as char).collect::<String>();
    trim_end_nulls(&mut text);
    text
}

pub(crate) fn utf8_decode(bytes: Vec<u8>) -> Result<String> {
    String::from_utf8(bytes)
        .map(|mut text| {
            trim_end_nulls(&mut text);
            text
        })
        .map_err(Into::into)
}

pub(crate) fn utf8_decode_str(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes)
        .map(trim_end_nulls_str)
        .map_err(Into::into)
}

pub(crate) fn utf16_decode(words: &[u16]) -> Result<String> {
    String::from_utf16(words)
        .map(|mut text| {
            trim_end_nulls(&mut text);
            text
        })
        .map_err(|_| LoftyError::new(ErrorKind::TextDecode("Given an invalid UTF-16 string")))
}

pub(crate) fn utf16_decode_bytes(bytes: &[u8], endianness: fn([u8; 2]) -> u16) -> Result<String> {
    if bytes.is_empty() {
        return Ok(String::new());
    }

    let unverified: Vec<u16> = bytes
        .chunks_exact(2)
        // In ID3v2, it is possible to have multiple UTF-16 strings separated by null.
        // This also makes it possible for us to encounter multiple BOMs in a single string.
        // We must filter them out.
        .filter_map(|c| match c {
            [0xFF, 0xFE] | [0xFE, 0xFF] => None,
            _ => Some(endianness(c.try_into().unwrap())), // Infallible
        })
        .collect();

    utf16_decode(&unverified)
}

pub(crate) fn encode_text(text: &str, text_encoding: TextEncoding, terminated: bool) -> Vec<u8> {
    match text_encoding {
        TextEncoding::Latin1 => {
            let mut out = text.chars().map(|c| c as u8).collect::<Vec<u8>>();

            if terminated {
                out.push(0)
            }

            out
        }
        TextEncoding::UTF16 => utf16_encode(text, u16::to_ne_bytes, true, terminated),
        TextEncoding::UTF16BE => utf16_encode(text, u16::to_be_bytes, false, terminated),
        TextEncoding::UTF8 => {
            let mut out = text.as_bytes().to_vec();

            if terminated {
                out.push(0);
            }

            out
        }
    }
}

pub(crate) fn trim_end_nulls(text: &mut String) {
    if text.ends_with('\0') {
        let new_len = text.trim_end_matches('\0').len();
        text.truncate(new_len);
    }
}

pub(crate) fn trim_end_nulls_str(text: &str) -> &str {
    text.trim_end_matches('\0')
}

fn utf16_encode(
    text: &str,
    endianness: fn(u16) -> [u8; 2],
    bom: bool,
    terminated: bool,
) -> Vec<u8> {
    let mut encoded = Vec::<u8>::new();

    if bom {
        encoded.extend_from_slice(&endianness(0xFEFF_u16));
    }

    for ch in text.encode_utf16() {
        encoded.extend_from_slice(&endianness(ch));
    }

    if terminated {
        encoded.extend_from_slice(&[0, 0]);
    }

    encoded
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/audio_tags/util/text/tests.rs"]
mod tests;
