pub(super) mod properties;

use super::tag::VorbisComments;
use crate::audio_tags::config::ParseOptions;
use crate::audio_tags::error::Result;
use crate::audio_tags::ogg::constants::SPEEXHEADER;
use properties::SpeexProperties;

use std::io::{Read, Seek};

pub struct SpeexFile {
    pub(crate) vorbis_comments_tag: VorbisComments,

    pub(crate) properties: SpeexProperties,
}

impl SpeexFile {
    fn read_from<R>(reader: &mut R, parse_options: ParseOptions) -> Result<Self>
    where
        R: Read + Seek,
    {
        let file_information = super::read::read_from(reader, SPEEXHEADER, &[], 2, parse_options)?;

        Ok(Self {
            properties: if parse_options.read_properties {
                properties::read_properties(reader, &file_information.1, &file_information.2)?
            } else {
                SpeexProperties::default()
            },
            // A metadata packet is mandatory in Speex
            vorbis_comments_tag: file_information.0.unwrap_or_default(),
        })
    }
}

crate::audio_tags::macros::impl_audio_file!(SpeexFile, SpeexProperties, Self::read_from);
impl From<SpeexFile> for crate::audio_tags::file::TaggedFile {
    fn from(input: SpeexFile) -> Self {
        let mut tags = Vec::new();
        tags.push(input.vorbis_comments_tag.into());
        crate::audio_tags::file::TaggedFile::new(
            crate::audio_tags::file::FileType::Speex,
            input.properties.into(),
            tags,
        )
    }
}
