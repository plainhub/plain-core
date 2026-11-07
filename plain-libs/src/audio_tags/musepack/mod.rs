pub mod constants;
mod read;
pub mod sv4to6;
pub mod sv7;
pub mod sv8;

use crate::audio_tags::ape::tag::ApeTag;
use crate::audio_tags::id3::v1::tag::Id3v1Tag;
use crate::audio_tags::id3::v2::tag::Id3v2Tag;
use crate::audio_tags::properties::FileProperties;

#[derive(Debug, Clone, PartialEq)]
pub enum MpcProperties {
    Sv8(sv8::MpcSv8Properties),

    Sv7(sv7::MpcSv7Properties),

    Sv4to6(sv4to6::MpcSv4to6Properties),
}

impl Default for MpcProperties {
    fn default() -> Self {
        Self::Sv8(sv8::MpcSv8Properties::default())
    }
}

impl From<MpcProperties> for FileProperties {
    fn from(input: MpcProperties) -> Self {
        match input {
            MpcProperties::Sv8(sv8prop) => sv8prop.into(),
            MpcProperties::Sv7(sv7prop) => sv7prop.into(),
            MpcProperties::Sv4to6(sv4to6prop) => sv4to6prop.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MpcStreamVersion {
    #[default]
    Sv8,

    Sv7,

    Sv4to6,
}

#[derive(Default)]

pub struct MpcFile {
    pub(crate) stream_version: MpcStreamVersion,

    pub(crate) id3v2_tag: Option<Id3v2Tag>,

    pub(crate) id3v1_tag: Option<Id3v1Tag>,

    pub(crate) ape_tag: Option<ApeTag>,

    pub(crate) properties: MpcProperties,
}

impl MpcFile {
    pub fn stream_version(&self) -> MpcStreamVersion {
        self.stream_version
    }
}

crate::audio_tags::macros::impl_audio_file!(MpcFile, MpcProperties, read::read_from);
impl From<MpcFile> for crate::audio_tags::file::TaggedFile {
    fn from(input: MpcFile) -> Self {
        let mut tags = Vec::new();
        if let Some(tag) = input.id3v2_tag {
            tags.push(tag.into());
        }
        if let Some(tag) = input.id3v1_tag {
            tags.push(tag.into());
        }
        if let Some(tag) = input.ape_tag {
            tags.push(tag.into());
        }
        crate::audio_tags::file::TaggedFile::new(
            crate::audio_tags::file::FileType::Mpc,
            input.properties.into(),
            tags,
        )
    }
}
impl MpcFile {
    pub fn id3v2_mut(&mut self) -> Option<&mut Id3v2Tag> {
        self.id3v2_tag.as_mut()
    }
    pub fn set_id3v2(&mut self, tag: Id3v2Tag) -> Option<Id3v2Tag> {
        self.id3v2_tag.replace(tag)
    }
}
