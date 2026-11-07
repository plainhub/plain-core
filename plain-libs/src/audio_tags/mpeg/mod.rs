mod constants;
pub(crate) mod header;
mod properties;
mod read;

pub use header::{ChannelMode, Emphasis, Layer, MpegVersion};
pub use properties::MpegProperties;

use crate::audio_tags::ape::tag::ApeTag;
use crate::audio_tags::id3::v1::tag::Id3v1Tag;
use crate::audio_tags::id3::v2::tag::Id3v2Tag;

#[derive(Default)]

pub struct MpegFile {
    pub(crate) id3v2_tag: Option<Id3v2Tag>,

    pub(crate) id3v1_tag: Option<Id3v1Tag>,

    pub(crate) ape_tag: Option<ApeTag>,

    pub(crate) properties: MpegProperties,
}

crate::audio_tags::macros::impl_audio_file!(MpegFile, MpegProperties, read::read_from);
impl From<MpegFile> for crate::audio_tags::file::TaggedFile {
    fn from(input: MpegFile) -> Self {
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
            crate::audio_tags::file::FileType::Mpeg,
            input.properties.into(),
            tags,
        )
    }
}
impl MpegFile {
    pub fn id3v2_mut(&mut self) -> Option<&mut Id3v2Tag> {
        self.id3v2_tag.as_mut()
    }
    pub fn set_id3v2(&mut self, tag: Id3v2Tag) -> Option<Id3v2Tag> {
        self.id3v2_tag.replace(tag)
    }
}
