use crate::zip::CompressionMethod;
use crate::zip::DateTime;
use crate::zip::HasZipMetadata;
use crate::zip::ZIP64_BYTES_THR;
use crate::zip::ZipReadOptions;
use crate::zip::format::data_descriptor::ZipDataDescriptor;
use crate::zip::format::ffi;
use crate::zip::format::flags::ZipFileFlags;
use crate::zip::format::system::System;
use crate::zip::read::ExtraField;
use crate::zip::read::readers::{ZipFileReader, ZipFileSeekReader};
use crate::zip::result::ZipResult;
use crate::zip::result::invalid;
use crate::zip::types::ZipFileData;
use crate::zip::write::options::SimpleFileOptions;
use core::mem::replace;
use std::borrow::Cow;
use std::ffi::OsStr;
use std::io::{self, Cursor, Read, Seek, SeekFrom, copy, sink};
use std::path::{Path, PathBuf};
#[derive(Debug)]
pub struct ZipFile<'a, R: Read + ?Sized> {
    pub(crate) file_name_raw: Cow<'a, [u8]>,
    pub(crate) data: Cow<'a, ZipFileData>,
    pub(crate) reader: ZipFileReader<'a, R>,
}
#[derive(Debug)]
pub struct ZipFileEntryWithDataDescriptor<'a> {
    pub(crate) file_name_raw: Cow<'a, [u8]>,
    pub(crate) data: Cow<'a, ZipFileData>,
    pub(crate) data_descriptor: Option<ZipDataDescriptor>,
}
#[derive(Debug)]
pub struct ZipFileEntry<'a> {
    pub(crate) file_name_raw: Cow<'a, [u8]>,
    pub(crate) data: Cow<'a, ZipFileData>,
}
pub struct ZipFileSeek<'a, R> {
    pub(crate) file_name_raw: Cow<'a, [u8]>,
    pub(crate) data: Cow<'a, ZipFileData>,
    pub(crate) reader: ZipFileSeekReader<'a, R>,
}
macro_rules! zip_file_methods {
    () => {
        pub fn name_raw(&self) -> &[u8] {
            &self.file_name_raw
        }
        pub fn name(&self) -> ZipResult<Cow<'_, str>> {
            self.get_metadata().name(&self.file_name_raw)
        }

        pub fn enclosed_name(&self) -> Option<PathBuf> {
            self.get_metadata().enclosed_name(&self.name().ok()?)
        }
        pub fn header_start(&self) -> u64 {
            self.get_metadata().header_start
        }
        pub fn central_header_start(&self) -> u64 {
            self.get_metadata().central_header_start
        }
        pub fn version_made_by(&self) -> (u8, u8) {
            (
                self.get_metadata().version_made_by / 10,
                self.get_metadata().version_made_by % 10,
            )
        }
        pub fn version_needed(&self) -> u16 {
            self.get_metadata().version_needed()
        }
        pub fn comment(&self) -> &str {
            &self.get_metadata().file_comment
        }
        pub fn system(&self) -> System {
            self.get_metadata().system
        }
        pub fn external_attributes(&self) -> u32 {
            self.get_metadata().external_attributes
        }
        pub fn flags(&self) -> ZipFileFlags {
            self.get_metadata().flags
        }
        pub fn compression(&self) -> CompressionMethod {
            self.get_metadata().compression_method
        }
        pub fn encrypted(&self) -> bool {
            self.data.is_encrypted()
        }
        pub fn compressed_size(&self) -> u64 {
            self.get_metadata().compressed_size
        }
        pub fn size(&self) -> u64 {
            self.get_metadata().uncompressed_size
        }
        pub fn last_modified(&self) -> Option<DateTime> {
            self.get_metadata().last_modified_time
        }
        pub fn is_dir(&self) -> bool {
            self.get_metadata().is_dir(&self.file_name_raw)
        }
        pub fn is_symlink(&self) -> bool {
            self.unix_mode()
                .is_some_and(|mode| mode & ffi::S_IFLNK == ffi::S_IFLNK)
        }
        pub fn is_file(&self) -> bool {
            !self.is_dir() && !self.is_symlink()
        }

        pub fn unix_mode(&self) -> Option<u32> {
            self.get_metadata().unix_mode()
        }
        pub fn crc32(&self) -> u32 {
            self.get_metadata().crc32
        }
    };
}
impl<'a> ZipFileEntryWithDataDescriptor<'a> {
    zip_file_methods!();
    pub fn data_descriptor(&self) -> &Option<ZipDataDescriptor> {
        &self.data_descriptor
    }
}
impl HasZipMetadata for ZipFileEntryWithDataDescriptor<'_> {
    fn get_metadata(&self) -> &ZipFileData {
        self.data.as_ref()
    }
}
impl<'a> ZipFileEntry<'a> {
    zip_file_methods!();
}
impl HasZipMetadata for ZipFileEntry<'_> {
    fn get_metadata(&self) -> &ZipFileData {
        self.data.as_ref()
    }
}
impl<'a, R: Read + ?Sized> ZipFile<'a, R> {
    zip_file_methods!();
    pub(crate) fn take_raw_reader(&mut self) -> io::Result<io::Take<&'a mut R>> {
        replace(&mut self.reader, ZipFileReader::NoReader).into_inner()
    }

    pub fn data_start(&self) -> Option<u64> {
        self.data.data_start.get().copied()
    }
    pub fn options(&self) -> SimpleFileOptions {
        let mut options = SimpleFileOptions::default()
            .large_file(self.compressed_size().max(self.size()) >= ZIP64_BYTES_THR)
            .compression_method(self.compression())
            .system(self.system())
            .external_attributes(self.external_attributes())
            .last_modified_time(
                self.last_modified()
                    .filter(DateTime::is_valid)
                    .unwrap_or_else(DateTime::default_for_write),
            );
        options.normalize();
        options
    }
}
impl<R: Read> ZipFile<'_, R> {
    pub fn extra_data_fields(&self) -> impl Iterator<Item = &ExtraField> {
        self.data.extra_fields.inner.iter()
    }
}
impl<R: Read + ?Sized> HasZipMetadata for ZipFile<'_, R> {
    fn get_metadata(&self) -> &ZipFileData {
        self.data.as_ref()
    }
}
impl<R: Read + ?Sized> Read for ZipFile<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.reader.read_to_end(buf)
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.reader.read_to_string(buf)
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.reader.read_exact(buf)
    }
}
impl<R: Read> Read for ZipFileSeek<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.reader {
            ZipFileSeekReader::Raw(r) => r.read(buf),
        }
    }
}
impl<R: Seek> Seek for ZipFileSeek<'_, R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match &mut self.reader {
            ZipFileSeekReader::Raw(r) => r.seek(pos),
        }
    }
}
impl<R> HasZipMetadata for ZipFileSeek<'_, R> {
    fn get_metadata(&self) -> &ZipFileData {
        self.data.as_ref()
    }
}
impl<'a, R> ZipFileSeek<'a, R> {
    zip_file_methods!();
}
impl<R: Read + ?Sized> Drop for ZipFile<'_, R> {
    fn drop(&mut self) {
        if let Cow::Owned(_) = self.data {
            if let Ok(mut inner) = self.take_raw_reader() {
                let _ = copy(&mut inner, &mut sink());
            }
        }
    }
}
