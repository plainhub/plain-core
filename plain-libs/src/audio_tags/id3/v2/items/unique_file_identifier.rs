use crate::audio_tags::config::ParsingMode;
use crate::audio_tags::error::{Id3v2Error, Id3v2ErrorKind, Result};
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::macros::parse_mode_choice;
use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding, decode_text, encode_text};

use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::io::Read;

const FRAME_ID: FrameId<'static> = FrameId::Valid(Cow::Borrowed("UFID"));

#[derive(Clone, Debug, Eq)]
pub struct UniqueFileIdentifierFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub owner: String,

    pub identifier: Vec<u8>,
}

impl PartialEq for UniqueFileIdentifierFrame<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner
    }
}

impl Hash for UniqueFileIdentifierFrame<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.owner.hash(state);
    }
}

impl UniqueFileIdentifierFrame<'_> {
    pub fn new(owner: String, identifier: Vec<u8>) -> Self {
        let header = FrameHeader::new(FRAME_ID, FrameFlags::default());
        Self {
            header,
            owner,
            identifier,
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
        parse_mode: ParsingMode,
    ) -> Result<Option<Self>>
    where
        R: Read,
    {
        let owner_decode_result = decode_text(
            reader,
            TextDecodeOptions::new()
                .encoding(TextEncoding::Latin1)
                .terminated(true),
        )?;

        let owner;
        match owner_decode_result.text_or_none() {
            Some(valid) => owner = valid,
            None => {
                parse_mode_choice!(
                    parse_mode,
                    BESTATTEMPT: owner = String::new(),
                    DEFAULT: return Err(Id3v2Error::new(Id3v2ErrorKind::MissingUfidOwner).into())
                );
            }
        }

        let mut identifier = Vec::new();
        reader.read_to_end(&mut identifier)?;

        let header = FrameHeader::new(FRAME_ID, frame_flags);
        Ok(Some(Self {
            header,
            owner,
            identifier,
        }))
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let Self {
            owner, identifier, ..
        } = self;

        let mut content = Vec::with_capacity(owner.len() + 1 + identifier.len());
        content.extend(encode_text(owner.as_str(), TextEncoding::Latin1, true));
        content.extend_from_slice(identifier);

        content
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/audio_tags/id3/v2/items/unique_file_identifier/tests.rs"]
mod tests;
