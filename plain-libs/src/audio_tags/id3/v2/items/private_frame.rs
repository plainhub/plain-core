use crate::audio_tags::error::Result;
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::util::alloc::VecFallibleCapacity;
use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding, decode_text, encode_text};

use std::borrow::Cow;
use std::io::Read;

const FRAME_ID: FrameId<'static> = FrameId::Valid(Cow::Borrowed("PRIV"));

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PrivateFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub owner: String,

    pub private_data: Vec<u8>,
}

impl PrivateFrame<'_> {
    pub fn new(owner: String, private_data: Vec<u8>) -> Self {
        let header = FrameHeader::new(FRAME_ID, FrameFlags::default());
        Self {
            header,
            owner,
            private_data,
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

    pub fn parse<R>(reader: &mut R, frame_flags: FrameFlags) -> Result<Option<Self>>
    where
        R: Read,
    {
        let Ok(owner) = decode_text(
            reader,
            TextDecodeOptions::new()
                .encoding(TextEncoding::Latin1)
                .terminated(true),
        ) else {
            return Ok(None);
        };

        let owner = owner.content;

        let mut private_data = Vec::new();
        reader.read_to_end(&mut private_data)?;

        let header = FrameHeader::new(FRAME_ID, frame_flags);
        Ok(Some(PrivateFrame {
            header,
            owner,
            private_data,
        }))
    }

    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        let Self {
            owner,
            private_data,
            ..
        } = self;

        let mut content = Vec::try_with_capacity_stable(owner.len() + private_data.len())?;
        content.extend(encode_text(owner.as_str(), TextEncoding::Latin1, true));
        content.extend_from_slice(private_data);

        Ok(content)
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/audio_tags/id3/v2/items/private_frame/tests.rs"]
mod tests;
