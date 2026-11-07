use crate::audio_tags::error::LoftyError;
use crate::audio_tags::tag::{Accessor, Tag, TagType};

use std::path::Path;

pub trait TagExt: Accessor + Into<Tag> + Sized + private::Sealed {
    type Err: From<std::io::Error> + From<LoftyError>;

    type RefKey<'a>
    where
        Self: 'a;

    fn tag_type(&self) -> TagType;

    fn len(&self) -> usize;

    fn contains<'a>(&'a self, key: Self::RefKey<'a>) -> bool;

    fn is_empty(&self) -> bool;

    fn clear(&mut self);
}

// https://rust-lang.github.io/api-guidelines/future-proofing.html#c-sealed
mod private {
    use crate::audio_tags::ape::ApeTag;
    use crate::audio_tags::id3::v1::Id3v1Tag;
    use crate::audio_tags::id3::v2::Id3v2Tag;
    use crate::audio_tags::iff::aiff::AiffTextChunks;
    use crate::audio_tags::iff::wav::RiffInfoList;
    use crate::audio_tags::mp4::Ilst;
    use crate::audio_tags::ogg::VorbisComments;
    use crate::audio_tags::tag::Tag;

    pub trait Sealed {}

    impl Sealed for AiffTextChunks {}
    impl Sealed for ApeTag {}
    impl Sealed for Id3v1Tag {}
    impl Sealed for Id3v2Tag {}
    impl Sealed for Ilst {}
    impl Sealed for RiffInfoList {}
    impl Sealed for Tag {}
    impl Sealed for VorbisComments {}
}
