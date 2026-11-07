use super::Tag;

pub trait SplitTag: private::Sealed {
    type Remainder: MergeTag;

    fn split_tag(self) -> (Self::Remainder, Tag);
}

pub trait MergeTag: private::Sealed {
    type Merged: SplitTag;

    fn merge_tag(self, tag: Tag) -> Self::Merged;
}

// https://rust-lang.github.io/api-guidelines/future-proofing.html#c-sealed
mod private {
    use crate::audio_tags::ape::ApeTag;
    use crate::audio_tags::id3::v1::Id3v1Tag;
    use crate::audio_tags::id3::v2::Id3v2Tag;
    use crate::audio_tags::iff::aiff::AiffTextChunks;
    use crate::audio_tags::iff::wav::RiffInfoList;
    use crate::audio_tags::ogg::VorbisComments;
    use crate::audio_tags::tag::Tag;

    pub trait Sealed {}

    impl Sealed for AiffTextChunks {}
    impl Sealed for crate::audio_tags::iff::aiff::tag::SplitTagRemainder {}

    impl Sealed for ApeTag {}
    impl Sealed for crate::audio_tags::ape::tag::SplitTagRemainder {}

    impl Sealed for Id3v1Tag {}
    impl Sealed for crate::audio_tags::id3::v1::tag::SplitTagRemainder {}

    impl Sealed for Id3v2Tag {}
    impl Sealed for crate::audio_tags::id3::v2::tag::SplitTagRemainder {}

    impl Sealed for crate::audio_tags::mp4::Ilst {}
    impl Sealed for crate::audio_tags::mp4::ilst::SplitTagRemainder {}

    impl Sealed for RiffInfoList {}
    impl Sealed for crate::audio_tags::iff::wav::tag::SplitTagRemainder {}

    impl Sealed for Tag {}
    impl Sealed for crate::audio_tags::tag::SplitTagRemainder {}

    impl Sealed for VorbisComments {}
    impl Sealed for crate::audio_tags::ogg::tag::SplitTagRemainder {}
}
