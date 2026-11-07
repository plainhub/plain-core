pub(super) mod properties;

use super::find_last_page;
use super::tag::VorbisComments;
use crate::audio_tags::config::ParseOptions;
use crate::audio_tags::error::Result;
use crate::audio_tags::ogg::constants::{OPUSHEAD, OPUSTAGS};
use properties::OpusProperties;

use std::io::{Read, Seek};

pub struct OpusFile {
    pub(crate) vorbis_comments_tag: VorbisComments,

    pub(crate) properties: OpusProperties,
}

impl OpusFile {
    fn read_from<R>(reader: &mut R, parse_options: ParseOptions) -> Result<Self>
    where
        R: Read + Seek,
    {
        let file_information =
            super::read::read_from(reader, OPUSHEAD, OPUSTAGS, 2, parse_options)?;

        Ok(Self {
            properties: if parse_options.read_properties {
                properties::read_properties(reader, &file_information.1, &file_information.2)?
            } else {
                OpusProperties::default()
            },
            // A metadata packet is mandatory in Opus
            vorbis_comments_tag: file_information.0.unwrap_or_default(),
        })
    }
}

crate::audio_tags::macros::impl_audio_file!(OpusFile, OpusProperties, Self::read_from);
impl From<OpusFile> for crate::audio_tags::file::TaggedFile {
    fn from(input: OpusFile) -> Self {
        let mut tags = Vec::new();
        tags.push(input.vorbis_comments_tag.into());
        crate::audio_tags::file::TaggedFile::new(
            crate::audio_tags::file::FileType::Opus,
            input.properties.into(),
            tags,
        )
    }
}
