pub(super) mod properties;

use super::find_last_page;
use super::tag::VorbisComments;
use crate::audio_tags::config::ParseOptions;
use crate::audio_tags::error::Result;
use crate::audio_tags::ogg::constants::{VORBIS_COMMENT_HEAD, VORBIS_IDENT_HEAD};
use properties::VorbisProperties;

use std::io::{Read, Seek};

pub struct VorbisFile {
    pub(crate) vorbis_comments_tag: VorbisComments,

    pub(crate) properties: VorbisProperties,
}

impl VorbisFile {
    fn read_from<R>(reader: &mut R, parse_options: ParseOptions) -> Result<Self>
    where
        R: Read + Seek,
    {
        let file_information = super::read::read_from(
            reader,
            VORBIS_IDENT_HEAD,
            VORBIS_COMMENT_HEAD,
            3,
            parse_options,
        )?;

        Ok(Self {
            properties: if parse_options.read_properties {
                properties::read_properties(reader, &file_information.1, &file_information.2)?
            } else {
                VorbisProperties::default()
            },
            // A metadata packet is mandatory in OGG Vorbis
            vorbis_comments_tag: file_information.0.unwrap_or_default(),
        })
    }
}

crate::audio_tags::macros::impl_audio_file!(VorbisFile, VorbisProperties, Self::read_from);
impl From<VorbisFile> for crate::audio_tags::file::TaggedFile {
    fn from(input: VorbisFile) -> Self {
        let mut tags = Vec::new();
        tags.push(input.vorbis_comments_tag.into());
        crate::audio_tags::file::TaggedFile::new(
            crate::audio_tags::file::FileType::Vorbis,
            input.properties.into(),
            tags,
        )
    }
}
