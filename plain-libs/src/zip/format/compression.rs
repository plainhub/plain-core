#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[repr(u16)]
#[non_exhaustive]
pub enum Compression {
    #[default]
    Stored = 0,
    Shrink = 1,
    ReduceFactor1 = 2,
    ReduceFactor2 = 3,
    ReduceFactor3 = 4,
    ReduceFactor4 = 5,
    Implode = 6,
    TokenizingCompression = 7,
    Deflated = 8,
    Deflate64 = 9,
    PkwareDataCompressionImploding = 10,
    Reserved11 = 11,
    Bzip2 = 12,
    Reserved13 = 13,
    Lzma = 14,
    Reserved15 = 15,
    CmpscCompression = 16,
    Reserved17 = 17,
    IbmTerseNew = 18,
    IbmLz77 = 19,
    DeprecatedZstd = 20,
    Zstd = 93,
    Mp3 = 94,
    Xz = 95,
    Jpeg = 96,
    WavPack = 97,
    Ppmd = 98,
    Aes = 99,
    Unknown(u16),
}
impl From<u16> for Compression {
    fn from(value: u16) -> Self {
        match value {
            0 => Self::Stored,
            1 => Self::Shrink,
            2 => Self::ReduceFactor1,
            3 => Self::ReduceFactor2,
            4 => Self::ReduceFactor3,
            5 => Self::ReduceFactor4,
            6 => Self::Implode,
            7 => Self::TokenizingCompression,
            8 => Self::Deflated,
            9 => Self::Deflate64,
            10 => Self::PkwareDataCompressionImploding,
            11 => Self::Reserved11,
            12 => Self::Bzip2,
            13 => Self::Reserved13,
            14 => Self::Lzma,
            15 => Self::Reserved15,
            16 => Self::CmpscCompression,
            17 => Self::Reserved17,
            18 => Self::IbmTerseNew,
            19 => Self::IbmLz77,
            20 => Self::DeprecatedZstd,
            93 => Self::Zstd,
            94 => Self::Mp3,
            95 => Self::Xz,
            96 => Self::Jpeg,
            97 => Self::WavPack,
            98 => Self::Ppmd,
            99 => Self::Aes,
            n => Self::Unknown(n),
        }
    }
}
impl From<Compression> for u16 {
    fn from(value: Compression) -> Self {
        match value {
            Compression::Stored => 0,
            Compression::Shrink => 1,
            Compression::ReduceFactor1 => 2,
            Compression::ReduceFactor2 => 3,
            Compression::ReduceFactor3 => 4,
            Compression::ReduceFactor4 => 5,
            Compression::Implode => 6,
            Compression::TokenizingCompression => 7,
            Compression::Deflated => 8,
            Compression::Deflate64 => 9,
            Compression::PkwareDataCompressionImploding => 10,
            Compression::Reserved11 => 11,
            Compression::Bzip2 => 12,
            Compression::Reserved13 => 13,
            Compression::Lzma => 14,
            Compression::Reserved15 => 15,
            Compression::CmpscCompression => 16,
            Compression::Reserved17 => 17,
            Compression::IbmTerseNew => 18,
            Compression::IbmLz77 => 19,
            Compression::DeprecatedZstd => 20,
            Compression::Zstd => 93,
            Compression::Mp3 => 94,
            Compression::Xz => 95,
            Compression::Jpeg => 96,
            Compression::WavPack => 97,
            Compression::Ppmd => 98,
            Compression::Aes => 99,
            Compression::Unknown(value) => value,
        }
    }
}
impl Compression {
    #[must_use]
    pub fn as_u16(self) -> u16 {
        u16::from(self)
    }
}
