use crate::audio_tags::error::{ErrorKind, Id3v2Error, Id3v2ErrorKind, LoftyError, Result};
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding, decode_text, encode_text};

use std::io::{Cursor, Read};

const FRAME_ID: FrameId<'static> = FrameId::Valid(std::borrow::Cow::Borrowed("GEOB"));

#[derive(PartialEq, Clone, Debug, Eq, Hash)]
pub struct GeneralEncapsulatedObject<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub encoding: TextEncoding,

    pub mime_type: Option<String>,

    pub file_name: Option<String>,

    pub descriptor: Option<String>,

    pub data: Vec<u8>,
}

impl GeneralEncapsulatedObject<'_> {
    pub fn new(
        encoding: TextEncoding,
        mime_type: Option<String>,
        file_name: Option<String>,
        descriptor: Option<String>,
        data: Vec<u8>,
    ) -> Self {
        let header = FrameHeader::new(FRAME_ID, FrameFlags::default());
        Self {
            header,
            encoding,
            mime_type,
            file_name,
            descriptor,
            data,
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

    pub fn parse(data: &[u8], frame_flags: FrameFlags) -> Result<Self> {
        if data.len() < 4 {
            return Err(Id3v2Error::new(Id3v2ErrorKind::BadFrameLength).into());
        }

        let encoding = TextEncoding::from_u8(data[0])
            .ok_or_else(|| LoftyError::new(ErrorKind::TextDecode("Found invalid encoding")))?;

        let mut cursor = Cursor::new(&data[1..]);

        let mime_type = decode_text(
            &mut cursor,
            TextDecodeOptions::new()
                .encoding(TextEncoding::Latin1)
                .terminated(true),
        )?;

        let text_decode_options = TextDecodeOptions::new().encoding(encoding).terminated(true);

        let file_name = decode_text(&mut cursor, text_decode_options)?;
        let descriptor = decode_text(&mut cursor, text_decode_options)?;

        let mut data = Vec::new();
        cursor.read_to_end(&mut data)?;

        let header = FrameHeader::new(FRAME_ID, frame_flags);
        Ok(Self {
            header,
            encoding,
            mime_type: mime_type.text_or_none(),
            file_name: file_name.text_or_none(),
            descriptor: descriptor.text_or_none(),
            data,
        })
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let encoding = self.encoding;

        let mut bytes = vec![encoding as u8];

        if let Some(ref mime_type) = self.mime_type {
            bytes.extend(mime_type.as_bytes())
        }

        bytes.push(0);

        let file_name = self.file_name.as_deref();
        bytes.extend(&*encode_text(file_name.unwrap_or(""), encoding, true));

        let descriptor = self.descriptor.as_deref();
        bytes.extend(&*encode_text(descriptor.unwrap_or(""), encoding, true));

        bytes.extend(&self.data);

        bytes
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/audio_tags/id3/v2/items/encapsulated_object/tests.rs"]
mod tests;
