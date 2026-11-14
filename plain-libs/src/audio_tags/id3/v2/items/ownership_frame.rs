use crate::audio_tags::error::{ErrorKind, Id3v2Error, Id3v2ErrorKind, LoftyError, Result};
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::util::text::{
    TextDecodeOptions, TextEncoding, decode_text, encode_text, utf8_decode_str,
};

use std::borrow::Cow;
use std::hash::Hash;
use std::io::Read;

use crate::binary::ReadBytesExt;

const FRAME_ID: FrameId<'static> = FrameId::Valid(Cow::Borrowed("OWNE"));

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct OwnershipFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub encoding: TextEncoding,

    pub price_paid: String,

    pub date_of_purchase: String,

    pub seller: String,
}

impl OwnershipFrame<'_> {
    pub fn new(
        encoding: TextEncoding,
        price_paid: String,
        date_of_purchase: String,
        seller: String,
    ) -> Self {
        let header = FrameHeader::new(FRAME_ID, FrameFlags::default());
        Self {
            header,
            encoding,
            price_paid,
            date_of_purchase,
            seller,
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
        let Ok(encoding_byte) = reader.read_u8() else {
            return Ok(None);
        };

        let encoding = TextEncoding::from_u8(encoding_byte)
            .ok_or_else(|| LoftyError::new(ErrorKind::TextDecode("Found invalid encoding")))?;
        let price_paid = decode_text(
            reader,
            TextDecodeOptions::new()
                .encoding(TextEncoding::Latin1)
                .terminated(true),
        )?
        .content;

        let mut date_bytes = [0u8; 8];
        reader.read_exact(&mut date_bytes)?;

        let date_of_purchase = utf8_decode_str(&date_bytes)?.to_owned();

        let seller = decode_text(reader, TextDecodeOptions::new().encoding(encoding))?.content;

        let header = FrameHeader::new(FRAME_ID, frame_flags);
        Ok(Some(OwnershipFrame {
            header,
            encoding,
            price_paid,
            date_of_purchase,
            seller,
        }))
    }

    pub fn as_bytes(&self, is_id3v23: bool) -> Result<Vec<u8>> {
        let mut encoding = self.encoding;
        if is_id3v23 {
            encoding = encoding.to_id3v23();
        }

        let mut bytes = vec![encoding as u8];

        bytes.extend(encode_text(&self.price_paid, TextEncoding::Latin1, true));
        if self.date_of_purchase.len() < 8 {
            return Err(Id3v2Error::new(Id3v2ErrorKind::BadFrameLength).into());
        }

        bytes.extend(self.date_of_purchase.as_bytes().iter().take(8));
        bytes.extend(encode_text(&self.seller, encoding, false));

        Ok(bytes)
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/audio_tags/id3/v2/items/ownership_frame/tests.rs"]
mod tests;
