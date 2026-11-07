mod atom_info;
pub(crate) mod ilst;
mod moov;
mod properties;
mod read;

// Exports

pub mod constants {
    pub use super::ilst::constants::*;
}

pub use crate::audio_tags::mp4::properties::{AudioObjectType, Mp4Codec, Mp4Properties};
pub use atom_info::AtomIdent;
pub use ilst::Ilst;
pub use ilst::advisory_rating::AdvisoryRating;
pub use ilst::atom::{Atom, AtomData};
pub use ilst::data_type::DataType;

pub(crate) use properties::SAMPLE_RATES;

pub struct Mp4File {
    pub(crate) ftyp: String,

    pub(crate) ilst_tag: Option<Ilst>,

    pub(crate) properties: Mp4Properties,
}

impl Mp4File {
    pub fn ftyp(&self) -> &str {
        self.ftyp.as_ref()
    }
}

crate::audio_tags::macros::impl_audio_file!(Mp4File, Mp4Properties, read::read_from);
impl From<Mp4File> for crate::audio_tags::file::TaggedFile {
    fn from(input: Mp4File) -> Self {
        let mut tags = Vec::new();
        if let Some(tag) = input.ilst_tag {
            tags.push(tag.into());
        }
        crate::audio_tags::file::TaggedFile::new(
            crate::audio_tags::file::FileType::Mp4,
            input.properties.into(),
            tags,
        )
    }
}
impl Mp4File {
    pub fn ilst(&self) -> Option<&Ilst> {
        self.ilst_tag.as_ref()
    }
}
