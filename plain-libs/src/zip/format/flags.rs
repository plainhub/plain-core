# [rustfmt :: skip] # [repr (u16)] # [derive (Debug , Copy , Clone , PartialEq , Eq)] # [non_exhaustive] pub enum ZipFlags { Encrypted = 0b0000_0000_0000_0001 , CompressionSetting = 0b0000_0000_0000_0010 , CompressionSetting2 = 0b0000_0000_0000_0100 , UsingDataDescriptor = 0b0000_0000_0000_1000 , ReservedEnhancedDeflating = 0b0000_0000_0001_0000 , CompressedPatchedData = 0b0000_0000_0010_0000 , StrongEncryption = 0b0000_0000_0100_0000 , LanguageEncoding = 0b0000_1000_0000_0000 , ReservedEnhancedCompression = 0b0001_0000_0000_0000 , Masked = 0b0010_0000_0000_0000 , ReservedAlternateStream = 0b0100_0000_0000_0000 , Reserved = 0b1000_0000_0000_0000 }
impl ZipFlags {
    #[inline]
    pub(crate) fn matching(flags: u16, matching_flag: Self) -> bool {
        flags & u16::from(matching_flag) != 0
    }
    #[inline]
    pub(crate) const fn as_u16(self) -> u16 {
        self as u16
    }
}
impl From<ZipFlags> for u16 {
    fn from(value: ZipFlags) -> u16 {
        value.as_u16()
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ZipFileFlags(pub(crate) u16);
impl ZipFileFlags {
    #[inline]
    #[must_use]
    pub fn matching(&self, matching_flag: ZipFlags) -> bool {
        ZipFlags::matching(self.0, matching_flag)
    }
    #[inline]
    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.matching(ZipFlags::Encrypted)
    }
    #[inline]
    #[must_use]
    pub fn is_using_data_descriptor(&self) -> bool {
        self.matching(ZipFlags::UsingDataDescriptor)
    }
    #[inline]
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }
    #[inline]
    pub(crate) fn set(&mut self, flag: ZipFlags, on: bool) {
        if on {
            self.0 |= flag.as_u16();
        } else {
            self.0 &= !flag.as_u16();
        }
    }
}
impl From<u16> for ZipFileFlags {
    fn from(value: u16) -> Self {
        ZipFileFlags(value)
    }
}
impl core::ops::BitOrAssign<u16> for ZipFileFlags {
    fn bitor_assign(&mut self, rhs: u16) {
        self.0 |= rhs;
    }
}
