mod properties;
pub(crate) mod read;
pub(crate) mod tag;

use crate::audio_tags::id3::v2::tag::Id3v2Tag;

// Exports
pub use crate::audio_tags::iff::wav::properties::{WavFormat, WavProperties};
pub use tag::RiffInfoList;

pub struct WavFile {
    pub(crate) riff_info_tag: Option<RiffInfoList>,

    pub(crate) id3v2_tag: Option<Id3v2Tag>,

    pub(crate) properties: WavProperties,
}

crate::audio_tags::macros::impl_audio_file!(WavFile, WavProperties, read::read_from);
impl From<WavFile> for crate::audio_tags::file::TaggedFile {
    fn from(input: WavFile) -> Self {
        let mut tags = Vec::new();
        if let Some(tag) = input.riff_info_tag {
            tags.push(tag.into());
        }
        if let Some(tag) = input.id3v2_tag {
            tags.push(tag.into());
        }
        crate::audio_tags::file::TaggedFile::new(
            crate::audio_tags::file::FileType::Wav,
            input.properties.into(),
            tags,
        )
    }
}
impl WavFile {
    pub fn id3v2_mut(&mut self) -> Option<&mut Id3v2Tag> {
        self.id3v2_tag.as_mut()
    }
    pub fn set_id3v2(&mut self, tag: Id3v2Tag) -> Option<Id3v2Tag> {
        self.id3v2_tag.replace(tag)
    }
}
