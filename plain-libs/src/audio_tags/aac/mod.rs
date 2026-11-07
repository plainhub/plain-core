// TODO: Currently we only support ADTS, might want to look into ADIF in the future.

mod header;
mod properties;
mod read;

use crate::audio_tags::id3::v1::tag::Id3v1Tag;
use crate::audio_tags::id3::v2::tag::Id3v2Tag;

// Exports

pub use properties::AACProperties;

#[derive(Default)]

pub struct AacFile {
    pub(crate) id3v2_tag: Option<Id3v2Tag>,

    pub(crate) id3v1_tag: Option<Id3v1Tag>,
    pub(crate) properties: AACProperties,
}

crate::audio_tags::macros::impl_audio_file!(AacFile, AACProperties, read::read_from);
impl From<AacFile> for crate::audio_tags::file::TaggedFile {
    fn from(input: AacFile) -> Self {
        let mut tags = Vec::new();
        if let Some(tag) = input.id3v2_tag {
            tags.push(tag.into());
        }
        if let Some(tag) = input.id3v1_tag {
            tags.push(tag.into());
        }
        crate::audio_tags::file::TaggedFile::new(
            crate::audio_tags::file::FileType::Aac,
            input.properties.into(),
            tags,
        )
    }
}
impl AacFile {
    pub fn id3v2_mut(&mut self) -> Option<&mut Id3v2Tag> {
        self.id3v2_tag.as_mut()
    }
    pub fn set_id3v2(&mut self, tag: Id3v2Tag) -> Option<Id3v2Tag> {
        self.id3v2_tag.replace(tag)
    }
}
