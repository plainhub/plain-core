use core::fmt::Display;
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum AesVendorVersion {
    Ae1 = 0x0001,
    Ae2 = 0x0002,
}
impl AesVendorVersion {
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self as u16
    }
    #[must_use]
    pub const fn is_ae2_encrypted(&self) -> bool {
        matches!(self, AesVendorVersion::Ae2)
    }
}
impl TryFrom<u16> for AesVendorVersion {
    type Error = &'static str;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        let aes_vendor_version = match value {
            0x0001 => AesVendorVersion::Ae1,
            0x0002 => AesVendorVersion::Ae2,
            _ => return Err("Invalid AES vendor version"),
        };
        Ok(aes_vendor_version)
    }
}
impl From<AesVendorVersion> for u16 {
    fn from(value: AesVendorVersion) -> Self {
        value.as_u16()
    }
}
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
#[repr(u8)]
pub enum AesMode {
    Aes128 = 0x01,
    Aes192 = 0x02,
    Aes256 = 0x03,
}
impl AesMode {
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}
impl Display for AesMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Aes128 => write!(f, "AES-128"),
            Self::Aes192 => write!(f, "AES-192"),
            Self::Aes256 => write!(f, "AES-256"),
        }
    }
}
impl TryFrom<u8> for AesMode {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let mode = match value {
            0x01 => AesMode::Aes128,
            0x02 => AesMode::Aes192,
            0x03 => AesMode::Aes256,
            _ => return Err("Invalid AES encryption strength"),
        };
        Ok(mode)
    }
}
impl AesMode {
    #[must_use]
    pub const fn key_length(&self) -> usize {
        match self {
            Self::Aes128 => 16,
            Self::Aes192 => 24,
            Self::Aes256 => 32,
        }
    }
}
