use crate::audio_tags::error::Result;
use crate::audio_tags::id3::v2::frame::content::verify_encoding;
use crate::audio_tags::id3::v2::header::Id3v2Version;
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding, decode_text, encode_text};

use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::io::Read;

use crate::utils::binary::ReadBytesExt;

const FRAME_ID: FrameId<'static> = FrameId::Valid(Cow::Borrowed("WXXX"));

#[derive(Clone, Debug, Eq)]
pub struct ExtendedUrlFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub encoding: TextEncoding,

    pub description: String,

    pub content: String,
}

impl PartialEq for ExtendedUrlFrame<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.description == other.description
    }
}

impl Hash for ExtendedUrlFrame<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.description.hash(state);
    }
}

impl ExtendedUrlFrame<'_> {
    pub fn new(encoding: TextEncoding, description: String, content: String) -> Self {
        let header = FrameHeader::new(FRAME_ID, FrameFlags::default());
        Self {
            header,
            encoding,
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

    pub fn parse<R>(
        reader: &mut R,
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
        let description = decode_text(
            reader,
            TextDecodeOptions::new().encoding(encoding).terminated(true),
        )?
        .content;
        let content = decode_text(
            reader,
            TextDecodeOptions::new().encoding(TextEncoding::Latin1),
        )?
        .content;

        let header = FrameHeader::new(FRAME_ID, frame_flags);
        Ok(Some(ExtendedUrlFrame {
            header,
            encoding,
            description,
            content,
        }))
    }

    pub fn as_bytes(&self, is_id3v23: bool) -> Vec<u8> {
        let mut encoding = self.encoding;
        if is_id3v23 {
            encoding = encoding.to_id3v23();
        }

        let mut bytes = vec![encoding as u8];

        bytes.extend(encode_text(&self.description, encoding, true).iter());
        bytes.extend(encode_text(&self.content, encoding, false));

        bytes
    }
}
