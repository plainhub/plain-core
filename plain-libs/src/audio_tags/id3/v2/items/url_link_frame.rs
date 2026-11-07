use crate::audio_tags::error::Result;
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding, decode_text, encode_text};

use std::hash::Hash;
use std::io::Read;

#[derive(Clone, Debug, Eq)]
pub struct UrlLinkFrame<'a> {
    pub(crate) header: FrameHeader<'a>,
    pub(crate) content: String,
}

impl PartialEq for UrlLinkFrame<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.header.id == other.header.id
    }
}

impl Hash for UrlLinkFrame<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.header.id.hash(state);
    }
}

impl<'a> UrlLinkFrame<'a> {
    pub fn new(id: FrameId<'a>, content: String) -> Self {
        UrlLinkFrame {
            header: FrameHeader::new(id, FrameFlags::default()),
            content,
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
    ) -> Result<Option<Self>>
    where
        R: Read,
    {
        let url = decode_text(
            reader,
            TextDecodeOptions::new()
                .encoding(TextEncoding::Latin1)
                .terminated(true),
        )?;
        if url.bytes_read == 0 {
            return Ok(None);
        }

        let header = FrameHeader::new(id, frame_flags);
        Ok(Some(UrlLinkFrame {
            header,
            content: url.content,
        }))
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        encode_text(&self.content, TextEncoding::Latin1, false)
    }

    pub fn url(&self) -> &str {
        &self.content
    }

    pub fn set_url(&mut self, url: String) -> bool {
        if TextEncoding::verify_latin1(&url) {
            self.content = url;
            return true;
        }

        false
    }
}
