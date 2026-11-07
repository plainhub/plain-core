mod properties;
mod read;
pub(crate) mod tag;

use crate::audio_tags::id3::v2::tag::Id3v2Tag;

// Exports

pub use properties::{AiffCompressionType, AiffProperties};
pub use tag::{AiffTextChunks, Comment};

pub struct AiffFile {
    pub(crate) text_chunks_tag: Option<AiffTextChunks>,

    pub(crate) id3v2_tag: Option<Id3v2Tag>,

    pub(crate) properties: AiffProperties,
}

crate::audio_tags::macros::impl_audio_file!(AiffFile, AiffProperties, read::read_from);
impl From<AiffFile> for crate::audio_tags::file::TaggedFile {
    fn from(input: AiffFile) -> Self {
        let mut tags = Vec::new();
        if let Some(tag) = input.text_chunks_tag {
            tags.push(tag.into());
        }
        if let Some(tag) = input.id3v2_tag {
            tags.push(tag.into());
        }
        crate::audio_tags::file::TaggedFile::new(
            crate::audio_tags::file::FileType::Aiff,
            input.properties.into(),
            tags,
        )
    }
}
impl AiffFile {
    pub fn text_chunks(&self) -> Option<&AiffTextChunks> {
        self.text_chunks_tag.as_ref()
    }
    pub fn id3v2_mut(&mut self) -> Option<&mut Id3v2Tag> {
        self.id3v2_tag.as_mut()
    }
    pub fn set_id3v2(&mut self, tag: Id3v2Tag) -> Option<Id3v2Tag> {
        self.id3v2_tag.replace(tag)
    }
}
