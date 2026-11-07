use crate::audio_tags::error::{ErrorKind, Id3v2Error, Id3v2ErrorKind, LoftyError, Result};
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::macros::err;
use crate::audio_tags::util::text::{
    TextDecodeOptions, TextEncoding, decode_text, encode_text, read_to_terminator,
    utf16_decode_bytes,
};

use std::borrow::Cow;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use crate::utils::binary::{BigEndian, ReadBytesExt, WriteBytesExt};

const FRAME_ID: FrameId<'static> = FrameId::Valid(Cow::Borrowed("SYLT"));

#[derive(Copy, Clone, PartialEq, Debug, Eq, Hash)]
#[repr(u8)]
pub enum TimestampFormat {
    MPEG = 1,

    MS = 2,
}

impl TimestampFormat {
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(Self::MPEG),
            2 => Some(Self::MS),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Eq, Hash)]
#[repr(u8)]

pub enum SyncTextContentType {
    Other = 0,
    Lyrics = 1,
    TextTranscription = 2,
    PartName = 3,
    Events = 4,
    Chord = 5,
    Trivia = 6,
    WebpageURL = 7,
    ImageURL = 8,
}

impl SyncTextContentType {
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Other),
            1 => Some(Self::Lyrics),
            2 => Some(Self::TextTranscription),
            3 => Some(Self::PartName),
            4 => Some(Self::Events),
            5 => Some(Self::Chord),
            6 => Some(Self::Trivia),
            7 => Some(Self::WebpageURL),
            8 => Some(Self::ImageURL),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SynchronizedTextFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub encoding: TextEncoding,

    pub language: [u8; 3],

    pub timestamp_format: TimestampFormat,

    pub content_type: SyncTextContentType,

    pub description: Option<String>,

    pub content: Vec<(u32, String)>,
}

impl SynchronizedTextFrame<'_> {
    pub fn new(
        encoding: TextEncoding,
        language: [u8; 3],
        timestamp_format: TimestampFormat,
        content_type: SyncTextContentType,
        description: Option<String>,
        content: Vec<(u32, String)>,
    ) -> Self {
        let header = FrameHeader::new(FRAME_ID, FrameFlags::default());
        Self {
            header,
            encoding,
            language,
            timestamp_format,
            content_type,
            description,
            content,
        }
    }

    pub fn id(&self) -> FrameId<'_> {
        FRAME_ID
    }

    pub fn flags(&self) -> FrameFlags {
        self.header.flags
    }

    pub fn set_flags(&mut self, flags: FrameFlags) {
        self.header.flags = flags;
    }

    // Infallible
    pub fn parse(data: &[u8], frame_flags: FrameFlags) -> Result<Self> {
        if data.len() < 7 {
            return Err(Id3v2Error::new(Id3v2ErrorKind::BadFrameLength).into());
        }

        let encoding = TextEncoding::from_u8(data[0])
            .ok_or_else(|| LoftyError::new(ErrorKind::TextDecode("Found invalid encoding")))?;
        let language: [u8; 3] = data[1..4].try_into().unwrap();
        if language.iter().any(|c| !c.is_ascii_alphabetic()) {
            return Err(Id3v2Error::new(Id3v2ErrorKind::BadSyncText).into());
        }
        let timestamp_format = TimestampFormat::from_u8(data[4])
            .ok_or_else(|| Id3v2Error::new(Id3v2ErrorKind::BadTimestampFormat))?;
        let content_type = SyncTextContentType::from_u8(data[5])
            .ok_or_else(|| Id3v2Error::new(Id3v2ErrorKind::BadSyncText))?;

        let mut cursor = Cursor::new(&data[6..]);
        let description = crate::audio_tags::util::text::decode_text(
            &mut cursor,
            TextDecodeOptions::new().encoding(encoding).terminated(true),
        )
        .map_err(|_| Id3v2Error::new(Id3v2ErrorKind::BadSyncText))?
        .text_or_none();

        let mut endianness: fn([u8; 2]) -> u16 = u16::from_le_bytes;

        // It's possible for the description to be the only string with a BOM
        // To be safe, we change the encoding to the concrete variant determined from the description
        if encoding == TextEncoding::UTF16 {
            endianness = match cursor.get_ref()[..=1] {
                [0xFF, 0xFE] => u16::from_le_bytes,
                [0xFE, 0xFF] => u16::from_be_bytes,
                // Since the description was already read, we can assume the BOM was valid
                _ => unreachable!(),
            };
        }

        let mut pos = 0;
        let total = (data.len() - 6) as u64 - cursor.stream_position()?;

        let mut content = Vec::new();

        while pos < total {
            let text = (|| -> Result<String> {
                if encoding == TextEncoding::UTF16 {
                    // Check for a BOM
                    let mut bom = [0; 2];
                    cursor
                        .read_exact(&mut bom)
                        .map_err(|_| Id3v2Error::new(Id3v2ErrorKind::BadSyncText))?;

                    cursor.seek(SeekFrom::Current(-2))?;

                    // Encountered text that doesn't include a BOM
                    if bom != [0xFF, 0xFE] && bom != [0xFE, 0xFF] {
                        let (raw_text, _) = read_to_terminator(&mut cursor, TextEncoding::UTF16);
                        return utf16_decode_bytes(&raw_text, endianness)
                            .map_err(|_| Id3v2Error::new(Id3v2ErrorKind::BadSyncText).into());
                    }
                }

                let decoded_text = decode_text(
                    &mut cursor,
                    TextDecodeOptions::new().encoding(encoding).terminated(true),
                )
                .map_err(|_| Id3v2Error::new(Id3v2ErrorKind::BadSyncText))?;
                pos += decoded_text.bytes_read as u64;

                Ok(decoded_text.content)
            })()?;

            let time = cursor
                .read_u32::<BigEndian>()
                .map_err(|_| Id3v2Error::new(Id3v2ErrorKind::BadSyncText))?;
            pos += 4;

            content.push((time, text));
        }

        let header = FrameHeader::new(FRAME_ID, frame_flags);
        Ok(Self {
            header,
            encoding,
            language,
            timestamp_format,
            content_type,
            description,
            content,
        })
    }

    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        let mut data = vec![self.encoding as u8];

        if self.language.len() == 3 && self.language.iter().all(u8::is_ascii_alphabetic) {
            data.write_all(&self.language)?;
            data.write_u8(self.timestamp_format as u8)?;
            data.write_u8(self.content_type as u8)?;

            if let Some(description) = &self.description {
                data.write_all(&encode_text(description, self.encoding, true))?;
            } else {
                data.write_u8(0)?;
            }

            for (time, text) in &self.content {
                data.write_all(&encode_text(text, self.encoding, true))?;
                data.write_u32::<BigEndian>(*time)?;
            }

            if data.len() as u64 > u64::from(u32::MAX) {
                err!(TooMuchData);
            }

            return Ok(data);
        }

        Err(Id3v2Error::new(Id3v2ErrorKind::BadSyncText).into())
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/audio_tags/id3/v2/items/sync_text/tests.rs"]
mod tests;
