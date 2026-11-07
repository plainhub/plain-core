use crate::audio_tags::error::{Id3v2Error, Id3v2ErrorKind, Result};
use crate::audio_tags::id3::v2::frame::content::verify_encoding;
use crate::audio_tags::id3::v2::header::Id3v2Version;
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};
use crate::audio_tags::tag::items::Lang;
use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding, decode_text, encode_text};

use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::io::Read;

use crate::utils::binary::ReadBytesExt;

// Generic struct for a text frame that has a language
//
// This exists to deduplicate some code between `CommentFrame` and `UnsynchronizedTextFrame`
struct LanguageFrame {
    pub encoding: TextEncoding,
    pub language: Lang,
    pub description: String,
    pub content: String,
}

impl LanguageFrame {
    fn parse<R>(reader: &mut R, version: Id3v2Version) -> Result<Option<Self>>
    where
        R: Read,
    {
        let Ok(encoding_byte) = reader.read_u8() else {
            return Ok(None);
        };

        let encoding = verify_encoding(encoding_byte, version)?;

        let mut language = [0; 3];
        reader.read_exact(&mut language)?;

        let description = decode_text(
            reader,
            TextDecodeOptions::new().encoding(encoding).terminated(true),
        )?
        .content;
        let content = decode_text(reader, TextDecodeOptions::new().encoding(encoding))?.content;

        Ok(Some(Self {
            encoding,
            language,
            description,
            content,
        }))
    }

    fn create_bytes(
        mut encoding: TextEncoding,
        language: [u8; 3],
        description: &str,
        content: &str,
        is_id3v23: bool,
    ) -> Result<Vec<u8>> {
        if is_id3v23 {
            encoding = encoding.to_id3v23();
        }

        let mut bytes = vec![encoding as u8];

        if language.len() != 3 || language.iter().any(|c| !c.is_ascii_alphabetic()) {
            return Err(Id3v2Error::new(Id3v2ErrorKind::InvalidLanguage(language)).into());
        }

        bytes.extend(language);
        bytes.extend(encode_text(description, encoding, true).iter());
        bytes.extend(encode_text(content, encoding, false));

        Ok(bytes)
    }
}

#[derive(Clone, Debug, Eq)]
pub struct CommentFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub encoding: TextEncoding,

    pub language: Lang,

    pub description: String,

    pub content: String,
}

impl PartialEq for CommentFrame<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.language == other.language && self.description == other.description
    }
}

impl Hash for CommentFrame<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.language.hash(state);
        self.description.hash(state);
    }
}

impl CommentFrame<'_> {
    const FRAME_ID: FrameId<'static> = FrameId::Valid(Cow::Borrowed("COMM"));

    pub fn new(
        encoding: TextEncoding,
        language: Lang,
        description: String,
        content: String,
    ) -> Self {
        let header = FrameHeader::new(Self::FRAME_ID, FrameFlags::default());
        Self {
            header,
            encoding,
            language,
            description,
            content,
        }
    }

    pub fn id(&self) -> FrameId<'_> {
        Self::FRAME_ID
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
        let Some(language_frame) = LanguageFrame::parse(reader, version)? else {
            return Ok(None);
        };

        let header = FrameHeader::new(Self::FRAME_ID, frame_flags);
        Ok(Some(Self {
            header,
            encoding: language_frame.encoding,
            language: language_frame.language,
            description: language_frame.description,
            content: language_frame.content,
        }))
    }

    pub fn as_bytes(&self, is_id3v23: bool) -> Result<Vec<u8>> {
        LanguageFrame::create_bytes(
            self.encoding,
            self.language,
            &self.description,
            &self.content,
            is_id3v23,
        )
    }
}

#[derive(Clone, Debug, Eq)]
pub struct UnsynchronizedTextFrame<'a> {
    pub(crate) header: FrameHeader<'a>,

    pub encoding: TextEncoding,

    pub language: Lang,

    pub description: String,

    pub content: String,
}

impl PartialEq for UnsynchronizedTextFrame<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.language == other.language && self.description == other.description
    }
}

impl Hash for UnsynchronizedTextFrame<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.language.hash(state);
        self.description.hash(state);
    }
}

impl UnsynchronizedTextFrame<'_> {
    const FRAME_ID: FrameId<'static> = FrameId::Valid(Cow::Borrowed("USLT"));

    pub fn new(
        encoding: TextEncoding,
        language: Lang,
        description: String,
        content: String,
    ) -> Self {
        let header = FrameHeader::new(Self::FRAME_ID, FrameFlags::default());
        Self {
            header,
            encoding,
            language,
            description,
            content,
        }
    }

    pub fn id(&self) -> FrameId<'_> {
        Self::FRAME_ID
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
        let Some(language_frame) = LanguageFrame::parse(reader, version)? else {
            return Ok(None);
        };

        let header = FrameHeader::new(Self::FRAME_ID, frame_flags);
        Ok(Some(Self {
            header,
            encoding: language_frame.encoding,
            language: language_frame.language,
            description: language_frame.description,
            content: language_frame.content,
        }))
    }

    pub fn as_bytes(&self, is_id3v23: bool) -> Result<Vec<u8>> {
        LanguageFrame::create_bytes(
            self.encoding,
            self.language,
            &self.description,
            &self.content,
            is_id3v23,
        )
    }
}
