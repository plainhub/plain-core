#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Magic(u32);
impl Magic {
    pub const LOCAL_FILE_HEADER_SIGNATURE: Self = Self::literal(0x0403_4b50);
    pub const CENTRAL_DIRECTORY_HEADER_SIGNATURE: Self = Self::literal(0x0201_4b50);
    pub const CENTRAL_DIRECTORY_END_SIGNATURE: Self = Self::literal(0x0605_4b50);
    pub const CENTRAL_DIRECTORY_DIGITAL_SIGNATURE: Self = Self::literal(0x0505_4b50);
    pub const ZIP64_CENTRAL_DIRECTORY_END_SIGNATURE: Self = Self::literal(0x0606_4b50);
    pub const ZIP64_CENTRAL_DIRECTORY_END_LOCATOR_SIGNATURE: Self = Self::literal(0x0706_4b50);
    pub const DATA_DESCRIPTOR_SIGNATURE: Self = Self::literal(0x0807_4b50);
    #[must_use]
    pub const fn literal(x: u32) -> Self {
        Self(x)
    }
    #[inline(always)]
    #[must_use]
    pub const fn from_le_bytes(bytes: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(bytes))
    }
    #[inline(always)]
    #[must_use]
    pub const fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
    #[inline(always)]
    #[must_use]
    pub fn from_le(self) -> Self {
        Self(u32::from_le(self.0))
    }
    #[inline(always)]
    #[must_use]
    pub fn to_le(self) -> Self {
        Self(u32::to_le(self.0))
    }
}
