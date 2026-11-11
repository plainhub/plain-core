use crate::zip::datetime::DateTime;
use crate::zip::extra_fields::ExtraFields;
use crate::zip::format::aes::{AesMode, AesVendorVersion};
use crate::zip::format::blocks::{FixedSizeBlock, ZipLocalEntryBlock};
use crate::zip::format::data_descriptor::{
    Zip64DataDescriptorBlock, ZipDataDescriptor, ZipDataDescriptorBlock,
};
use crate::zip::format::ffi;
use crate::zip::format::flags::ZipFileFlags;
use crate::zip::format::flags::ZipFlags;
use crate::zip::format::functions::{
    get_flags, get_unix_mode, get_version_needed, is_dir, needs_language_encoding,
};
use crate::zip::format::magic::Magic;
use crate::zip::format::system::System;
use crate::zip::format::{ZIP64_BYTES_THR, ZIP64_BYTES_THR_U32};
use crate::zip::path::{enclosed_name, file_name_sanitized};
use crate::zip::read::readers::{SeekableTake, ZipFileReader, make_crypto_reader, make_reader};
use crate::zip::read::{HeaderConsistencyMetadata, HeaderDataDescriptorMetadata};
use crate::zip::result::{ZipError, ZipResult, invalid};
use crate::zip::write::FileOptionExtension;
use crate::zip::write::FileOptions;
use crate::zip::{CompressionMethod, ExtraField, ZipReadOptions};
use std::borrow::Cow;
use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom, Take};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
pub(crate) struct ZipRawValues {
    pub(crate) crc32: u32,
    pub(crate) compressed_size: u64,
    pub(crate) uncompressed_size: u64,
    pub(crate) language_encoding: Option<bool>,
}
pub use crate::zip::format::{DEFAULT_VERSION, MIN_VERSION};
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ZipFileData {
    pub system: System,
    pub version_made_by: u8,
    pub flags: ZipFileFlags,
    pub compression_method: CompressionMethod,
    pub last_modified_time: Option<DateTime>,
    pub crc32: u32,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub file_comment: Box<str>,
    pub header_start: u64,
    pub central_header_start: u64,
    pub data_start: OnceLock<u64>,
    pub external_attributes: u32,
    pub large_file: bool,
    pub extra_fields: ExtraFields,
}
impl ZipFileData {
    pub(crate) fn make_reader<'a, R: Read>(
        &self,
        limit_reader: Take<&'a mut R>,
        mut options: ZipReadOptions<'_>,
    ) -> ZipResult<ZipFileReader<'a, R>> {
        if options.ignore_encryption_flag {
            options.password = None;
        } else {
            match (options.password, self.is_encrypted()) {
                (None, true) => {
                    return Err(ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED));
                }
                (Some(_), false) => options.password = None,
                _ => {}
            }
        }
        let compression_method = self.compression_method;
        let uncompressed_size = self.uncompressed_size;
        let crc32 = if options.ignore_crc {
            None
        } else {
            Some(self.crc32)
        };
        let size_limit = (!options.size_unknown).then_some(uncompressed_size);
        let crypto_reader = make_crypto_reader(self, limit_reader, options.password)?;
        let aes_vendor_version = self.aes_settings().map(|aes| aes.1);
        make_reader(
            compression_method,
            uncompressed_size,
            size_limit,
            crc32,
            aes_vendor_version,
            crypto_reader,
            #[cfg(any())]
            self.flags.as_u16(),
        )
    }
    #[inline]
    pub(crate) fn name<'a>(&self, file_name_raw: &'a [u8]) -> ZipResult<Cow<'a, str>> {
        use crate::zip::cp437::FromCp437;
        Ok(
            if let Ok(file_name_utf8) = std::str::from_utf8(file_name_raw) {
                file_name_utf8.into()
            } else {
                file_name_raw.from_cp437().map_err(std::io::Error::other)?
            },
        )
    }
    pub fn aes_settings(&self) -> Option<(AesMode, AesVendorVersion)> {
        { None }
    }
    #[inline]
    pub fn is_encrypted(&self) -> bool {
        self.flags.is_encrypted()
    }
    #[inline]
    pub fn is_using_data_descriptor(&self) -> bool {
        self.flags.is_using_data_descriptor()
    }
    pub fn data_start(&self, reader: &mut (impl Read + Seek + ?Sized)) -> ZipResult<u64> {
        match self.data_start.get() {
            Some(data_start) => Ok(*data_start),
            None => {
                reader.seek(SeekFrom::Start(self.header_start))?;
                let block = ZipLocalEntryBlock::parse(reader)?;
                let variable_fields_len =
                    u64::from(block.file_name_length) + u64::from(block.extra_field_length);
                let data_start = self
                    .header_start
                    .checked_add((size_of::<Magic>() + size_of::<ZipLocalEntryBlock>()) as u64)
                    .and_then(|v| v.checked_add(variable_fields_len))
                    .ok_or(ZipError::InvalidArchive(Cow::Borrowed(
                        "Local header end offset overflows u64",
                    )))?;
                match self.data_start.set(data_start) {
                    Ok(()) => (),
                    Err(existing_value) => {
                        debug_assert_eq!(existing_value, data_start);
                    }
                }
                Ok(data_start)
            }
        }
    }
    pub(crate) fn find_content<'a, R: Read + Seek + ?Sized>(
        &self,
        reader: &'a mut R,
    ) -> ZipResult<Take<&'a mut R>> {
        let data_start = self.data_start(reader)?;
        self.check_data_end(data_start)?;
        reader.seek(SeekFrom::Start(data_start))?;
        Ok(reader.take(self.compressed_size))
    }
    pub(crate) fn find_content_seek<'a, R: Read + Seek + ?Sized>(
        &self,
        reader: &'a mut R,
    ) -> ZipResult<SeekableTake<'a, R>> {
        let data_start = self.data_start(reader)?;
        self.check_data_end(data_start)?;
        reader.seek(SeekFrom::Start(data_start))?;
        Ok(SeekableTake::new(reader, self.compressed_size)?)
    }
    fn check_data_end(&self, data_start: u64) -> ZipResult<u64> {
        data_start
            .checked_add(self.compressed_size)
            .ok_or(invalid!("File data end offset overflows u64"))
    }
    pub(crate) fn consistency_metadata<'a>(
        &'a self,
        name: &'a [u8],
        is_local: bool,
    ) -> HeaderConsistencyMetadata<&'a [u8]> {
        let is_using_data_descriptor = self.is_using_data_descriptor();
        let (upath, upath_crc32) = crate::zip::read::get_upath(&self.extra_fields);
        let decoder_flags = if self.compression_method == CompressionMethod::IMPLODE {
            self.flags.as_u16()
                & (ZipFlags::CompressionSetting.as_u16() | ZipFlags::CompressionSetting2.as_u16())
        } else {
            0
        };
        HeaderConsistencyMetadata {
            name,
            upath,
            upath_crc32,
            compression_method: self.compression_method.serialize_to_u16(),
            is_encrypted: self.is_encrypted(),
            is_using_data_descriptor,
            decoder_flags,
            last_modified_time: self.last_modified_time,
            data: HeaderDataDescriptorMetadata {
                crc32: self.crc32,
                compressed_size: self.compressed_size,
                uncompressed_size: self.uncompressed_size,
                is_pending_data_descriptor: is_local && is_using_data_descriptor,
            },
        }
    }
    pub(crate) fn validate_header_consistency<R: Read + Seek + ?Sized>(
        &self,
        reader: &mut R,
        expected_name: &[u8],
    ) -> ZipResult<()> {
        let current_pos = reader.stream_position()?;
        reader.seek(SeekFrom::Start(self.header_start))?;
        let block = match ZipLocalEntryBlock::parse(reader) {
            Ok(b) => b,
            Err(e) => {
                let _ = reader.seek(SeekFrom::Start(current_pos));
                return Err(e);
            }
        };
        let mut local_name = vec![0u8; block.file_name_length as usize];
        if let Err(e) = reader.read_exact(&mut local_name) {
            let _ = reader.seek(SeekFrom::Start(current_pos));
            return Err(e.into());
        }
        let mut local_extra = vec![0u8; block.extra_field_length as usize];
        if let Err(e) = reader.read_exact(&mut local_extra) {
            let _ = reader.seek(SeekFrom::Start(current_pos));
            return Err(e.into());
        }
        reader.seek(SeekFrom::Start(current_pos))?;
        let variable_fields_len =
            u64::from(block.file_name_length) + u64::from(block.extra_field_length);
        let data_start = self
            .header_start
            .checked_add((size_of::<Magic>() + size_of::<ZipLocalEntryBlock>()) as u64)
            .and_then(|v| v.checked_add(variable_fields_len))
            .ok_or(ZipError::InvalidArchive(Cow::Borrowed(
                "Local header end offset overflows u64",
            )))?;
        let _ = self.data_start.set(data_start);
        let local_extra_fields = ExtraFields::parse(&local_extra, &block, true)?;
        let mut local_data = Self::from_local_block(block, local_extra_fields)?;
        local_data.apply_extra_fields(&mut local_name)?;
        let local_meta = local_data.consistency_metadata(&local_name, true);
        let central_meta = self.consistency_metadata(expected_name, false);
        local_meta.validate_matches(&central_meta)
    }
    pub(crate) fn is_dir(&self, file_name: &[u8]) -> bool {
        is_dir(file_name)
    }
    pub(crate) fn file_name_sanitized(&self, file_name: &str) -> PathBuf {
        let no_null_filename = match file_name.find('\0') {
            Some(index) => &file_name[0..index],
            None => file_name,
        };
        file_name_sanitized(no_null_filename)
    }
    pub(crate) fn enclosed_name(&self, file_name: &str) -> Option<PathBuf> {
        if file_name.contains('\0') {
            return None;
        }
        let enclosed = enclosed_name(file_name)?;
        Some(enclosed)
    }
    pub(crate) fn extern_compression(&self) -> u16 {
        { self.compression_method.serialize_to_u16() }
    }
    pub(crate) const fn unix_mode(&self) -> Option<u32> {
        get_unix_mode(self.system, self.external_attributes)
    }
    pub fn version_needed(&self) -> u16 {
        get_version_needed(
            self.compression_method,
            self.aes_settings(),
            self.is_encrypted(),
            self.large_file,
            self.unix_mode(),
        )
    }
    pub(crate) fn initialize_local_block<T: FileOptionExtension>(
        file_name_raw: &[u8],
        options: &FileOptions<'_, '_, T>,
        raw_values: &ZipRawValues,
        header_start: u64,
        #[cfg_attr(not(any()), allow(unused_mut))] mut extra_fields: ExtraFields,
    ) -> Self {
        let compression_method = options.compression_method;
        match options.encrypt_with {
            _ => {}
        }
        let permissions = options
            .permissions
            .unwrap_or(FileOptions::DEFAULT_FILE_PERMISSION);
        let system = if (permissions & ffi::S_IFLNK) == ffi::S_IFLNK {
            System::Unix
        } else if let Some(system_option) = options.system {
            system_option
        } else if cfg!(windows) {
            System::Dos
        } else {
            System::Unix
        };
        let external_attributes = if let Some(external_attr) = options.external_attributes {
            external_attr
        } else {
            let mut external_attributes = permissions << 16;
            if system == System::Dos {
                if is_dir(file_name_raw) {
                    external_attributes |= 0x10;
                }
                if options
                    .permissions
                    .is_some_and(|permissions| permissions & 0o444 == 0)
                {
                    external_attributes |= 0x01;
                }
            }
            external_attributes
        };
        let mut flags = ZipFileFlags(0);
        if options.has_encryption() {
            flags |= ZipFlags::Encrypted.as_u16();
        }
        if std::str::from_utf8(file_name_raw).is_ok() && !file_name_raw.is_ascii() {
            flags |= ZipFlags::LanguageEncoding.as_u16();
        }
        let mut local_block = ZipFileData {
            system,
            version_made_by: DEFAULT_VERSION,
            flags,
            compression_method,
            last_modified_time: Some(options.last_modified_time),
            crc32: raw_values.crc32,
            compressed_size: raw_values.compressed_size,
            uncompressed_size: raw_values.uncompressed_size,
            file_comment: String::with_capacity(0).into_boxed_str(),
            header_start,
            data_start: OnceLock::new(),
            central_header_start: 0,
            external_attributes,
            large_file: options.large_file,
            extra_fields,
        };
        local_block.version_made_by = local_block.version_needed() as u8;
        local_block
    }
    pub(crate) fn from_local_block(
        block: ZipLocalEntryBlock,
        extra_fields: ExtraFields,
    ) -> ZipResult<Self> {
        let ZipLocalEntryBlock {
            version_made_by,
            flags,
            compression_method,
            last_mod_time,
            last_mod_date,
            crc32,
            compressed_size,
            uncompressed_size,
            ..
        } = block;
        let compression_method = CompressionMethod::parse_from_u16(compression_method);
        let (version_made_by, system) = System::extract_bytes(version_made_by);
        let data = ZipFileData {
            system,
            version_made_by,
            flags: ZipFileFlags(flags),
            compression_method,
            last_modified_time: DateTime::try_from_msdos(last_mod_date, last_mod_time).ok(),
            crc32,
            compressed_size: compressed_size.into(),
            uncompressed_size: uncompressed_size.into(),
            file_comment: String::with_capacity(0).into_boxed_str(),
            header_start: 0,
            data_start: OnceLock::new(),
            central_header_start: 0,
            external_attributes: 0,
            large_file: false,
            extra_fields,
        };
        Ok(data)
    }
    pub(crate) fn flags(&self, file_name_raw: &[u8]) -> u16 {
        let mut flags = self.flags;
        if !self.file_comment.is_ascii() && core::str::from_utf8(file_name_raw).is_ok() {
            flags.set(ZipFlags::LanguageEncoding, true);
        }
        get_flags(flags)
    }
    pub(crate) fn drop_unicode_path_renaming(&mut self, file_name_raw: &[u8]) {
        const UNICODE_PATH: u16 =
            crate::zip::format::extra_fields::ExtraFieldId::UnicodePath.as_u16();
        let crc = crc32fast::hash(file_name_raw);
        self.extra_fields.inner.retain(|field| match field {
            ExtraField::UnicodePath(unicode) => {
                unicode.crc32 != crc || *unicode.content == *file_name_raw
            }
            ExtraField::Custom(custom) if custom.header_id == UNICODE_PATH => {
                custom.data.len() < 5
                    || custom.data[1..5] != crc.to_le_bytes()
                    || custom.data[5..] == *file_name_raw
            }
            _ => true,
        });
    }
    pub(crate) fn set_language_encoding_for(&mut self, file_name_raw: &[u8]) {
        let on = needs_language_encoding(file_name_raw, &self.file_comment);
        self.flags.set(ZipFlags::LanguageEncoding, on);
    }
    pub(crate) fn clamp_size_field(&self, field: u64) -> Result<u32, std::io::Error> {
        if self.large_file {
            Ok(ZIP64_BYTES_THR_U32)
        } else {
            let size: u32 = field.try_into().map_err(|_| {
                std::io::Error::other(format!(
                    "File size {field} exceeds maximum size for non-ZIP64 files"
                ))
            })?;
            Ok(size.min(ZIP64_BYTES_THR_U32 - 1))
        }
    }
    pub(crate) fn write_data_descriptor<W: std::io::Write>(
        &self,
        writer: &mut W,
        auto_large_file: bool,
    ) -> Result<(), ZipError> {
        if self.large_file {
            self.zip64_data_descriptor_block().write(writer)?;
            return Ok(());
        }
        if self.compressed_size >= ZIP64_BYTES_THR || self.uncompressed_size >= ZIP64_BYTES_THR {
            if auto_large_file {
                self.zip64_data_descriptor_block().write(writer)?;
                return Ok(());
            }
            return Err(ZipError::Io(std::io::Error::other(
                "Large file option has not been set - use .large_file(true) in options",
            )));
        }
        self.data_descriptor_block().write(writer)?;
        Ok(())
    }
    pub(crate) fn data_descriptor_block(&self) -> ZipDataDescriptorBlock {
        ZipDataDescriptorBlock {
            crc32: self.crc32,
            compressed_size: self.compressed_size as u32,
            uncompressed_size: self.uncompressed_size as u32,
        }
    }
    pub(crate) fn zip64_data_descriptor_block(&self) -> Zip64DataDescriptorBlock {
        Zip64DataDescriptorBlock {
            crc32: self.crc32,
            compressed_size: self.compressed_size,
            uncompressed_size: self.uncompressed_size,
        }
    }
    pub(crate) fn get_data_descriptor<R: Read + Seek>(
        &self,
        mut reader: &mut R,
    ) -> ZipResult<Option<ZipDataDescriptor>> {
        if !self.is_using_data_descriptor() {
            return Ok(None);
        }
        let current = reader.stream_position()?;
        match self.data_start(&mut reader) {
            Ok(data_start) => {
                let is_zip64 = self.compressed_size >= ZIP64_BYTES_THR
                    || self.uncompressed_size >= ZIP64_BYTES_THR
                    || self
                        .extra_fields
                        .inner
                        .iter()
                        .any(|e| matches!(e, ExtraField::Zip64ExtendedInformation(_)));
                let desc_start = data_start.checked_add(self.compressed_size).ok_or(
                    ZipError::InvalidArchive(Cow::Borrowed("Data descriptor offset overflows u64")),
                )?;
                reader.seek(SeekFrom::Start(desc_start))?;
                let res = if is_zip64 {
                    let mut buff: [u8; 24] = [0; Zip64DataDescriptorBlock::SIZE];
                    if let Err(err) = reader.read_exact(&mut buff) {
                        Err(err)
                    } else {
                        Ok(ZipDataDescriptor::Zip64DataDescriptorBlock(
                            Zip64DataDescriptorBlock::parse(&buff)
                                .map_err(|s| ZipError::InvalidArchive(Cow::Borrowed(s)))?,
                        ))
                    }
                } else {
                    let mut buff: [u8; 16] = [0; ZipDataDescriptorBlock::SIZE];
                    if let Err(err) = reader.read_exact(&mut buff) {
                        Err(err)
                    } else {
                        Ok(ZipDataDescriptor::ZipDataDescriptorBlock(
                            ZipDataDescriptorBlock::parse(&buff)
                                .map_err(|s| ZipError::InvalidArchive(Cow::Borrowed(s)))?,
                        ))
                    }
                };
                match res {
                    Ok(data_desc) => {
                        reader.seek(SeekFrom::Start(current))?;
                        Ok(Some(data_desc))
                    }
                    Err(err) => {
                        reader.seek(SeekFrom::Start(current))?;
                        Err(err.into())
                    }
                }
            }
            Err(_err) => Ok(None),
        }
    }
}
