pub(crate) mod constants;
pub(crate) mod header;
mod properties;
mod read;
pub(crate) mod tag;

use crate::audio_tags::id3::v1::tag::Id3v1Tag;
use crate::audio_tags::id3::v2::tag::Id3v2Tag;

// Exports

pub use crate::audio_tags::picture::APE_PICTURE_TYPES;
pub use properties::ApeProperties;
pub use tag::ApeTag;
pub use tag::item::ApeItem;

pub struct ApeFile {
    pub(crate) id3v1_tag: Option<Id3v1Tag>,

    pub(crate) id3v2_tag: Option<Id3v2Tag>,

    pub(crate) ape_tag: Option<ApeTag>,

    pub(crate) properties: ApeProperties,
}

crate::audio_tags::macros::impl_audio_file!(ApeFile, ApeProperties, read::read_from);
impl From<ApeFile> for crate::audio_tags::file::TaggedFile {
    fn from(input: ApeFile) -> Self {
        let mut tags = Vec::new();
        if let Some(tag) = input.id3v1_tag {
            tags.push(tag.into());
        }
        if let Some(tag) = input.id3v2_tag {
            tags.push(tag.into());
        }
        if let Some(tag) = input.ape_tag {
            tags.push(tag.into());
        }
        crate::audio_tags::file::TaggedFile::new(
            crate::audio_tags::file::FileType::Ape,
            input.properties.into(),
            tags,
        )
    }
}
impl ApeFile {
    pub fn id3v2_mut(&mut self) -> Option<&mut Id3v2Tag> {
        self.id3v2_tag.as_mut()
    }
    pub fn set_id3v2(&mut self, tag: Id3v2Tag) -> Option<Id3v2Tag> {
        self.id3v2_tag.replace(tag)
    }
}
