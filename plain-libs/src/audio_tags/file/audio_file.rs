use super::tagged_file::TaggedFile;
use crate::audio_tags::config::ParseOptions;
use crate::audio_tags::error::Result;

use std::io::{Read, Seek};

pub trait AudioFile: Into<TaggedFile> {
    type Properties;

    fn read_from<R>(reader: &mut R, parse_options: ParseOptions) -> Result<Self>
    where
        R: Read + Seek,
        Self: Sized;

    fn properties(&self) -> &Self::Properties;
}
