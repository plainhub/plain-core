use crate::audio_tags::config::ParsingMode;
use crate::audio_tags::error::Result;
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::macros::err;
use crate::audio_tags::tag::items::Timestamp;
use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding, decode_text, encode_text};

use std::io::Read;

use crate::utils::binary::ReadBytesExt;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]

pub struct TimestampFrame<'a> {
    pub(crate) header: FrameHeader<'a>,
    pub encoding: TextEncoding,
    pub timestamp: Timestamp,
}

impl PartialOrd for TimestampFrame<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimestampFrame<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

impl<'a> TimestampFrame<'a> {
    pub fn new(id: FrameId<'a>, encoding: TextEncoding, timestamp: Timestamp) -> Self {
        let header = FrameHeader::new(id, FrameFlags::default());
        Self {
            header,
            encoding,
            timestamp,
        }
    }

    pub fn id(&self) -> &FrameId<'_> {
        &self.header.id
    }

    pub fn flags(&self) -> FrameFlags {
        self.header.flags
    }

    pub fn set_flags(&mut self, flags: FrameFlags) {
        self.header.flags = flags;
    }

    pub fn parse<R>(
        reader: &mut R,
        id: FrameId<'a>,
        frame_flags: FrameFlags,
        parse_mode: ParsingMode,
    ) -> Result<Option<Self>>
    where
        R: Read,
    {
        let Ok(encoding_byte) = reader.read_u8() else {
            return Ok(None);
        };
        let Some(encoding) = TextEncoding::from_u8(encoding_byte) else {
            if parse_mode != ParsingMode::Relaxed {
                err!(TextDecode("Found invalid encoding"))
            }
            return Ok(None);
        };

        let value = decode_text(reader, TextDecodeOptions::new().encoding(encoding))?.content;
        if !value.is_ascii() {
            if parse_mode == ParsingMode::Strict {
                err!(BadTimestamp("Timestamp contains non-ASCII characters"))
            }
            return Ok(None);
        }

        let header = FrameHeader::new(id, frame_flags);
        let mut frame = TimestampFrame {
            header,
            encoding,
            timestamp: Timestamp::default(),
        };

        let reader = &mut value.as_bytes();

        let result;
        match Timestamp::parse(reader, parse_mode) {
            Ok(timestamp) => result = timestamp,
            Err(e) => {
                if parse_mode != ParsingMode::Relaxed {
                    return Err(e);
                }
                return Ok(None);
            }
        }

        let Some(timestamp) = result else {
            // Timestamp is empty
            return Ok(None);
        };

        frame.timestamp = timestamp;
        Ok(Some(frame))
    }

    pub fn as_bytes(&self, is_id3v23: bool) -> Result<Vec<u8>> {
        let mut encoding = self.encoding;
        if is_id3v23 {
            encoding = encoding.to_id3v23();
        }

        self.timestamp.verify()?;

        let mut encoded_text = encode_text(&self.timestamp.to_string(), encoding, false);
        encoded_text.insert(0, encoding as u8);

        Ok(encoded_text)
    }
}
