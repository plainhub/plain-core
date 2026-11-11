use crate::zip::CompressionMethod;
use crate::zip::datetime::DateTime;
use crate::zip::extra_fields::CustomExtraField;
use crate::zip::extra_fields::ExtendedTimestamp;
use crate::zip::format::extra_fields::ExtraFieldId;
use crate::zip::format::ffi;
use crate::zip::format::system::System;
use crate::zip::result::{ZipResult, invalid};
use crate::zip::zipcrypto::ZipCryptoKeys;
use core::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum EncryptWith<'k> {
    ZipCrypto(ZipCryptoKeys, PhantomData<&'k ()>),
}
#[non_exhaustive]
#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub struct FileOptions<'k, 'n, T: sealed::FileOptionExtension> {
    pub(crate) compression_method: CompressionMethod,
    pub(crate) compression_level: Option<i64>,
    pub(crate) external_attributes: Option<u32>,
    pub(crate) last_modified_time: DateTime,
    pub(crate) permissions: Option<u32>,
    pub(crate) large_file: bool,
    pub(crate) encrypt_with: Option<EncryptWith<'k>>,
    pub(crate) extended_options: T,
    pub(crate) alignment: u16,
    pub(crate) alignment_header_id: u16,
    pub(crate) system: Option<System>,
    pub(crate) name: Option<&'n [u8]>,
}
pub type SimpleFileOptions = FileOptions<'static, 'static, ()>;
impl FileOptions<'static, 'static, ()> {
    pub(crate) const DEFAULT_FILE_PERMISSION: u32 = 0o644 | ffi::S_IFREG;
    pub(crate) const DEFAULT_DIR_PERMISSIONS: u32 = 0o755 | ffi::S_IFDIR;
    pub(crate) const DEFAULT_SYMLINK_PERMISSIONS: u32 = 0o777 | ffi::S_IFLNK;
}
pub(crate) mod sealed {
    use super::ExtendedFileOptions;
    use crate::zip::extra_fields::CustomExtraField;
    use std::sync::Arc;
    pub trait Sealed {}
    pub trait FileOptionExtension: Default + Sealed {
        fn extra_fields(&self) -> Option<&Arc<Vec<CustomExtraField>>>;
        fn file_comment(&self) -> Option<&str>;
        fn take_file_comment(&mut self) -> Option<Box<str>>;
        fn validate_extra_fields(&self) -> bool {
            false
        }
    }
    impl Sealed for () {}
    impl FileOptionExtension for () {
        fn extra_fields(&self) -> Option<&Arc<Vec<CustomExtraField>>> {
            None
        }
        fn file_comment(&self) -> Option<&str> {
            None
        }
        fn take_file_comment(&mut self) -> Option<Box<str>> {
            None
        }
    }
    impl Sealed for ExtendedFileOptions {}
    impl FileOptionExtension for ExtendedFileOptions {
        fn extra_fields(&self) -> Option<&Arc<Vec<CustomExtraField>>> {
            Some(&self.extra_fields)
        }
        fn file_comment(&self) -> Option<&str> {
            self.file_comment.as_ref().map(Box::as_ref)
        }
        fn take_file_comment(&mut self) -> Option<Box<str>> {
            self.file_comment.take()
        }
        fn validate_extra_fields(&self) -> bool {
            self.validate_extra_fields
        }
    }
}
pub type FullFileOptions<'k, 'n> = FileOptions<'k, 'n, ExtendedFileOptions>;
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
#[derive(Clone, Default, Eq, PartialEq)]
pub struct ExtendedFileOptions {
    pub(crate) extra_fields: Arc<Vec<CustomExtraField>>,
    pub(crate) file_comment: Option<Box<str>>,
    pub(crate) validate_extra_fields: bool,
}
impl ExtendedFileOptions {
    pub(crate) fn extra_field_parses(header_id: u16, data: &[u8], central_only: bool) -> bool {
        use crate::zip::extra_fields::{ExtraField, UsedExtraField};
        use crate::zip::format::blocks::{Pod, ZipLocalEntryBlock};
        use std::io::Cursor;
        let Ok(size) = u16::try_from(data.len()) else {
            return false;
        };
        let mut serialized = Vec::with_capacity(data.len() + 4);
        serialized.extend_from_slice(&header_id.to_le_bytes());
        serialized.extend_from_slice(&size.to_le_bytes());
        serialized.extend_from_slice(data);
        let block = ZipLocalEntryBlock::zeroed();
        if ExtraField::parse(&mut Cursor::new(&serialized), &block, false).is_err() {
            return false;
        }
        central_only
            || header_id != UsedExtraField::ExtendedTimestamp.as_u16()
            || ExtendedTimestamp::try_from_reader(&mut Cursor::new(data), size, true).is_ok()
    }
    pub fn add_extra_field<D: AsRef<[u8]>>(
        &mut self,
        header_id: u16,
        data: D,
        central_only: bool,
    ) -> ZipResult<()> {
        let data = data.as_ref();
        let len = data.len() + 4;
        let extra_fields_len: usize = self.extra_fields.iter().map(|x| x.full_size(false)).sum();
        if extra_fields_len + len > u16::MAX as usize {
            Err(invalid!("Extra data field would be longer than allowed"))
        } else if self.validate_extra_fields
            && !Self::extra_field_parses(header_id, data, central_only)
        {
            Err(invalid!(
                "Extra field {header_id:#06x} would not be readable: malformed data"
            ))
        } else {
            Arc::make_mut(&mut self.extra_fields).push(CustomExtraField::new(
                central_only,
                header_id,
                data.to_vec().into_boxed_slice(),
            ));
            Ok(())
        }
    }
}
impl Debug for ExtendedFileOptions {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        f.debug_struct("ExtendedFileOptions")
            .field("extra_fields", &self.extra_fields)
            .field("file_comment", &self.file_comment)
            .field("validate_extra_fields", &self.validate_extra_fields)
            .finish()
    }
}
impl<'k, 'n, T: sealed::FileOptionExtension> FileOptions<'k, 'n, T> {
    pub(crate) fn normalize(&mut self) {
        if !self.last_modified_time.is_valid() {
            self.last_modified_time = FileOptions::<T>::default().last_modified_time;
        }
        if let Some(perms) = &mut self.permissions
            && (*perms & !0o777 == 0)
        {
            *perms |= ffi::S_IFREG;
        }
    }
    pub const fn has_encryption(&self) -> bool {
        self.encrypt_with.is_some()
    }
    #[must_use]
    pub const fn compression_method(mut self, method: CompressionMethod) -> Self {
        self.compression_method = method;
        self
    }
    #[must_use]
    pub const fn system(mut self, system: System) -> Self {
        self.system = Some(system);
        self
    }
    #[must_use]
    pub const fn compression_level(mut self, level: Option<i64>) -> Self {
        self.compression_level = level;
        self
    }
    #[must_use]
    pub const fn last_modified_time(mut self, mod_time: DateTime) -> Self {
        self.last_modified_time = mod_time;
        self
    }
    #[must_use]
    pub const fn unix_permissions(mut self, mode: u32) -> Self {
        self.permissions = Some(mode & 0o777);
        self
    }
    #[must_use]
    pub const fn external_attributes(mut self, external_perms: u32) -> Self {
        self.external_attributes = Some(external_perms);
        self
    }
    #[must_use]
    pub const fn large_file(mut self, large: bool) -> Self {
        self.large_file = large;
        self
    }

