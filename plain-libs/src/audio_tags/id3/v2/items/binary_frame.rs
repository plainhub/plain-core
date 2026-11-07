use crate::audio_tags::error::Result;
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};

use std::io::Read;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct BinaryFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub data: Vec<u8>,
}

impl<'a> BinaryFrame<'a> {
    pub fn new(id: FrameId<'a>, data: Vec<u8>) -> Self {
        let header = FrameHeader::new(id, FrameFlags::default());
        Self { header, data }
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

    pub fn parse<R>(reader: &mut R, id: FrameId<'a>, frame_flags: FrameFlags) -> Result<Self>
    where
        R: Read,
    {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;

        let header = FrameHeader::new(id, frame_flags);
        Ok(BinaryFrame { header, data })
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let Self { data, .. } = self;
        data.clone()
    }
}
