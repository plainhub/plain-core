use crate::zip::CompressionMethod;
use crate::zip::format::aes::{AesMode, AesVendorVersion};
use crate::zip::format::flags::{ZipFileFlags, ZipFlags};
use crate::zip::format::system::System;
use crate::zip::format::{DEFAULT_VERSION, MIN_VERSION, ffi};
#[inline]
pub(crate) fn is_dir(filename: &[u8]) -> bool {
    matches!(filename.last(), Some(b'/') | Some(b'\\'))
}
const fn clear_specials_bits(unix_perms: u32) -> u32 {
    unix_perms & !0o7000
}
#[inline]
pub(crate) const fn get_unix_mode(system: System, external_attributes: u32) -> Option<u32> {
    if external_attributes == 0 {
        return None;
    }
    let unix_mode = external_attributes >> 16;
    match system {
        System::Unix => Some(clear_specials_bits(unix_mode)),
        System::Dos => {
            let dos_attributes = (external_attributes & 0xFF) as u8;
            let mut mode = if (dos_attributes & 0x10) != 0 {
                ffi::S_IFDIR | 0o0775
            } else {
                ffi::S_IFREG | 0o0664
            };
            if (dos_attributes & 0x01) != 0 {
                mode &= !0o222;
            }
            Some(mode)
        }
        _ => {
            if unix_mode != 0 {
                return Some(clear_specials_bits(unix_mode));
            }
            None
        }
    }
}
#[inline]
pub(crate) fn get_version_needed(
    compression_method: CompressionMethod,
    aes_settings: Option<(AesMode, AesVendorVersion)>,
    is_encrypted: bool,
    large_file: bool,
    unix_mode: Option<u32>,
) -> u16 {
    let compression_version: u16 = match compression_method {
        CompressionMethod::Stored => u16::from(MIN_VERSION),
        CompressionMethod::Deflated => 20,
        _ => u16::from(DEFAULT_VERSION),
    };
    let crypto_version: u16 = if aes_settings.is_some() {
        51
    } else if is_encrypted {
        20
    } else {
        10
    };
    let misc_feature_version: u16 = if large_file {
        45
    } else if unix_mode.is_some_and(|mode| mode & ffi::S_IFDIR == ffi::S_IFDIR) {
        20
    } else {
        10
    };
    compression_version
        .max(crypto_version)
        .max(misc_feature_version)
}
pub(crate) fn needs_language_encoding(file_name_raw: &[u8], file_comment: &str) -> bool {
    let utf8_name = !file_name_raw.is_ascii() && core::str::from_utf8(file_name_raw).is_ok();
    utf8_name || !file_comment.is_ascii()
}
pub(crate) fn get_flags(flags: ZipFileFlags) -> u16 {
    let utf8_bit: u16 = if flags.matching(ZipFlags::LanguageEncoding) {
        ZipFlags::LanguageEncoding.as_u16()
    } else {
        0
    };
    let using_data_descriptor_bit = if flags.is_using_data_descriptor() {
        ZipFlags::UsingDataDescriptor.as_u16()
    } else {
        0
    };
    let encrypted_bit: u16 = if flags.is_encrypted() { 1u16 << 0 } else { 0 };
    utf8_bit | using_data_descriptor_bit | encrypted_bit
}
