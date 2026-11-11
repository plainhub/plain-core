use crate::zip::compression::CompressionMethod;
use crate::zip::cp437::FromCp437;
use crate::zip::datetime::DateTime;
use crate::zip::extra_fields::{ExtraField, ExtraFields};
use crate::zip::format::blocks::{CentralDirectoryEndInfo, DataAndPosition, ZipCentralEntryBlock};
use crate::zip::format::flags::{ZipFileFlags, ZipFlags};
use crate::zip::format::system::System;
use crate::zip::result::{ZipError, ZipResult, invalid};
use crate::zip::types::ZipFileData;
use crate::indexmap::IndexMap;
use std::io::{self, Read, Seek};
use std::sync::OnceLock;
mod config;
pub use config::{ArchiveOffset, Config};
pub(crate) mod magic_finder;
pub(crate) mod readers;
pub(crate) mod zipfile;
pub use zipfile::{ZipFile, ZipFileEntry, ZipFileEntryWithDataDescriptor, ZipFileSeek};
pub(crate) mod zip_archive;
pub use zip_archive::{ZipArchive, ZipArchiveMetadata};
#[derive(Debug)]
pub(crate) struct CentralDirectoryInfo {
    pub(crate) archive_offset: u64,
    pub(crate) directory_start: u64,
    pub(crate) directory_size: u64,
    pub(crate) number_of_files: usize,
    pub(crate) disk_number: u32,
    pub(crate) disk_with_central_directory: u32,
}
impl<'a> TryFrom<&'a CentralDirectoryEndInfo> for CentralDirectoryInfo {
    type Error = ZipError;
    fn try_from(value: &'a CentralDirectoryEndInfo) -> Result<Self, Self::Error> {
        let (
            relative_cd_offset,
            directory_size,
            number_of_files,
            disk_number,
            disk_with_central_directory,
        ) = match &value.eocd64 {
            Some(DataAndPosition { data: eocd64, .. }) => {
                if eocd64.number_of_files_on_this_disk > eocd64.number_of_files {
                    return Err(invalid!(
                        "ZIP64 footer indicates more files on this disk than in the whole archive"
                    ));
                }
                (
                    eocd64.central_directory_offset,
                    eocd64.central_directory_size,
                    eocd64.number_of_files as usize,
                    eocd64.disk_number,
                    eocd64.disk_with_central_directory,
                )
            }
            _ => (
                u64::from(value.eocd.data.central_directory_offset),
                u64::from(value.eocd.data.central_directory_size),
                value.eocd.data.number_of_files_on_this_disk as usize,
                u32::from(value.eocd.data.disk_number),
                u32::from(value.eocd.data.disk_with_central_directory),
            ),
        };
        let directory_start = relative_cd_offset
            .checked_add(value.archive_offset)
            .ok_or(invalid!("Invalid central directory size or offset"))?;
        Ok(Self {
            archive_offset: value.archive_offset,
            directory_start,
            directory_size,
            number_of_files,
            disk_number,
            disk_with_central_directory,
        })
    }
}
impl<R: Read + Seek> ZipArchive<R> {
    pub(crate) fn merge_contents<W: std::io::Write + Seek>(
        &mut self,
        mut w: W,
    ) -> ZipResult<IndexMap<Box<[u8]>, ZipFileData>> {
        if self.shared.files.is_empty() {
            return Ok(IndexMap::new());
        }
        let mut new_files = self.shared.files.clone();
        let first_new_file_header_start = w.stream_position()?;
        new_files.values_mut().try_for_each(|f| {
            f.header_start = f
                .header_start
                .checked_add(first_new_file_header_start)
                .ok_or(invalid!(
                    "new header start from merge would have been too large"
                ))?;
            f.central_header_start = 0;
            if let Some(old_data_start) = f.data_start.take() {
                let new_data_start = old_data_start
                    .checked_add(first_new_file_header_start)
                    .ok_or(invalid!(
                        "new data start from merge would have been too large"
                    ))?;
                f.data_start.get_or_init(|| new_data_start);
            }
            Ok::<_, ZipError>(())
        })?;
        self.reader.rewind()?;
        let length_to_read = self.shared.dir_start;
        let mut limited_raw = (&mut self.reader as &mut dyn Read).take(length_to_read);
        io::copy(&mut limited_raw, &mut w)?;
        Ok(new_files)
    }
}
#[inline]
fn read_variable_length_byte_field<R: Read>(reader: &mut R, len: usize) -> ZipResult<Vec<u8>> {
    if len == 0 {
        return Ok(Vec::new());
    }
    let mut data = vec![0; len];
    if let Err(e) = reader.read_exact(&mut data) {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            return Err(invalid!(
                "Variable-length field extends beyond file boundary"
            ));
        }
        return Err(e.into());
    }
    Ok(data)
}
fn central_header_to_zip_file_inner<R: Read>(
    reader: &mut R,
    archive_offset: u64,
    central_header_start: u64,
    block: ZipCentralEntryBlock,
) -> ZipResult<(ZipFileData, Vec<u8>)> {
    let ZipCentralEntryBlock {
        version_made_by,
        flags,
        compression_method,
        last_mod_time,
        last_mod_date,
        crc32,
        compressed_size,
        uncompressed_size,
        file_name_length,
        extra_field_length,
        file_comment_length,
        external_file_attributes,
        offset,
        ..
    } = block;
    let is_utf8 = ZipFlags::matching(flags, ZipFlags::LanguageEncoding);
    let mut file_name_raw = read_variable_length_byte_field(reader, file_name_length as usize)?;
    let extra_fields_raw = read_variable_length_byte_field(reader, extra_field_length as usize)?;
    let file_comment: Box<str> = if file_comment_length == 0 {
        Box::default()
    } else {
        let file_comment_raw =
            read_variable_length_byte_field(reader, file_comment_length as usize)?;
        if is_utf8 {
            String::from_utf8_lossy(&file_comment_raw).into()
        } else {
            file_comment_raw.from_cp437()?.into()
        }
    };
    let (version_made_by, system) = System::extract_bytes(version_made_by);
    let extra_fields = ExtraFields::parse(&extra_fields_raw, &block, false)?;
    let mut result = ZipFileData {
        system,
        version_made_by,
        compression_method: CompressionMethod::parse_from_u16(compression_method),
        last_modified_time: DateTime::try_from_msdos(last_mod_date, last_mod_time).ok(),
        crc32,
        compressed_size: compressed_size.into(),
        uncompressed_size: uncompressed_size.into(),
        flags: ZipFileFlags(flags),
        file_comment,
        header_start: offset.into(),
        central_header_start,
        data_start: OnceLock::new(),
        external_attributes: external_file_attributes,
        large_file: false,
        extra_fields,
    };
    result.apply_extra_fields(&mut file_name_raw)?;
    result.header_start = result
        .header_start
        .checked_add(archive_offset)
        .ok_or(invalid!("Archive header is too large"))?;
    Ok((result, file_name_raw))
}
pub trait HasZipMetadata {
    fn get_metadata(&self) -> &ZipFileData;
}
#[derive(Default)]
#[non_exhaustive]
pub struct ZipReadOptions<'a> {
    pub(crate) password: Option<&'a [u8]>,
    pub(crate) ignore_encryption_flag: bool,
    pub(crate) ignore_crc: bool,
    force_compressed_size: Option<u64>,
    force_uncompressed_size: Option<u64>,
    force_crc: Option<u32>,
    pub(crate) size_unknown: bool,
    pub(crate) enforce_header_consistency: bool,
}
impl<'a> ZipReadOptions<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn password(mut self, password: Option<&'a [u8]>) -> Self {
        self.password = password;
        self
    }
    #[must_use]
    pub fn ignore_encryption_flag(mut self, ignore: bool) -> Self {
        self.ignore_encryption_flag = ignore;
        self
    }

    #[must_use]
    pub fn enforce_header_consistency(mut self, enforce: bool) -> Self {
        self.enforce_header_consistency = enforce;
        self
    }
}
pub(crate) fn get_upath(extra_fields: &ExtraFields) -> (Option<&[u8]>, Option<u32>) {
    extra_fields
        .inner
        .iter()
        .rev()
        .find_map(|f| match f {
            ExtraField::UnicodePath(u) => Some((&*u.content, u.crc32)),
            _ => None,
        })
        .unzip()
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub(crate) struct HeaderDataDescriptorMetadata {
    pub(crate) crc32: u32,
    pub(crate) compressed_size: u64,
    pub(crate) uncompressed_size: u64,
    pub(crate) is_pending_data_descriptor: bool,
}
impl HeaderDataDescriptorMetadata {
    pub(crate) fn validate_matches(&self, central: &Self) -> ZipResult<()> {
        if self.crc32 != central.crc32 && (!self.is_pending_data_descriptor || self.crc32 != 0) {
            return Err(invalid!(
                "header inconsistency detected: CRC32 checksum mismatch (does not match central directory)"
            ));
        }
        if self.compressed_size != central.compressed_size
            && (!self.is_pending_data_descriptor || self.compressed_size != 0)
        {
            return Err(invalid!(
                "header inconsistency detected: compressed size mismatch (does not match central directory)"
            ));
        }
        if self.uncompressed_size != central.uncompressed_size
            && (!self.is_pending_data_descriptor || self.uncompressed_size != 0)
        {
            return Err(invalid!(
                "header inconsistency detected: uncompressed size mismatch (does not match central directory)"
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HeaderConsistencyMetadata<T: AsRef<[u8]>, U: AsRef<[u8]> = T> {
    pub(crate) name: T,
    pub(crate) upath: Option<U>,
    pub(crate) upath_crc32: Option<u32>,
    pub(crate) compression_method: u16,
    pub(crate) is_encrypted: bool,
    pub(crate) is_using_data_descriptor: bool,
    pub(crate) decoder_flags: u16,
    pub(crate) last_modified_time: Option<DateTime>,
    pub(crate) data: HeaderDataDescriptorMetadata,
}
impl HeaderConsistencyMetadata<&[u8]> {
    pub(crate) fn into_owned(self) -> HeaderConsistencyMetadata<Box<[u8]>> {
        HeaderConsistencyMetadata {
            name: self.name.into(),
            upath: self.upath.map(Into::into),
            upath_crc32: self.upath_crc32,
            compression_method: self.compression_method,
            is_encrypted: self.is_encrypted,
            is_using_data_descriptor: self.is_using_data_descriptor,
            decoder_flags: self.decoder_flags,
            last_modified_time: self.last_modified_time,
            data: self.data,
        }
    }
}
impl<T: AsRef<[u8]>, U: AsRef<[u8]>> HeaderConsistencyMetadata<T, U> {
    pub(crate) fn validate_identity_matches<T2: AsRef<[u8]>, U2: AsRef<[u8]>>(
        &self,
        central: &HeaderConsistencyMetadata<T2, U2>,
    ) -> ZipResult<()> {
        let local_effective = self
            .upath
            .as_ref()
            .map(AsRef::as_ref)
            .unwrap_or_else(|| self.name.as_ref());
        if self.upath.as_ref().map(AsRef::as_ref) != central.upath.as_ref().map(AsRef::as_ref)
            || self.upath_crc32 != central.upath_crc32
        {
            return Err(invalid!(
                "header inconsistency detected: UnicodePath (0x7075) extra field mismatch for '{}'",
                String::from_utf8_lossy(local_effective)
            ));
        }
        if self.name.as_ref() != central.name.as_ref() {
            return Err(invalid!(
                "header inconsistency detected: Local file header filename mismatch (does not match central directory)"
            ));
        }
        if self.compression_method != central.compression_method {
            return Err(invalid!(
                "header inconsistency detected: compression method mismatch (does not match central directory)"
            ));
        }
        if self.is_encrypted != central.is_encrypted {
            return Err(invalid!(
                "header inconsistency detected: encryption flag mismatch for '{}'",
                String::from_utf8_lossy(self.name.as_ref())
            ));
        }
        if self.is_using_data_descriptor != central.is_using_data_descriptor {
            return Err(invalid!(
                "header inconsistency detected: data descriptor flag mismatch for '{}'",
                String::from_utf8_lossy(self.name.as_ref())
            ));
        }
        if self.decoder_flags != central.decoder_flags {
            return Err(invalid!(
                "header inconsistency detected: compression flags mismatch for '{}'",
                String::from_utf8_lossy(self.name.as_ref())
            ));
        }
        if self.last_modified_time != central.last_modified_time {
            return Err(invalid!(
                "header inconsistency detected: last modified time mismatch for '{}'",
                String::from_utf8_lossy(self.name.as_ref())
            ));
        }
        Ok(())
    }
    pub(crate) fn validate_data_matches<T2: AsRef<[u8]>, U2: AsRef<[u8]>>(
        &self,
        central: &HeaderConsistencyMetadata<T2, U2>,
    ) -> ZipResult<()> {
        self.data.validate_matches(&central.data)
    }
    pub(crate) fn validate_matches<T2: AsRef<[u8]>, U2: AsRef<[u8]>>(
        &self,
        central: &HeaderConsistencyMetadata<T2, U2>,
    ) -> ZipResult<()> {
        self.validate_identity_matches(central)?;
        self.validate_data_matches(central)?;
        Ok(())
    }
}
