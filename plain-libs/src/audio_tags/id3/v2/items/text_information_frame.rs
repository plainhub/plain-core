use crate::audio_tags::error::Result;
use crate::audio_tags::id3::v2::frame::content::verify_encoding;
use crate::audio_tags::id3::v2::header::Id3v2Version;
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding, decode_text, encode_text};

use crate::utils::binary::ReadBytesExt;

use std::hash::Hash;
use std::io::Read;

#[derive(Clone, Debug, Eq)]
pub struct TextInformationFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub encoding: TextEncoding,

    pub value: String,
}

impl PartialEq for TextInformationFrame<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.header.id == other.header.id
    }
}

impl Hash for TextInformationFrame<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.header.id.hash(state);
    }
}

impl<'a> TextInformationFrame<'a> {
    pub fn new(id: FrameId<'a>, encoding: TextEncoding, value: String) -> Self {
        let header = FrameHeader::new(id, FrameFlags::default());
        Self {
            header,
            encoding,
            value,
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
        version: Id3v2Version,
    ) -> Result<Option<Self>>
    where
        R: Read,
    {
        let Ok(encoding_byte) = reader.read_u8() else {
            return Ok(None);
        };

        let encoding = verify_encoding(encoding_byte, version)?;
        let value = decode_text(reader, TextDecodeOptions::new().encoding(encoding))?.content;

        let header = FrameHeader::new(id, frame_flags);
        Ok(Some(TextInformationFrame {
            header,
            encoding,
            value,
        }))
    }

    pub fn as_bytes(&self, is_id3v23: bool) -> Vec<u8> {
        let mut encoding = self.encoding;
        if is_id3v23 {
            encoding = encoding.to_id3v23();
        }

        let mut content = encode_text(&self.value, encoding, false);

        content.insert(0, encoding as u8);
        content
    }
}
