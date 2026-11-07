mod properties;
mod read;

use crate::audio_tags::ape::tag::ApeTag;
use crate::audio_tags::id3::v1::tag::Id3v1Tag;

// Exports
pub use properties::WavPackProperties;

#[derive(Default)]

pub struct WavPackFile {
    pub(crate) id3v1_tag: Option<Id3v1Tag>,

    pub(crate) ape_tag: Option<ApeTag>,

    pub(crate) properties: WavPackProperties,
}

crate::audio_tags::macros::impl_audio_file!(WavPackFile, WavPackProperties, read::read_from);
impl From<WavPackFile> for crate::audio_tags::file::TaggedFile {
    fn from(input: WavPackFile) -> Self {
        let mut tags = Vec::new();
        if let Some(tag) = input.id3v1_tag {
            tags.push(tag.into());
        }
        if let Some(tag) = input.ape_tag {
            tags.push(tag.into());
        }
        crate::audio_tags::file::TaggedFile::new(
            crate::audio_tags::file::FileType::WavPack,
            input.properties.into(),
            tags,
        )
    }
}
