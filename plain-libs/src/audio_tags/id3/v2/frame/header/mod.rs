pub(super) mod parse;

use crate::audio_tags::error;
use crate::audio_tags::error::{Id3v2Error, Id3v2ErrorKind, LoftyError};
use crate::audio_tags::id3::v2::FrameFlags;
use crate::audio_tags::prelude::ItemKey;
use crate::audio_tags::tag::TagType;

use std::borrow::Cow;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]

pub struct FrameHeader<'a> {
    pub(crate) id: FrameId<'a>,
    pub flags: FrameFlags,
}

impl<'a> FrameHeader<'a> {
    pub const fn new(id: FrameId<'a>, flags: FrameFlags) -> Self {
        Self { id, flags }
    }

    pub const fn id(&'a self) -> &'a FrameId<'a> {
        &self.id
    }
}

#[derive(PartialEq, Clone, Debug, Eq, Hash)]
pub enum FrameId<'a> {
    Valid(Cow<'a, str>),

    Outdated(Cow<'a, str>),
}

impl<'a> FrameId<'a> {
    pub fn new<I>(id: I) -> error::Result<Self>
    where
        I: Into<Cow<'a, str>>,
    {
        Self::new_cow(id.into())
    }

    // Split from generic, public method to avoid code bloat by monomorphization.
    pub(in crate::audio_tags::id3::v2::frame) fn new_cow(id: Cow<'a, str>) -> error::Result<Self> {
        Self::verify_id(&id)?;

        match id.len() {
            3 => Ok(FrameId::Outdated(id)),
            4 => Ok(FrameId::Valid(id)),
            _ => Err(
                Id3v2Error::new(Id3v2ErrorKind::BadFrameId(id.into_owned().into_bytes())).into(),
            ),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            FrameId::Valid(v) | FrameId::Outdated(v) => v,
        }
    }

    pub(in crate::audio_tags::id3::v2::frame) fn verify_id(id_str: &str) -> error::Result<()> {
        for c in id_str.chars() {
            if !c.is_ascii_uppercase() && !c.is_ascii_digit() {
                return Err(Id3v2Error::new(Id3v2ErrorKind::BadFrameId(
                    id_str.as_bytes().to_vec(),
                ))
                .into());
            }
        }

        Ok(())
    }

    pub fn as_borrowed(&'a self) -> Self {
        match self {
            Self::Valid(inner) => Self::Valid(Cow::Borrowed(inner)),
            Self::Outdated(inner) => Self::Outdated(Cow::Borrowed(inner)),
        }
    }

    pub fn into_owned(self) -> FrameId<'static> {
        match self {
            Self::Valid(inner) => FrameId::Valid(Cow::Owned(inner.into_owned())),
            Self::Outdated(inner) => FrameId::Outdated(Cow::Owned(inner.into_owned())),
        }
    }

    pub fn into_inner(self) -> Cow<'a, str> {
        match self {
            FrameId::Valid(v) | FrameId::Outdated(v) => v,
        }
    }
}

impl Display for FrameId<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'a> Into<Cow<'a, str>> for FrameId<'a> {
    fn into(self) -> Cow<'a, str> {
        self.into_inner()
    }
}

impl<'a> TryFrom<&'a ItemKey> for FrameId<'a> {
    type Error = LoftyError;

    fn try_from(value: &'a ItemKey) -> std::prelude::rust_2015::Result<Self, Self::Error> {
        match value {
            ItemKey::Unknown(unknown) if unknown.len() == 4 => {
                Self::verify_id(unknown)?;
                Ok(Self::Valid(Cow::Borrowed(unknown)))
            }
            k => {
                if let Some(mapped) = k.map_key(TagType::Id3v2, false) {
                    if mapped.len() == 4 {
                        Self::verify_id(mapped)?;
                        return Ok(Self::Valid(Cow::Borrowed(mapped)));
                    }
                }

                Err(Id3v2Error::new(Id3v2ErrorKind::UnsupportedFrameId(k.clone())).into())
            }
        }
    }
}
