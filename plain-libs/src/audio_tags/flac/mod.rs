pub(crate) mod block;
pub(crate) mod properties;
mod read;

use crate::audio_tags::error::{LoftyError, Result};
use crate::audio_tags::file::{FileType, TaggedFile};
use crate::audio_tags::id3::v2::tag::Id3v2Tag;
use crate::audio_tags::ogg::tag::VorbisCommentsRef;
use crate::audio_tags::ogg::{OggPictureStorage, VorbisComments};
use crate::audio_tags::picture::{Picture, PictureInformation};
use crate::audio_tags::tag::TagExt;

use std::borrow::Cow;

// Exports
pub use properties::FlacProperties;

pub struct FlacFile {
    pub(crate) id3v2_tag: Option<Id3v2Tag>,

    pub(crate) vorbis_comments_tag: Option<VorbisComments>,
    pub(crate) pictures: Vec<(Picture, PictureInformation)>,

    pub(crate) properties: FlacProperties,
}

impl FlacFile {}

impl OggPictureStorage for FlacFile {
    fn pictures(&self) -> &[(Picture, PictureInformation)] {
        &self.pictures
    }
}

impl From<FlacFile> for TaggedFile {
    fn from(mut value: FlacFile) -> Self {
        TaggedFile {
            ty: FileType::Flac,
            properties: value.properties.into(),
            tags: {
                let mut tags = Vec::with_capacity(2);

                if let Some(id3v2) = value.id3v2_tag {
                    tags.push(id3v2.into());
                }

                // Move our pictures into a `VorbisComments` tag, creating one if necessary
                match value.vorbis_comments_tag {
                    Some(mut vorbis_comments) => {
                        vorbis_comments.pictures.append(&mut value.pictures);
                        tags.push(vorbis_comments.into());
                    }
                    None if !value.pictures.is_empty() => tags.push(
                        VorbisComments {
                            vendor: String::new(),
                            items: Vec::new(),
                            pictures: value.pictures,
                        }
                        .into(),
                    ),
                    _ => {}
                }

                tags
            },
        }
    }
}

crate::audio_tags::macros::impl_audio_file!(FlacFile, FlacProperties, read::read_from);
impl FlacFile {
    pub fn id3v2_mut(&mut self) -> Option<&mut Id3v2Tag> {
        self.id3v2_tag.as_mut()
    }
    pub fn set_id3v2(&mut self, tag: Id3v2Tag) -> Option<Id3v2Tag> {
        self.id3v2_tag.replace(tag)
    }
}
