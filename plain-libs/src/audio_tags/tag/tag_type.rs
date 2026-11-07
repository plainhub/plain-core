#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TagType {
    Ape,

    Id3v1,

    Id3v2,

    Mp4Ilst,

    VorbisComments,

    RiffInfo,

    AiffText,
}
