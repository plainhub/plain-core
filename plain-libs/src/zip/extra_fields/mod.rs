use core::fmt::Display;
mod custom_extra_field;
mod data_stream_alignment;
mod extended_timestamp;
mod extra_field;
mod ntfs;
mod zip64_extended_information;
mod zipinfo_utf8;
pub use crate::zip::format::extra_fields::EXTRA_FIELD_MAPPING;
pub use crate::zip::format::extra_fields::ExtraFieldId;
pub use custom_extra_field::CustomExtraField;
pub use data_stream_alignment::DataStreamAlignment;
pub use extended_timestamp::ExtendedTimestamp;
pub use extra_field::{ExtraField, ExtraFields};
pub use ntfs::Ntfs;
pub use zip64_extended_information::{Zip64ExtendedInformation, Zip64Sizes};
pub use zipinfo_utf8::UnicodeExtraField;
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum UsedExtraField {
    Zip64ExtendedInfo = ExtraFieldId::Zip64ExtendedInfo.as_u16(),
    Ntfs = ExtraFieldId::Ntfs.as_u16(),
    ExtendedTimestamp = ExtraFieldId::ExtendedTimestamp.as_u16(),
    UnicodeComment = ExtraFieldId::UnicodeComment.as_u16(),
    UnicodePath = ExtraFieldId::UnicodePath.as_u16(),
    AeXEncryption = ExtraFieldId::AeXEncryption.as_u16(),
    DataStreamAlignment = ExtraFieldId::DataStreamAlignment.as_u16(),
    AndroidZipAlignment = ExtraFieldId::AndroidZipAlignment.as_u16(),
}
impl UsedExtraField {
    pub const fn to_le_bytes(self) -> [u8; 2] {
        let field_u16 = self.as_u16();
        field_u16.to_le_bytes()
    }
    pub const fn as_u16(self) -> u16 {
        self as u16
    }
}
impl From<UsedExtraField> for u16 {
    fn from(value: UsedExtraField) -> Self {
        value.as_u16()
    }
}
impl Display for UsedExtraField {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:04X}", *self as u16)
    }
}
macro_rules ! extra_field_match { ($ x : expr , $ ($ variant : path) ,+ $ (,) ?) => { match $ x { $ (v if v == $ variant as u16 => Ok ($ variant) ,) + _ => Err (()) , } } ; }
impl TryFrom<u16> for UsedExtraField {
    type Error = ();
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        extra_field_match!(
            value,
            UsedExtraField::Zip64ExtendedInfo,
            UsedExtraField::Ntfs,
            UsedExtraField::ExtendedTimestamp,
            UsedExtraField::UnicodeComment,
            UsedExtraField::UnicodePath,
            UsedExtraField::DataStreamAlignment,
            UsedExtraField::AndroidZipAlignment,
            UsedExtraField::AeXEncryption,
        )
    }
}
