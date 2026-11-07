use crate::audio_tags::error::{ErrorKind, Id3v2Error, Id3v2ErrorKind, LoftyError, Result};
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding, decode_text, encode_text};

use std::borrow::Cow;
use std::hash::{Hash, Hasher};

use crate::utils::binary::ReadBytesExt as _;

const FRAME_ID: FrameId<'static> = FrameId::Valid(Cow::Borrowed("ATXT"));

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct AudioTextFrameFlags {
    pub scrambling: bool,
}

impl AudioTextFrameFlags {
    pub fn from_u8(byte: u8) -> Self {
        Self {
            scrambling: byte & 0x01 > 0,
        }
    }

    pub fn as_u8(&self) -> u8 {
        let mut byte = 0_u8;

        if self.scrambling {
            byte |= 0x01
        }

        byte
    }
}

#[derive(Clone, Debug, Eq)]
pub struct AudioTextFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub encoding: TextEncoding,

    pub mime_type: String,

    pub flags: AudioTextFrameFlags,

    pub equivalent_text: String,

    pub audio_data: Vec<u8>,
}

impl PartialEq for AudioTextFrame<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.equivalent_text == other.equivalent_text
    }
}

impl Hash for AudioTextFrame<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.equivalent_text.hash(state);
    }
}

impl AudioTextFrame<'_> {
    pub fn new(
        encoding: TextEncoding,
        mime_type: String,
        flags: AudioTextFrameFlags,
        equivalent_text: String,
        audio_data: Vec<u8>,
    ) -> Self {
        let header = FrameHeader::new(FRAME_ID, FrameFlags::default());
        Self {
            header,
            encoding,
            mime_type,
            flags,
            equivalent_text,
            audio_data,
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

    pub fn parse(bytes: &[u8], frame_flags: FrameFlags) -> Result<Self> {
        if bytes.len() < 4 {
            return Err(Id3v2Error::new(Id3v2ErrorKind::BadFrameLength).into());
        }

        let content = &mut &bytes[..];
        let encoding = TextEncoding::from_u8(content.read_u8()?)
            .ok_or_else(|| LoftyError::new(ErrorKind::TextDecode("Found invalid encoding")))?;

        let mime_type = decode_text(
            content,
            TextDecodeOptions::new()
                .encoding(TextEncoding::Latin1)
                .terminated(true),
        )?
        .content;

        let flags = AudioTextFrameFlags::from_u8(content.read_u8()?);

        let equivalent_text = decode_text(
            content,
            TextDecodeOptions::new().encoding(encoding).terminated(true),
        )?
        .content;

        let header = FrameHeader::new(FRAME_ID, frame_flags);
        Ok(Self {
            header,
            encoding,
            mime_type,
            flags,
            equivalent_text,
            audio_data: content.to_vec(),
        })
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut content = vec![self.encoding as u8];

        content.extend(encode_text(
            self.mime_type.as_str(),
            TextEncoding::Latin1,
            true,
        ));
        content.push(self.flags.as_u8());
        content.extend(encode_text(&self.equivalent_text, self.encoding, true));
        content.extend(&self.audio_data);
        content
    }
}

const SCRAMBLING_TABLE: [u8; 127] = {
    let mut scrambling_table = [0_u8; 127];
    scrambling_table[0] = 0xFE;

    let mut i = 0;
    loop {
        let byte = scrambling_table[i];

        let bit7 = (byte >> 7) & 0x01;
        let bit6 = (byte >> 6) & 0x01;
        let bit5 = (byte >> 5) & 0x01;
        let bit4 = (byte >> 4) & 0x01;
        let bit3 = (byte >> 3) & 0x01;
        let bit2 = (byte >> 2) & 0x01;
        let bit1 = (byte >> 1) & 0x01;
        let bit0 = byte & 0x01;

        let new_byte = ((bit6 ^ bit5) << 7)
            + ((bit5 ^ bit4) << 6)
            + ((bit4 ^ bit3) << 5)
            + ((bit3 ^ bit2) << 4)
            + ((bit2 ^ bit1) << 3)
            + ((bit1 ^ bit0) << 2)
            + ((bit7 ^ bit5) << 1)
            + (bit6 ^ bit4);

        if new_byte == 0xFE {
            break;
        }

        i += 1;
        scrambling_table[i] = new_byte;
    }

    scrambling_table
};

pub fn scramble(audio_data: &mut [u8]) {
    let mut idx = 0;
    for b in audio_data.iter_mut() {
        *b ^= SCRAMBLING_TABLE[idx];
        if idx == 126 {
            idx = 0;
        } else {
            idx += 1;
        }
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/audio_tags/id3/v2/items/audio_text_frame/tests.rs"]
mod tests;
