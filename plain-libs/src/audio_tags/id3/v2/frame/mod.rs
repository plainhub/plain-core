pub(super) mod content;
mod conversion;
pub(super) mod header;
pub(super) mod read;

use super::header::Id3v2Version;
use super::items::{
    AttachedPictureFrame, BinaryFrame, CommentFrame, EventTimingCodesFrame, ExtendedTextFrame,
    ExtendedUrlFrame, KeyValueFrame, OwnershipFrame, PopularimeterFrame, PrivateFrame,
    RelativeVolumeAdjustmentFrame, TextInformationFrame, TimestampFrame, UniqueFileIdentifierFrame,
    UnsynchronizedTextFrame, UrlLinkFrame,
};
use crate::audio_tags::error::Result;
use crate::audio_tags::id3::v2::FrameHeader;
use crate::audio_tags::util::text::TextEncoding;
use header::FrameId;

use std::borrow::Cow;
use std::hash::Hash;
use std::ops::Deref;

pub(super) const MUSICBRAINZ_UFID_OWNER: &str = "http://musicbrainz.org";

pub(super) const EMPTY_CONTENT_DESCRIPTOR: String = String::new();

// TODO: Messy module, rough conversions

macro_rules! define_frames {
	(
		$(#[$meta:meta])*
		pub enum Frame<'a> {
			$(
				$variant:ident($type:ty),
			)*
		}
	) => {
		$(#[$meta])*
		pub enum Frame<'a> {
			$(
				$variant($type),
			)*
		}

		impl Frame<'_> {

			pub fn id(&self) -> &FrameId<'_> {
				match self {
					$(
						Frame::$variant(frame) => &frame.header.id,
					)*
				}
			}

			pub fn flags(&self) -> FrameFlags {
				match self {
					$(
						Frame::$variant(frame) => frame.flags(),
					)*
				}
			}

			pub fn set_flags(&mut self, flags: FrameFlags) {
				match self {
					$(
						Frame::$variant(frame) => frame.set_flags(flags),
					)*
				}
			}
		}

		$(
			impl<'a> From<$type> for Frame<'a> {
				fn from(value: $type) -> Self {
					Frame::$variant(value)
				}
			}
		)*
	}
}

