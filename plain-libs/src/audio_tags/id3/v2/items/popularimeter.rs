use crate::audio_tags::error::Result;
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::util::alloc::VecFallibleCapacity;
use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding, decode_text, encode_text};

use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::io::Read;

use crate::utils::binary::ReadBytesExt;

const FRAME_ID: FrameId<'static> = FrameId::Valid(Cow::Borrowed("POPM"));

#[derive(Clone, Debug, Eq)]
pub struct PopularimeterFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub email: String,

    pub rating: u8,

    pub counter: u64,
}

impl PartialEq for PopularimeterFrame<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.email == other.email
    }
}

impl Hash for PopularimeterFrame<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.email.hash(state);
    }
}

impl PopularimeterFrame<'_> {
    pub fn new(email: String, rating: u8, counter: u64) -> Self {
        let header = FrameHeader::new(FRAME_ID, FrameFlags::default());
        Self {
            header,
            email,
            rating,
            counter,
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

    pub fn parse<R>(reader: &mut R, frame_flags: FrameFlags) -> Result<Self>
    where
        R: Read,
    {
        let email = decode_text(
            reader,
            TextDecodeOptions::new()
                .encoding(TextEncoding::Latin1)
                .terminated(true),
        )?;
        let rating = reader.read_u8()?;

        let mut counter_content = Vec::new();
        reader.read_to_end(&mut counter_content)?;

        let counter;
        let remaining_size = counter_content.len();
        if remaining_size > 8 {
            counter = u64::MAX;
        } else {
            let mut counter_bytes = [0; 8];
            let counter_start_pos = 8 - remaining_size;

            counter_bytes[counter_start_pos..].copy_from_slice(&counter_content);
            counter = u64::from_be_bytes(counter_bytes);
        }

        let header = FrameHeader::new(FRAME_ID, frame_flags);
        Ok(Self {
            header,
            email: email.content,
            rating,
            counter,
        })
    }

    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        let mut content = Vec::try_with_capacity_stable(self.email.len() + 9)?;
        content.extend(encode_text(self.email.as_str(), TextEncoding::Latin1, true));
        content.push(self.rating);

        // When the counter reaches all one's, one byte is inserted in front of the counter
        // thus making the counter eight bits bigger in the same away as the play counter ("PCNT")
        //
        // $xx xx xx xx (xx ...)
        if let Ok(counter) = u32::try_from(self.counter) {
            content.extend(counter.to_be_bytes())
        } else {
            let counter_bytes = self.counter.to_be_bytes();
            let i = counter_bytes.iter().position(|b| *b != 0).unwrap_or(4);

            content.extend(&counter_bytes[i..]);
        }

        Ok(content)
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/audio_tags/id3/v2/items/popularimeter/tests.rs"]
mod tests;
