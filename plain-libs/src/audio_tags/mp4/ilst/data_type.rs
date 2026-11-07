use crate::audio_tags::picture::MimeType;

// OLD LINKS:
// * https://developer.apple.com/library/archive/documentation/QuickTime/QTFF/Metadata/Metadata.html#//apple_ref/doc/uid/TP40000939-CH1-SW35
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataType {
    Reserved = 0,

    Utf8 = 1,

    Utf16 = 2,

    SJis = 3,

    Utf8Sort = 4,

    Utf16Sort = 5,

    Gif = 12,

    Jpeg = 13,

    Png = 14,

    BeSignedInteger = 21,

    BeUnsignedInteger = 22,

    BeFloat32 = 23,

    BeFloat64 = 24,

    Bmp = 27,

    QuicktimeMetadata = 28,

    Signed8BitInteger = 65,

    Be16BitSignedInteger = 66,

    Be32BitSignedInteger = 67,

    BePointF32 = 70,

    BeDimensionsF32 = 71,

    BeRectF32 = 72,

    Be64BitSignedInteger = 74,

    Unsigned8BitInteger = 75,

    Be16BitUnsignedInteger = 76,

    Be32BitUnsignedInteger = 77,

    Be64BitUnsignedInteger = 78,

    AffineTransformF64 = 79,

    Other(u32),
}

impl From<u32> for DataType {
    fn from(value: u32) -> Self {
        match value {
            0 => DataType::Reserved,
            1 => DataType::Utf8,
            2 => DataType::Utf16,
            3 => DataType::SJis,
            4 => DataType::Utf8Sort,
            5 => DataType::Utf16Sort,
            12 => DataType::Gif,
            13 => DataType::Jpeg,
            14 => DataType::Png,
            21 => DataType::BeSignedInteger,
            22 => DataType::BeUnsignedInteger,
            23 => DataType::BeFloat32,
            24 => DataType::BeFloat64,
            27 => DataType::Bmp,
            28 => DataType::QuicktimeMetadata,
            65 => DataType::Signed8BitInteger,
            66 => DataType::Be16BitSignedInteger,
            67 => DataType::Be32BitSignedInteger,
            70 => DataType::BePointF32,
            71 => DataType::BeDimensionsF32,
            72 => DataType::BeRectF32,
            74 => DataType::Be64BitSignedInteger,
            75 => DataType::Unsigned8BitInteger,
            76 => DataType::Be16BitUnsignedInteger,
            77 => DataType::Be32BitUnsignedInteger,
            78 => DataType::Be64BitUnsignedInteger,
            79 => DataType::AffineTransformF64,
            other => DataType::Other(other),
        }
    }
}

impl From<DataType> for u32 {
    fn from(value: DataType) -> Self {
        match value {
            DataType::Reserved => 0,
            DataType::Utf8 => 1,
            DataType::Utf16 => 2,
            DataType::SJis => 3,
            DataType::Utf8Sort => 4,
            DataType::Utf16Sort => 5,
            DataType::Gif => 12,
            DataType::Jpeg => 13,
            DataType::Png => 14,
            DataType::BeSignedInteger => 21,
            DataType::BeUnsignedInteger => 22,
            DataType::BeFloat32 => 23,
            DataType::BeFloat64 => 24,
            DataType::Bmp => 27,
            DataType::QuicktimeMetadata => 28,
            DataType::Signed8BitInteger => 65,
            DataType::Be16BitSignedInteger => 66,
            DataType::Be32BitSignedInteger => 67,
            DataType::BePointF32 => 70,
            DataType::BeDimensionsF32 => 71,
            DataType::BeRectF32 => 72,
            DataType::Be64BitSignedInteger => 74,
            DataType::Unsigned8BitInteger => 75,
            DataType::Be16BitUnsignedInteger => 76,
            DataType::Be32BitUnsignedInteger => 77,
            DataType::Be64BitUnsignedInteger => 78,
            DataType::AffineTransformF64 => 79,
            DataType::Other(other) => other,
        }
    }
}

impl From<MimeType> for DataType {
    fn from(value: MimeType) -> Self {
        DataType::from(&value)
    }
}

impl From<&MimeType> for DataType {
    fn from(value: &MimeType) -> Self {
        match value {
            MimeType::Gif => DataType::Gif,
            MimeType::Jpeg => DataType::Jpeg,
            MimeType::Png => DataType::Png,
            MimeType::Bmp => DataType::Bmp,
            _ => DataType::Reserved,
        }
    }
}

impl DataType {
    pub const MAX: u32 = 16_777_215;
}