define_frames! {

    #[non_exhaustive]
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub enum Frame<'a> {

        Comment(CommentFrame<'a>),

        UnsynchronizedText(UnsynchronizedTextFrame<'a>),

        Text(TextInformationFrame<'a>),

        UserText(ExtendedTextFrame<'a>),

        Url(UrlLinkFrame<'a>),

        UserUrl(ExtendedUrlFrame<'a>),

        Picture(AttachedPictureFrame<'a>),

        Popularimeter(PopularimeterFrame<'a>),

        KeyValue(KeyValueFrame<'a>),

        RelativeVolumeAdjustment(RelativeVolumeAdjustmentFrame<'a>),

        UniqueFileIdentifier(UniqueFileIdentifierFrame<'a>),

        Ownership(OwnershipFrame<'a>),

        EventTimingCodes(EventTimingCodesFrame<'a>),

        Private(PrivateFrame<'a>),

        Timestamp(TimestampFrame<'a>),

        Binary(BinaryFrame<'a>),
    }
}

impl<'a> Frame<'a> {
    pub fn id_str(&self) -> &str {
        self.id().as_str()
    }

    // Used internally, has no correctness checks
    pub(crate) fn text(id: Cow<'a, str>, content: String) -> Self {
        Frame::Text(TextInformationFrame {
            header: FrameHeader::new(FrameId::Valid(id), FrameFlags::default()),
            encoding: TextEncoding::UTF8,
            value: content,
        })
    }
}

impl Frame<'_> {
    pub(super) fn is_empty(&self) -> Option<bool> {
        let is_empty = match self {
            Frame::Text(text) => text.value.is_empty(),
            Frame::UserText(extended_text) => extended_text.content.is_empty(),
            Frame::Url(link) => link.content.is_empty(),
            Frame::UserUrl(extended_url) => extended_url.content.is_empty(),
            Frame::Comment(comment) => comment.content.is_empty(),
            Frame::UnsynchronizedText(unsync_text) => unsync_text.content.is_empty(),
            Frame::Picture(picture) => picture.picture.data.is_empty(),
            Frame::KeyValue(key_value) => key_value.key_value_pairs.is_empty(),
            Frame::UniqueFileIdentifier(ufid) => ufid.identifier.is_empty(),
            Frame::EventTimingCodes(event_timing) => event_timing.events.is_empty(),
            Frame::Private(private) => private.private_data.is_empty(),
            Frame::Binary(binary) => binary.data.is_empty(),
            Frame::Popularimeter(_)
            | Frame::RelativeVolumeAdjustment(_)
            | Frame::Ownership(_)
            | Frame::Timestamp(_) => {
                // Undefined.
                return None;
            }
        };
        Some(is_empty)
    }
}

impl Frame<'_> {
    pub(super) fn as_bytes(&self, is_id3v23: bool) -> Result<Vec<u8>> {
        Ok(match self {
            Frame::Comment(comment) => comment.as_bytes(is_id3v23)?,
            Frame::UnsynchronizedText(lf) => lf.as_bytes(is_id3v23)?,
            Frame::Text(tif) => tif.as_bytes(is_id3v23),
            Frame::UserText(content) => content.as_bytes(is_id3v23),
            Frame::UserUrl(content) => content.as_bytes(is_id3v23),
            Frame::Url(link) => link.as_bytes(),
            Frame::Picture(attached_picture) => {
                let version = if is_id3v23 {
                    Id3v2Version::V3
                } else {
                    Id3v2Version::V4
                };

                attached_picture.as_bytes(version)?
            }
            Frame::Popularimeter(popularimeter) => popularimeter.as_bytes()?,
            Frame::KeyValue(content) => content.as_bytes(is_id3v23),
            Frame::RelativeVolumeAdjustment(frame) => frame.as_bytes(),
            Frame::UniqueFileIdentifier(frame) => frame.as_bytes(),
            Frame::Ownership(frame) => frame.as_bytes(is_id3v23)?,
            Frame::EventTimingCodes(frame) => frame.as_bytes(),
            Frame::Private(frame) => frame.as_bytes()?,
            Frame::Timestamp(frame) => frame.as_bytes(is_id3v23)?,
            Frame::Binary(frame) => frame.as_bytes(),
        })
    }

    pub(super) fn name(&self) -> &'static str {
        match self {
            Frame::Comment(_) => "Comment",
            Frame::UnsynchronizedText(_) => "UnsynchronizedText",
            Frame::Text { .. } => "Text",
            Frame::UserText(_) => "UserText",
            Frame::Url(_) => "Url",
            Frame::UserUrl(_) => "UserUrl",
            Frame::Picture { .. } => "Picture",
            Frame::Popularimeter(_) => "Popularimeter",
            Frame::KeyValue(_) => "KeyValue",
            Frame::UniqueFileIdentifier(_) => "UniqueFileIdentifier",
            Frame::RelativeVolumeAdjustment(_) => "RelativeVolumeAdjustment",
            Frame::Ownership(_) => "Ownership",
            Frame::EventTimingCodes(_) => "EventTimingCodes",
            Frame::Private(_) => "Private",
            Frame::Timestamp(_) => "Timestamp",
            Frame::Binary(_) => "Binary",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]

pub struct FrameFlags {
    pub tag_alter_preservation: bool,

    pub file_alter_preservation: bool,

    pub read_only: bool,

    pub grouping_identity: Option<u8>,

    pub compression: bool,

    pub encryption: Option<u8>,

    pub unsynchronisation: bool, /* TODO: Maybe? This doesn't seem very useful, and it is wasted effort if one forgets to make this false when writing. */

    pub data_length_indicator: Option<u32>,
}

impl FrameFlags {
    pub fn parse_id3v24(flags: u16) -> Self {
        FrameFlags {
            tag_alter_preservation: flags & 0x4000 == 0x4000,
            file_alter_preservation: flags & 0x2000 == 0x2000,
            read_only: flags & 0x1000 == 0x1000,
            grouping_identity: (flags & 0x0040 == 0x0040).then_some(0),
            compression: flags & 0x0008 == 0x0008,
            encryption: (flags & 0x0004 == 0x0004).then_some(0),
            unsynchronisation: flags & 0x0002 == 0x0002,
            data_length_indicator: (flags & 0x0001 == 0x0001).then_some(0),
        }
    }

    pub fn parse_id3v23(flags: u16) -> Self {
        FrameFlags {
            tag_alter_preservation: flags & 0x8000 == 0x8000,
            file_alter_preservation: flags & 0x4000 == 0x4000,
            read_only: flags & 0x2000 == 0x2000,
            grouping_identity: (flags & 0x0020 == 0x0020).then_some(0),
            compression: flags & 0x0080 == 0x0080,
            encryption: (flags & 0x0040 == 0x0040).then_some(0),
            unsynchronisation: false,
            data_length_indicator: None,
        }
    }

    pub fn as_id3v24_bytes(&self) -> u16 {
        let mut flags = 0;

        if *self == FrameFlags::default() {
            return flags;
        }

        if self.tag_alter_preservation {
            flags |= 0x4000
        }

        if self.file_alter_preservation {
            flags |= 0x2000
        }

        if self.read_only {
            flags |= 0x1000
        }

        if self.grouping_identity.is_some() {
            flags |= 0x0040
        }

        if self.compression {
            flags |= 0x0008
        }

        if self.encryption.is_some() {
            flags |= 0x0004
        }

        if self.unsynchronisation {
            flags |= 0x0002
        }

        if self.data_length_indicator.is_some() {
            flags |= 0x0001
        }

        flags
    }

    pub fn as_id3v23_bytes(&self) -> u16 {
        let mut flags = 0;

        if *self == FrameFlags::default() {
            return flags;
        }

        if self.tag_alter_preservation {
            flags |= 0x8000
        }

        if self.file_alter_preservation {
            flags |= 0x4000
        }

        if self.read_only {
            flags |= 0x2000
        }

        if self.grouping_identity.is_some() {
            flags |= 0x0020
        }

        if self.compression {
            flags |= 0x0080
        }

        if self.encryption.is_some() {
            flags |= 0x0040
        }

        flags
    }
}

#[derive(Clone)]
pub(crate) struct FrameRef<'a>(pub(crate) Cow<'a, Frame<'a>>);

impl<'a> Deref for FrameRef<'a> {
    type Target = Frame<'a>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl<'a> Frame<'a> {
    pub(crate) fn as_opt_ref(&'a self) -> Option<FrameRef<'a>> {
        if let FrameId::Valid(_) = self.id() {
            Some(FrameRef(Cow::Borrowed(self)))
        } else {
            None
        }
    }
}