    #[must_use]
    pub const fn alignment(&self) -> u16 {
        self.alignment
    }
    #[must_use]
    pub const fn alignment_header_id(&self) -> u16 {
        self.alignment_header_id
    }
}
impl FileOptions<'_, '_, ExtendedFileOptions> {
    #[must_use]
    pub fn validate_extra_fields(mut self, validate: bool) -> Self {
        self.extended_options.validate_extra_fields = validate;
        self
    }
    #[must_use]
    pub fn with_file_comment<S: Into<Box<str>>>(mut self, comment: S) -> Self {
        self.extended_options.file_comment = Some(comment.into());
        self
    }
    pub fn add_extra_field<D: AsRef<[u8]>>(
        &mut self,
        header_id: u16,
        data: D,
        central_only: bool,
    ) -> ZipResult<()> {
        self.extended_options
            .add_extra_field(header_id, data, central_only)
    }
}
impl FileOptions<'static, 'static, ()> {
    pub const DEFAULT: Self = Self {
        compression_method: CompressionMethod::DEFAULT,
        compression_level: None,
        last_modified_time: DateTime::DEFAULT,
        large_file: false,
        permissions: None,
        encrypt_with: None,
        extended_options: (),
        alignment: 1,
        alignment_header_id: ExtraFieldId::DataStreamAlignment.as_u16(),
        system: None,
        name: None,
        external_attributes: None,
    };
}
impl<'k, 'n> FileOptions<'k, 'n, ()> {
    #[must_use]
    pub fn into_full_options(self) -> FullFileOptions<'k, 'n> {
        FileOptions {
            compression_method: self.compression_method,
            compression_level: self.compression_level,
            last_modified_time: self.last_modified_time,
            permissions: self.permissions,
            large_file: self.large_file,
            encrypt_with: self.encrypt_with,
            extended_options: ExtendedFileOptions::default(),
            alignment: self.alignment,
            alignment_header_id: self.alignment_header_id,
            system: self.system,
            name: self.name,
            external_attributes: self.external_attributes,
        }
    }
}
impl<T: sealed::FileOptionExtension> Default for FileOptions<'_, '_, T> {
    fn default() -> Self {
        Self {
            compression_method: CompressionMethod::default(),
            compression_level: None,
            last_modified_time: DateTime::default_for_write(),
            permissions: None,
            large_file: false,
            encrypt_with: None,
            extended_options: T::default(),
            alignment: 1,
            alignment_header_id: ExtraFieldId::DataStreamAlignment.as_u16(),
            system: None,
            name: None,
            external_attributes: None,
        }
    }
}
