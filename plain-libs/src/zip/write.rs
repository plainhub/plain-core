use crate::zip::ExtraField;
use crate::zip::compression::CompressionMethod;
use crate::zip::datetime::DateTime;
use crate::zip::extra_fields::DataStreamAlignment;
use crate::zip::extra_fields::ExtraFields;
use crate::zip::extra_fields::Zip64ExtendedInformation;
use crate::zip::format::blocks::{
    FixedSizeBlock, Zip32CDEBlock, Zip32CentralDirectoryEnd, Zip64CentralDirectoryEnd,
    Zip64CentralDirectoryEndLocator, ZipCentralEntryBlock, ZipLocalEntryBlock,
};
use crate::zip::format::ffi;
use crate::zip::format::flags::ZipFlags;
use crate::zip::format::magic::Magic;
use crate::zip::format::{ZIP64_BYTES_THR, ZIP64_BYTES_THR_U32, ZIP64_ENTRY_THR};
use crate::zip::read::{Config, ZipArchive, ZipFile};
use crate::zip::result::{ZipError, ZipResult, invalid};
use crate::zip::types::{MIN_VERSION, ZipFileData, ZipRawValues};
use crate::zip::write::options::EncryptWith;
use core::default::Default;
use core::fmt::{Debug, Formatter};
use core::mem::{self, offset_of, size_of};
use core::str::{Utf8Error, from_utf8};
use crc32fast::Hasher;
use indexmap::IndexMap;
use std::io::Cursor;
use std::io::ErrorKind;
use std::io::SeekFrom;
use std::io::{self, Read, Seek, Write};
use std::path::Path;
pub(crate) mod options;
pub use crate::zip::write::options::{
    ExtendedFileOptions, FileOptions, FullFileOptions, SimpleFileOptions,
};
enum MaybeEncrypted<W> {
    Unencrypted(W),
    ZipCrypto(crate::zip::zipcrypto::ZipCryptoWriter<W>),
    Closed { bytes_written: usize },
}
impl<W: Write> MaybeEncrypted<W> {
    fn get_ref(&self) -> Option<&W> {
        match self {
            MaybeEncrypted::Unencrypted(w) => Some(w),
            MaybeEncrypted::ZipCrypto(w) => Some(w.get_ref()),
            MaybeEncrypted::Closed { .. } => None,
        }
    }
    unsafe fn get_mut(&mut self) -> Option<&mut W> {
        match self {
            MaybeEncrypted::Unencrypted(w) => Some(w),
            MaybeEncrypted::ZipCrypto(w) => Some(w.get_mut()),
            MaybeEncrypted::Closed { .. } => None,
        }
    }
}
impl<W> Debug for MaybeEncrypted<W> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            MaybeEncrypted::Unencrypted(_) => "Unencrypted",
            MaybeEncrypted::ZipCrypto(_) => "ZipCrypto",
            MaybeEncrypted::Closed { .. } => "Closed",
        })
    }
}
impl<W: Write> Write for MaybeEncrypted<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            MaybeEncrypted::Unencrypted(w) => w.write(buf),
            MaybeEncrypted::ZipCrypto(w) => w.write(buf),
            MaybeEncrypted::Closed { bytes_written } => {
                *bytes_written = (*bytes_written).saturating_add(buf.len());
                if *bytes_written > 2 {
                    Err(io::Error::new(
                        ErrorKind::FileTooLarge,
                        "More than 2 bytes unexpectedly written to Deflate encoder's closed internal writer",
                    ))
                } else {
                    Ok(buf.len())
                }
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            MaybeEncrypted::Unencrypted(w) => w.flush(),
            MaybeEncrypted::ZipCrypto(w) => w.flush(),
            MaybeEncrypted::Closed { .. } => Ok(()),
        }
    }
}
enum GenericZipWriter<W: Write + Seek> {
    Closed,
    Storer(MaybeEncrypted<W>),
    Deflater(flate2::write::DeflateEncoder<MaybeEncrypted<W>>),
}
impl<W: Write + Seek> Debug for GenericZipWriter<W> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Closed => f.write_str("Closed"),
            Self::Storer(w) => f.write_fmt(format_args!("Storer({w:?})")),
            Self::Deflater(w) => f.write_fmt(format_args!("Deflater({:?})", w.get_ref())),
        }
    }
}
pub(crate) mod zip_writer {
    use super::DeflateCache;
    use crate::zip::{
        types::ZipFileData,
        write::{GenericZipWriter, ZipWriterStats},
    };
    use core::fmt::{Debug, Formatter};
    use indexmap::IndexMap;
    use std::io::{Seek, Write};
    pub struct ZipWriter<W: Write + Seek> {
        pub(super) inner: GenericZipWriter<W>,
        pub(super) files: IndexMap<Box<[u8]>, ZipFileData>,
        pub(super) stats: ZipWriterStats,
        pub(super) writing_to_file: bool,
        pub(super) writing_raw: bool,
        pub(super) comment: Box<[u8]>,
        pub(super) zip64_extensible_data_sector: Option<Box<[u8]>>,
        pub(super) flush_on_finish_file: bool,
        pub(super) seek_possible: bool,
        pub(crate) auto_large_file: bool,
        pub(super) deflate_cache: DeflateCache<W>,
    }
    impl<W: Write + Seek> Debug for ZipWriter<W> {
        fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
            f . write_fmt (format_args ! ("ZipWriter {{files: {:?}, stats: {:?}, writing_to_file: {}, writing_raw: {}, comment: {:?}, flush_on_finish_file: {}}}" , self . files , self . stats , self . writing_to_file , self . writing_raw , self . comment , self . flush_on_finish_file))
        }
    }
    impl<W: Write + Seek> ZipWriter<W> {
        pub fn get_ref(&self) -> Option<&W> {
            match &self.inner {
                GenericZipWriter::Closed => None,
                GenericZipWriter::Storer(w) => w.get_ref(),
                GenericZipWriter::Deflater(w) => w.get_ref().get_ref(),
            }
        }
        pub unsafe fn get_mut(&mut self) -> Option<&mut W> {
            unsafe {
                match &mut self.inner {
                    GenericZipWriter::Closed => None,
                    GenericZipWriter::Storer(w) => w.get_mut(),
                    GenericZipWriter::Deflater(w) => w.get_mut().get_mut(),
                }
            }
        }
    }
}
pub use self::options::sealed::FileOptionExtension;
use crate::zip::CompressionMethod::Stored;
use crate::zip::result::ZipError::UnsupportedArchive;
use crate::zip::unstable::path_to_string;
use crate::zip::zipcrypto::CHUNK_SIZE;
pub use zip_writer::ZipWriter;
#[derive(Default, Debug)]
struct ZipWriterStats {
    hasher: Hasher,
    start: u64,
    bytes_written: u64,
}
impl<W: Write + Seek> Write for ZipWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.writing_to_file {
            return Err(io::Error::other("No file has been started"));
        }
        if buf.is_empty() {
            return Ok(0);
        }
        match self.inner.ref_mut() {
            Some(ref mut w) => {
                let write_result = w.write(buf);
                if let Ok(count) = write_result {
                    self.stats.update(&buf[..count]);
                    if self.stats.bytes_written >= ZIP64_BYTES_THR {
                        let is_large_file = self
                            .files
                            .last()
                            .ok_or_else(|| std::io::Error::other("Cannot get last file"))?
                            .1
                            .large_file;
                        if !is_large_file {
                            return Err(if let Err(e) = self.abort_file() {
                                let abort_io_err: io::Error = e.into();
                                io::Error::new(
                                    abort_io_err.kind(),
                                    format!(
                                        "Large file option has not been set and abort_file() failed: {abort_io_err}"
                                    ),
                                )
                            } else {
                                io::Error::other("Large file option has not been set")
                            });
                        }
                    }
                }
                write_result
            }
            None => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "write(): ZipWriter was already closed",
            )),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self.inner.ref_mut() {
            Some(ref mut w) => w.flush(),
            None => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "flush(): ZipWriter was already closed",
            )),
        }
    }
}
impl ZipWriterStats {
    fn update(&mut self, buf: &[u8]) {
        self.hasher.update(buf);
        self.bytes_written += buf.len() as u64;
    }
}
impl<A: Read + Write + Seek> ZipWriter<A> {
    pub fn new_append_with_config(config: Config, mut readwriter: A) -> ZipResult<ZipWriter<A>> {
        readwriter.seek(SeekFrom::Start(0))?;
        let shared = ZipArchive::get_metadata(config, &mut readwriter)?;
        Ok(ZipWriter {
            inner: GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(readwriter)),
            files: shared.files,
            stats: ZipWriterStats::default(),
            writing_to_file: false,
            comment: shared.comment,
            zip64_extensible_data_sector: shared.zip64_extensible_data_sector,
            writing_raw: true,
            flush_on_finish_file: false,
            seek_possible: true,
            auto_large_file: false,
            deflate_cache: DeflateCache::default(),
        })
    }
}
impl<A: Read + Write + Seek> ZipWriter<A> {
    pub fn deep_copy_file(&mut self, src_name: &str, dest_name: &str) -> ZipResult<()> {
        self.finish_file()?;
        if src_name == dest_name || self.files.contains_key(dest_name.as_bytes()) {
            return Err(invalid!("That file already exists"));
        }
        let write_position = self.inner.try_inner_mut()?.stream_position()?;
        let src_index = self.index_by_name(src_name.as_bytes())?;
        let src_data = &mut self.files[src_index];
        let src_data_start = src_data.data_start(self.inner.try_inner_mut()?)?;
        let available = write_position
            .checked_sub(src_data_start)
            .ok_or(invalid!("File data starts past the end of the archive"))?;
        let mut compressed_size = src_data.compressed_size;
        if compressed_size > available {
            compressed_size = available;
            src_data.compressed_size = compressed_size;
        }
        self.inner
            .try_inner_mut()?
            .seek(SeekFrom::Start(write_position))?;
        let mut new_data = src_data.clone();
        let dest_name_raw = dest_name.as_bytes();
        new_data.header_start = write_position;
        new_data.central_header_start = 0;
        new_data.set_language_encoding_for(dest_name_raw);
        new_data.drop_unicode_path_renaming(dest_name_raw);
        let index = self.insert_file_data(dest_name_raw, new_data)?;
        let new_data = &mut self.files[index];
        let result: io::Result<()> = {
            let plain_writer = self.inner.try_inner_mut()?;
            new_data.write_local_header(write_position, plain_writer, dest_name_raw, None)?;
            debug_assert_eq!(
                new_data.data_start.get(),
                Some(&plain_writer.stream_position()?)
            );
            self.writing_to_file = true;
            let mut buffer = [0u8; 64 * 1024];
            let mut src_pos = src_data_start;
            let mut dst_pos = plain_writer.stream_position()?;
            let mut remaining = compressed_size;
            while remaining != 0 {
                let n = remaining.min(buffer.len() as u64) as usize;
                plain_writer.seek(SeekFrom::Start(src_pos))?;
                plain_writer.read_exact(&mut buffer[..n])?;
                plain_writer.seek(SeekFrom::Start(dst_pos))?;
                plain_writer.write_all(&buffer[..n])?;
                src_pos = src_pos
                    .checked_add(n as u64)
                    .ok_or_else(|| io::Error::other("source offset overflows u64"))?;
                dst_pos = dst_pos
                    .checked_add(n as u64)
                    .ok_or_else(|| io::Error::other("destination offset overflows u64"))?;
                remaining -= n as u64;
            }
            if self.flush_on_finish_file {
                plain_writer.flush()?;
            }
            Ok(())
        };
        self.ok_or_abort_file(result)?;
        self.writing_to_file = false;
        Ok(())
    }
}
fn validate_extra_fields(
    extra_fields: &[crate::zip::extra_fields::CustomExtraField],
) -> ZipResult<()> {
    use crate::zip::extra_fields::EXTRA_FIELD_MAPPING;
    use crate::zip::extra_fields::UsedExtraField;
    for x in extra_fields.iter() {
        let header_id = x.header_id;
        if UsedExtraField::try_from(header_id).is_err() && EXTRA_FIELD_MAPPING.contains(&header_id)
        {
            return Err(ZipError::Io(io::Error::other(format!(
                "Extra data header ID {header_id:#06} (0x{header_id:x}) \
                        requires crate feature \"unreserved\"",
            ))));
        }
    }
    Ok(())
}
impl<W: Write + Seek> ZipWriter<W> {
    pub fn new(inner: W) -> ZipWriter<W> {
        ZipWriter {
            inner: GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(inner)),
            files: IndexMap::new(),
            stats: ZipWriterStats::default(),
            writing_to_file: false,
            writing_raw: false,
            comment: Box::new([]),
            zip64_extensible_data_sector: None,
            flush_on_finish_file: false,
            seek_possible: true,
            auto_large_file: false,
            deflate_cache: DeflateCache::default(),
        }
    }

    pub fn set_comment<S>(&mut self, comment: S) -> ZipResult<()>
    where
        S: Into<Box<str>>,
    {
        self.set_raw_comment(comment.into().into_boxed_bytes())
    }
    pub fn set_raw_comment(&mut self, comment: Box<[u8]>) -> ZipResult<()> {
        let max_comment_len = u16::MAX as usize;
        if comment.len() <= max_comment_len {
            self.comment = comment;
            return Ok(());
        }
        let (allowed, rest) = comment.split_at(max_comment_len);
        self.comment = allowed.into();
        Err(ZipError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Unexpected data comment - {} bytes too long", rest.len()),
        )))
    }

    pub const fn get_raw_comment(&self) -> &[u8] {
        &self.comment
    }

    fn ok_or_abort_file<T, E: Into<ZipError>>(&mut self, result: Result<T, E>) -> ZipResult<T> {
        match result {
            Err(e) => {
                let _ = self.abort_file();
                Err(e.into())
            }
            Ok(t) => Ok(t),
        }
    }
    fn start_entry<T: FileOptionExtension>(
        &mut self,
        file_name_raw: &[u8],
        mut options: FileOptions<'_, '_, T>,
        raw_values: Option<ZipRawValues>,
    ) -> ZipResult<()> {
        self.finish_file()?;
        let header_start = self.inner.try_inner_mut()?.stream_position()?;
        {
            if raw_values.is_none()
                && let Some(extra_fields) = options.extended_options.extra_fields()
            {
                validate_extra_fields(extra_fields)?;
            }
        }
        let raw_values = raw_values.unwrap_or(ZipRawValues {
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            language_encoding: None,
        });
        if options.extended_options.validate_extra_fields()
            && let Some(extra_fields) = options.extended_options.extra_fields()
        {
            for x in extra_fields.iter() {
                if !ExtendedFileOptions::extra_field_parses(x.header_id, &x.data, x.central_only) {
                    return Err(invalid!(
                        "Extra field {:#06x} would not be readable: malformed data",
                        x.header_id
                    ));
                }
                if x.header_id
                    == crate::zip::format::extra_fields::ExtraFieldId::UnicodePath.as_u16()
                    && let Some(content) = x.data.get(5..)
                    && x.data[1..5] == crc32fast::hash(file_name_raw).to_le_bytes()
                    && core::str::from_utf8(content).is_ok()
                    && content != file_name_raw
                {
                    return Err(invalid!(
                        "Extra field {:#06x} would not be readable: renames the entry",
                        x.header_id
                    ));
                }
            }
        }
        let extra_fields = match options.extended_options.extra_fields() {
            Some(data) => data.iter().map(|x| ExtraField::Custom(x.clone())).collect(),
            None => vec![],
        };
        let entry_len: usize = mem::size_of::<Magic>()
            + mem::size_of::<ZipLocalEntryBlock>()
            + file_name_raw.len()
            + extra_fields.iter().map(|x| x.size(false)).sum::<usize>();
        if entry_len >= (u16::MAX as usize) {
            return Err(invalid!(
                "ZipLocalBlock + filename + extra fields must be less than 64KiB when combined"
            ));
        }
        let mut file = ZipFileData::initialize_local_block(
            file_name_raw,
            &options,
            &raw_values,
            header_start,
            ExtraFields {
                inner: extra_fields,
            },
        );
        if let Some(comment) = options.extended_options.take_file_comment() {
            if comment.len() > u16::MAX as usize {
                return Err(invalid!("File comment must be less than 64KiB"));
            }
            if !comment.is_ascii() {
                file.flags |= ZipFlags::LanguageEncoding.as_u16();
            }
            file.file_comment = comment;
        }
        if let Some(language_encoding) = raw_values.language_encoding {
            file.flags
                .set(ZipFlags::LanguageEncoding, language_encoding);
        }
        if (!self.seek_possible || matches!(options.encrypt_with, Some(EncryptWith::ZipCrypto(..))))
            && file
                .unix_mode()
                .is_some_and(|p| p & ffi::S_IFDIR != ffi::S_IFDIR)
        {
            file.flags |= ZipFlags::UsingDataDescriptor.as_u16();
        }
        file.version_made_by = file.version_made_by.max(file.version_needed() as u8);
        let index = self.insert_file_data(file_name_raw, file)?;
        self.writing_to_file = true;
        let result: ZipResult<()> = {
            let file = &mut self.files[index];
            let writer = self.inner.try_inner_mut()?;
            file.write_local_header(
                header_start,
                writer,
                file_name_raw,
                Some((options.alignment, options.alignment_header_id)),
            )?;
            Ok(())
        };
        self.ok_or_abort_file(result)?;
        self.stats.start = self.inner.try_inner_mut()?.stream_position()?;
        match options.encrypt_with {
            Some(EncryptWith::ZipCrypto(keys, ..)) => {
                let writer = self.close_writer()?;
                let file = &mut self.files[index];
                let mut zipwriter = crate::zip::zipcrypto::ZipCryptoWriter {
                    writer,
                    keys,
                    buffer: [0u8; CHUNK_SIZE],
                };
                self.stats.start = zipwriter.writer.stream_position()?;
                let mut crypto_header = [0u8; 12];
                crypto_header[10..=11].copy_from_slice(
                    &file
                        .last_modified_time
                        .unwrap_or_else(DateTime::default_for_write)
                        .timepart()
                        .to_le_bytes(),
                );
                let result = zipwriter.write_all(&crypto_header);
                self.ok_or_abort_file(result)?;
                self.inner = GenericZipWriter::Storer(MaybeEncrypted::ZipCrypto(zipwriter));
            }
            _ => {}
        }
        let file = &mut self.files[index];
        file.data_start.get_or_init(|| self.stats.start);
        self.stats.bytes_written = 0;
        self.stats.hasher = Hasher::new();
        Ok(())
    }
    fn insert_file_data(&mut self, file_name_raw: &[u8], file: ZipFileData) -> ZipResult<usize> {
        if self.files.contains_key(file_name_raw) {
            return Err(invalid!(
                "Duplicate filename: {:?}",
                file_name_raw.escape_ascii()
            ));
        }
        let (index, _) = self.files.insert_full(file_name_raw.into(), file);
        Ok(index)
    }
    fn switch_inner(&mut self, make_new_self: SwitchWriterFunction<W>) -> ZipResult<()> {
        let cache = &mut self.deflate_cache;
        self.inner.switch_to(cache, make_new_self)
    }
    fn finish_file(&mut self) -> ZipResult<()> {
        if !self.writing_to_file {
            return Ok(());
        }
        let make_plain_writer = self.inner.prepare_next_writer(
            Stored,
            None,
            #[cfg(any())]
            None,
        )?;
        self.switch_inner(make_plain_writer)?;
        self.switch_to_non_encrypting_writer()?;
        let writer = self.inner.try_inner_mut()?;
        if !self.writing_raw {
            let Some((file_name_raw, file)) = self.files.last_mut() else {
                return Ok(());
            };
            file.uncompressed_size = self.stats.bytes_written;
            let file_end = writer.stream_position()?;
            debug_assert!(file_end >= self.stats.start);
            file.compressed_size = file_end - self.stats.start;
            if !file.is_using_data_descriptor() {}
            file.crc32 = if file
                .aes_settings()
                .is_some_and(|(_, vendor_version)| vendor_version.is_ae2_encrypted())
            {
                0
            } else {
                self.stats.hasher.clone().finalize()
            };
            if file.is_using_data_descriptor() {
                file.write_data_descriptor(writer, self.auto_large_file)?;
            } else {
                file.update_local_file_header(writer, file_name_raw)?;
                writer.seek(SeekFrom::Start(file_end))?;
            }
        }
        if self.flush_on_finish_file {
            let result = writer.flush();
            self.ok_or_abort_file(result)?;
        }
        self.writing_to_file = false;
        Ok(())
    }
    fn switch_to_non_encrypting_writer(&mut self) -> Result<(), ZipError> {
        self.inner = GenericZipWriter::Storer(
            match mem::replace(&mut self.inner, GenericZipWriter::Closed) {
                GenericZipWriter::Storer(MaybeEncrypted::ZipCrypto(writer)) => {
                    MaybeEncrypted::Unencrypted(writer.finish()?)
                }
                GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(w)) => {
                    MaybeEncrypted::Unencrypted(w)
                }
                GenericZipWriter::Storer(MaybeEncrypted::Closed { .. }) => {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "ZipWriter's inner writer is not available",
                    )
                    .into());
                }
                _ => unreachable!(
                    "switch_to_non_encrypting_writer called with incompatible GenericZipWriter variant"
                ),
            },
        );
        Ok(())
    }

    pub fn abort_file(&mut self) -> ZipResult<()> {
        let (_, last_file) = self.files.pop().ok_or(ZipError::FileNotFound)?;
        let make_plain_writer = self.inner.prepare_next_writer(
            Stored,
            None,
            #[cfg(any())]
            None,
        )?;
        self.switch_inner(make_plain_writer)?;
        self.switch_to_non_encrypting_writer()?;
        let last_file_start = last_file
            .data_start
            .get()
            .copied()
            .unwrap_or(last_file.header_start);
        let writer = self.inner.try_inner_mut()?;
        let current_position = writer.stream_position()?;
        let rewind_safe = last_file.header_start <= current_position
            && last_file_start <= current_position
            && self.files.values().all(|file| {
                file.data_start.get().copied().unwrap_or(file.header_start) < last_file_start
            });
        if rewind_safe {
            writer.seek(SeekFrom::Start(last_file.header_start))?;
        }
        self.writing_to_file = false;
        Ok(())
    }
    pub fn start_file<S: ToString, T: FileOptionExtension>(
        &mut self,
        name: S,
        options: FileOptions<'_, '_, T>,
    ) -> ZipResult<()> {
        let name = name.to_string();
        let options = FileOptions {
            name: Some(name.as_bytes()),
            ..options
        };
        self.start_file_with_options(options)
    }
    pub fn start_file_with_options<T: FileOptionExtension>(
        &mut self,
        mut options: FileOptions<'_, '_, T>,
    ) -> ZipResult<()> {
        let Some(name) = options.name else {
            return Err(invalid!("File name must be specified"));
        };
        options.normalize();
        let make_new_self = self.inner.prepare_next_writer(
            options.compression_method,
            options.compression_level,
            #[cfg(any())]
            options.zopfli_buffer_size,
        )?;
        self.start_entry(name, options, None)?;
        let result = self.switch_inner(make_new_self);
        self.ok_or_abort_file(result)?;
        self.writing_raw = false;
        Ok(())
    }

    pub fn raw_copy_file_rename<R: Read, S: ToString>(
        &mut self,
        file: ZipFile<'_, R>,
        name: S,
    ) -> ZipResult<()> {
        let options = Self::raw_copy_options(&file)?;
        let file_name = name.to_string();
        self.raw_copy_file_rename_internal(file, file_name.as_bytes(), options, None)
    }
    fn raw_copy_options<R: Read>(
        file: &ZipFile<'_, R>,
    ) -> ZipResult<FullFileOptions<'static, 'static>> {
        let mut options = file.options().into_full_options();
        if !file.comment().is_empty() {
            options = options.with_file_comment(file.comment());
        }
        for one_extra in file.extra_data_fields() {
            let mut buff = Cursor::new(Vec::new());
            one_extra.write(&mut buff, false)?;
            let buff = buff.into_inner();
            if buff.len() >= 4 {
                let header_id = u16::from_le_bytes([buff[0], buff[1]]);
                options.add_extra_field(header_id, &buff[4..], false)?;
            }
        }
        Ok(options)
    }
    fn raw_copy_file_rename_internal<R: Read, T: FileOptionExtension>(
        &mut self,
        mut file: ZipFile<'_, R>,
        name: &[u8],
        options: FileOptions<'_, '_, T>,
        language_encoding: Option<bool>,
    ) -> ZipResult<()> {
        let raw_values = ZipRawValues {
            crc32: file.crc32(),
            compressed_size: file.compressed_size(),
            uncompressed_size: file.size(),
            language_encoding,
        };
        self.start_entry(name, options, Some(raw_values))?;
        self.writing_to_file = true;
        self.writing_raw = true;
        io::copy(&mut file.take_raw_reader()?, self)?;
        self.finish_file()
    }

    pub fn add_directory<S, T: FileOptionExtension>(
        &mut self,
        name: S,
        mut options: FileOptions<'_, '_, T>,
    ) -> ZipResult<()>
    where
        S: Into<String>,
    {
        let permissions = options
            .permissions
            .get_or_insert(FileOptions::DEFAULT_DIR_PERMISSIONS);
        *permissions |= ffi::S_IFDIR;
        options.compression_method = Stored;
        options.encrypt_with = None;
        let name_as_string = name.into();
        let name_with_slash = match name_as_string.chars().last() {
            Some('/' | '\\') => name_as_string,
            _ => name_as_string + "/",
        };
        self.start_entry(name_with_slash.as_bytes(), options, None)?;
        self.writing_to_file = false;
        self.switch_to_non_encrypting_writer()?;
        Ok(())
    }

    pub fn finish(mut self) -> ZipResult<W> {
        let _central_start = self.finalize()?;
        self.close_writer()
    }
    fn close_writer(&mut self) -> ZipResult<W> {
        let inner = mem::replace(&mut self.inner, GenericZipWriter::Closed);
        match inner {
            GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(w)) => Ok(w),
            _ => Err(invalid!(
                "Should have switched to stored and unencrypted beforehand",
            )),
        }
    }
    pub fn add_symlink<N: ToString, T: ToString, E: FileOptionExtension>(
        &mut self,
        name: N,
        target: T,
        mut options: FileOptions<'_, '_, E>,
    ) -> ZipResult<()> {
        let permissions = options
            .permissions
            .get_or_insert(FileOptions::DEFAULT_SYMLINK_PERMISSIONS);
        *permissions |= ffi::S_IFLNK;
        options.compression_method = Stored;
        let file_name = name.to_string();
        self.start_entry(file_name.as_bytes(), options, None)?;
        self.writing_to_file = true;
        let result = self.write_all(target.to_string().as_bytes());
        self.ok_or_abort_file(result)?;
        self.writing_raw = false;
        self.finish_file()?;
        Ok(())
    }

    pub fn add_prepared_file(&mut self, file: PreparedZipFile) -> ZipResult<()> {
        let PreparedZipFile {
            file_name,
            options,
            crc32,
            uncompressed_size,
            compressed_size,
            data,
        } = file;
        let raw_values = ZipRawValues {
            crc32,
            compressed_size,
            uncompressed_size,
            language_encoding: None,
        };
        self.start_entry(&file_name, options, Some(raw_values))?;
        self.writing_raw = true;
        let result = self
            .inner
            .try_inner_mut()
            .and_then(|writer| writer.write_all(&data));
        self.ok_or_abort_file(result)?;
        let auto_large_file = self.auto_large_file;
        let result = (|| -> ZipResult<()> {
            let writer = self.inner.try_inner_mut()?;
            let Some((file_name_raw, file)) = self.files.last_mut() else {
                debug_assert!(
                    false,
                    "Newly added file not found while finishing it in add_prepared_file"
                );
                return Err(ZipError::FileNotFound);
            };
            if file.is_using_data_descriptor() {
                file.write_data_descriptor(writer, auto_large_file)?;
            } else if file.large_file {
                let file_end = writer.stream_position()?;
                file.update_local_file_header(writer, file_name_raw)?;
                writer.seek(SeekFrom::Start(file_end))?;
            }
            Ok(())
        })();
        self.ok_or_abort_file(result)?;
        self.finish_file()
    }
    fn finalize(&mut self) -> ZipResult<u64> {
        const STALE_FILL_MAX_LEN: u64 = 0xFF00;
        self.finish_file()?;
        let (central_start, is_zip64) = self.write_central_and_footer()?;
        if self.seek_possible {
            let writer = self.inner.try_inner_mut()?;
            let footer_end = writer.stream_position()?;
            let stream_end = writer.seek(SeekFrom::End(0))?;
            let archive_end = stream_end - self.comment.len() as u64;
            if archive_end <= footer_end
                && footer_end < stream_end
                && stream_end - footer_end < STALE_FILL_MAX_LEN
            {
                writer.seek(SeekFrom::Start(footer_end))?;
                io::copy(&mut io::repeat(0xFF).take(stream_end - footer_end), writer)?;
            } else if footer_end < stream_end {
                writer.seek(SeekFrom::Start(central_start))?;
                writer.write_all(&0_u32.to_le_bytes())?;
                let start_zip32_cde = footer_end
                    - (size_of::<Magic>() + size_of::<Zip32CDEBlock>()) as u64
                    - self.comment.len() as u64;
                writer.seek(SeekFrom::Start(start_zip32_cde))?;
                writer.write_all(&0_u32.to_le_bytes())?;
                let zip64_extensible_len = self
                    .zip64_extensible_data_sector
                    .as_ref()
                    .map(|e| e.len() as u64)
                    .unwrap_or(0);
                if is_zip64 {
                    let start_zip64_locator = start_zip32_cde
                        - (size_of::<Magic>() + size_of::<Zip64CentralDirectoryEndLocator>())
                            as u64;
                    writer.seek(SeekFrom::Start(start_zip64_locator))?;
                    writer.write_all(&0_u32.to_le_bytes())?;
                    let start_zip64_cde = start_zip64_locator
                        - (size_of::<Magic>() + Zip64CentralDirectoryEnd::MIN_FULL_SIZE) as u64
                        - zip64_extensible_len;
                    writer.seek(SeekFrom::Start(start_zip64_cde))?;
                    writer.write_all(&0_u32.to_le_bytes())?;
                }
                let central_and_footer_size = footer_end - central_start;
                writer.seek(SeekFrom::End(-(central_and_footer_size as i64)))?;
                let (_, new_is_zip64) = self.write_central_and_footer()?;
                #[cfg(debug_assertions)]
                {
                    let archive_end = if new_is_zip64 && new_is_zip64 != is_zip64 {
                        let new_size = (size_of::<Magic>()
                            + Zip64CentralDirectoryEnd::MIN_FULL_SIZE)
                            as u64
                            + zip64_extensible_len
                            + (size_of::<Magic>() + size_of::<Zip64CentralDirectoryEndLocator>())
                                as u64;
                        archive_end + new_size
                    } else {
                        archive_end
                    };
                    debug_assert!(
                        self.inner.try_inner_mut()?.stream_position()?
                            == archive_end + self.comment.len() as u64
                    );
                }
            } else {
                writer.seek(SeekFrom::Start(footer_end))?;
            }
        }
        Ok(central_start)
    }
    fn write_central_and_footer(&mut self) -> Result<(u64, bool), ZipError> {
        let writer = self.inner.try_inner_mut()?;
        let mut version_needed = u16::from(MIN_VERSION);
        let central_start = writer.stream_position()?;
        for (filename_raw, file) in &mut self.files {
            file.write_central_directory_header(writer, filename_raw)?;
            version_needed = version_needed.max(file.version_needed());
        }
        let central_size = writer.stream_position()? - central_start;
        let is64 = self.files.len() > ZIP64_ENTRY_THR
            || central_size.max(central_start) > ZIP64_BYTES_THR
            || self.zip64_extensible_data_sector.is_some();
        if is64 {
            let zip64_extensible_data_sector = self.zip64_extensible_data_sector.clone();
            let extensible_len = zip64_extensible_data_sector
                .as_ref()
                .map(|e| e.len() as u64)
                .unwrap_or(0);
            let zip64_footer = Zip64CentralDirectoryEnd {
                record_size: extensible_len + 44,
                version_made_by: version_needed,
                version_needed_to_extract: version_needed,
                disk_number: 0,
                disk_with_central_directory: 0,
                number_of_files_on_this_disk: self.files.len() as u64,
                number_of_files: self.files.len() as u64,
                central_directory_size: central_size,
                central_directory_offset: central_start,
                zip64_extensible_data_sector,
            };
            zip64_footer.write(writer)?;
            let zip64_footer = Zip64CentralDirectoryEndLocator {
                disk_with_central_directory: 0,
                end_of_central_directory_offset: central_start + central_size,
                number_of_disks: 1,
            };
            zip64_footer.write(writer)?;
        }
        let central_directory_size = if is64 {
            ZIP64_BYTES_THR_U32
        } else {
            central_size.min(ZIP64_BYTES_THR) as u32
        };
        let number_of_files = self.files.len().min(ZIP64_ENTRY_THR) as u16;
        let footer = Zip32CentralDirectoryEnd {
            disk_number: 0,
            disk_with_central_directory: 0,
            zip_file_comment: self.comment.clone(),
            number_of_files_on_this_disk: number_of_files,
            number_of_files,
            central_directory_size,
            central_directory_offset: central_start.min(ZIP64_BYTES_THR) as u32,
        };
        footer.write(writer)?;
        Ok((central_start, is64))
    }
    fn index_by_name(&self, name: &[u8]) -> ZipResult<usize> {
        self.files.get_index_of(name).ok_or(ZipError::FileNotFound)
    }
    pub fn shallow_copy_file(&mut self, src_name: &str, dest_name: &str) -> ZipResult<()> {
        self.finish_file()?;
        if src_name == dest_name {
            return Err(invalid!("Trying to copy a file to itself"));
        }
        let src_index = self.index_by_name(src_name.as_bytes())?;
        let mut dest_data = self.files[src_index].clone();
        let file_name_raw = dest_name.as_bytes();
        dest_data.central_header_start = 0;
        dest_data.set_language_encoding_for(file_name_raw);
        dest_data.drop_unicode_path_renaming(file_name_raw);
        self.insert_file_data(file_name_raw, dest_data)?;
        Ok(())
    }
}
impl<W: Write> ZipWriter<StreamWriter<W>> {}
impl<W: Write + Seek> Drop for ZipWriter<W> {
    fn drop(&mut self) {
        if !self.inner.is_closed()
            && let Err(e) = self.finalize()
        {
            let _ = write!(
                io::stderr(),
                "ZipWriter::drop: failed to finalize archive: {e:?}"
            );
        }
    }
}
type SwitchWriterFunction<W> =
    Box<dyn FnOnce(MaybeEncrypted<W>, &mut DeflateCache<W>) -> ZipResult<GenericZipWriter<W>>>;
pub(crate) struct DeflateCache<W: Write> {
    level: u32,
    idle: Option<flate2::write::DeflateEncoder<MaybeEncrypted<W>>>,
}
impl<W: Write> Default for DeflateCache<W> {
    fn default() -> Self {
        Self {
            level: 0,
            idle: None,
        }
    }
}
impl<W: Write> Debug for DeflateCache<W> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match &self.idle {
            Some(_) => write!(f, "DeflateCache(level {})", self.level),
            None => f.write_str("DeflateCache(empty)"),
        }
    }
}
impl<W: Write> DeflateCache<W> {
    fn encoder(
        &mut self,
        bare: MaybeEncrypted<W>,
        level: u32,
    ) -> io::Result<flate2::write::DeflateEncoder<MaybeEncrypted<W>>> {
        let cached_level = mem::replace(&mut self.level, level);
        match self.idle.take() {
            Some(mut encoder) if cached_level == level => {
                match mem::replace(encoder.get_mut(), bare) {
                    MaybeEncrypted::Closed { .. } => Ok(encoder),
                    other => {
                        let bare = mem::replace(encoder.get_mut(), other);
                        Ok(flate2::write::DeflateEncoder::new(
                            bare,
                            flate2::Compression::new(level),
                        ))
                    }
                }
            }
            _ => Ok(flate2::write::DeflateEncoder::new(
                bare,
                flate2::Compression::new(level),
            )),
        }
    }
    fn park(
        &mut self,
        mut encoder: flate2::write::DeflateEncoder<MaybeEncrypted<W>>,
    ) -> io::Result<MaybeEncrypted<W>> {
        let bare = encoder.reset(MaybeEncrypted::Closed { bytes_written: 0 })?;
        self.idle = Some(encoder);
        Ok(bare)
    }
}
impl<W: Write + Seek> GenericZipWriter<W> {
    fn prepare_next_writer(
        &self,
        compression: CompressionMethod,
        compression_level: Option<i64>,
        #[cfg(any())] zopfli_buffer_size: Option<usize>,
    ) -> ZipResult<SwitchWriterFunction<W>> {
        if let GenericZipWriter::Closed = self {
            return Err(
                io::Error::new(io::ErrorKind::BrokenPipe, "ZipWriter was already closed").into(),
            );
        }
        match compression {
            Stored => {
                if compression_level.is_some() {
                    Err(UnsupportedArchive("Unsupported compression level"))
                } else {
                    Ok(Box::new(|bare, _| Ok(GenericZipWriter::Storer(bare))))
                }
            }
            CompressionMethod::Deflated => {
                let default = { { i64::from(flate2::Compression::default().level()) } };
                let level = validate_value_in_range(
                    compression_level.unwrap_or(default),
                    deflate_compression_level_range(),
                )
                .ok_or(UnsupportedArchive("Unsupported compression level"))?;
                {
                    Ok(Box::new(move |bare, cache: &mut DeflateCache<W>| {
                        Ok(GenericZipWriter::Deflater(cache.encoder(bare, level)?))
                    }))
                }
            }
            CompressionMethod::AES => Err(UnsupportedArchive(
                "AES encryption is enabled through FileOptions::with_aes_encryption",
            )),
            CompressionMethod::Unsupported(..) => {
                Err(UnsupportedArchive("Unsupported compression"))
            }
        }
    }
    fn switch_to(
        &mut self,
        deflate_cache: &mut DeflateCache<W>,
        make_new_self: SwitchWriterFunction<W>,
    ) -> ZipResult<()> {
        let bare = match mem::replace(self, GenericZipWriter::Closed) {
            GenericZipWriter::Storer(w) => w,
            GenericZipWriter::Deflater(w) => deflate_cache.park(w)?,
            GenericZipWriter::Closed => {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "ZipWriter was already closed",
                )
                .into());
            }
        };
        *self = make_new_self(bare, deflate_cache)?;
        Ok(())
    }
    fn ref_mut(&mut self) -> Option<&mut dyn Write> {
        match self {
            GenericZipWriter::Storer(w) => Some(w as &mut dyn Write),
            GenericZipWriter::Deflater(w) => Some(w as &mut dyn Write),
            GenericZipWriter::Closed => None,
        }
    }
    const fn is_closed(&self) -> bool {
        matches!(*self, GenericZipWriter::Closed)
    }
    fn try_inner_mut(&mut self) -> Result<&mut W, std::io::Error> {
        match *self {
            GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(ref mut w)) => Ok(w),
            _ => Err(std::io::Error::other(
                "Should have switched to stored and unencrypted beforehand",
            )),
        }
    }
}
pub struct ZipFileBuilder {
    compressor: GenericZipWriter<Cursor<Vec<u8>>>,
    file_name: Box<[u8]>,
    options: FullFileOptions<'static, 'static>,
    hasher: Hasher,
    uncompressed_size: u64,
}
impl Debug for ZipFileBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "ZipFileBuilder {{file_name: {:?}, uncompressed_size: {}}}",
            self.file_name.escape_ascii().to_string(),
            self.uncompressed_size
        ))
    }
}
impl ZipFileBuilder {
    pub fn new<N: AsRef<str>, T: FileOptionExtension>(
        name: N,
        mut options: FileOptions<'_, '_, T>,
    ) -> ZipResult<Self> {
        if options.encrypt_with.is_some() {
            return Err(UnsupportedArchive(
                "Encrypted files cannot be compressed outside of a ZipWriter",
            ));
        }
        options.normalize();
        {
            if let Some(extra_fields) = options.extended_options.extra_fields() {
                validate_extra_fields(extra_fields)?;
            }
        }
        let owned_options = FullFileOptions {
            compression_method: options.compression_method,
            compression_level: options.compression_level,
            external_attributes: options.external_attributes,
            last_modified_time: options.last_modified_time,
            permissions: options.permissions,
            large_file: options.large_file,
            encrypt_with: None,
            extended_options: ExtendedFileOptions {
                extra_fields: options
                    .extended_options
                    .extra_fields()
                    .cloned()
                    .unwrap_or_default(),
                file_comment: options.extended_options.take_file_comment(),
                validate_extra_fields: options.extended_options.validate_extra_fields(),
            },
            alignment: options.alignment,
            alignment_header_id: options.alignment_header_id,
            system: options.system,
            name: None,
        };
        let mut compressor =
            GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(Cursor::new(Vec::new())));
        let make_compressor = compressor.prepare_next_writer(
            owned_options.compression_method,
            owned_options.compression_level,
            #[cfg(any())]
            owned_options.zopfli_buffer_size,
        )?;
        compressor.switch_to(&mut DeflateCache::default(), make_compressor)?;
        Ok(Self {
            compressor,
            file_name: name.as_ref().as_bytes().into(),
            options: owned_options,
            hasher: Hasher::new(),
            uncompressed_size: 0,
        })
    }

    pub fn finish(mut self) -> ZipResult<PreparedZipFile> {
        self.compressor.switch_to(
            &mut DeflateCache::default(),
            Box::new(|bare, _| Ok(GenericZipWriter::Storer(bare))),
        )?;
        let GenericZipWriter::Storer(MaybeEncrypted::Unencrypted(cursor)) = self.compressor else {
            return Err(ZipError::Io(io::Error::other(
                "Compressor was in an unexpected state",
            )));
        };
        let data = cursor.into_inner().into_boxed_slice();
        let compressed_size = data.len() as u64;
        let mut options = self.options;
        if compressed_size >= ZIP64_BYTES_THR || self.uncompressed_size >= ZIP64_BYTES_THR {
            options.large_file = true;
        }
        Ok(PreparedZipFile {
            file_name: self.file_name,
            options,
            crc32: self.hasher.finalize(),
            uncompressed_size: self.uncompressed_size,
            compressed_size,
            data,
        })
    }
}
impl Write for ZipFileBuilder {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let Some(w) = self.compressor.ref_mut() else {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "write(): ZipFileBuilder was already closed",
            ));
        };
        let count = w.write(buf)?;
        self.hasher.update(&buf[..count]);
        self.uncompressed_size += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        match self.compressor.ref_mut() {
            Some(w) => w.flush(),
            None => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "flush(): ZipFileBuilder was already closed",
            )),
        }
    }
}
pub struct PreparedZipFile {
    file_name: Box<[u8]>,
    options: FullFileOptions<'static, 'static>,
    crc32: u32,
    uncompressed_size: u64,
    compressed_size: u64,
    data: Box<[u8]>,
}
impl Debug for PreparedZipFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f . write_fmt (format_args ! ("PreparedZipFile {{file_name: {:?}, crc32: {}, uncompressed_size: {}, compressed_size: {}}}" , self . file_name . escape_ascii () . to_string () , self . crc32 , self . uncompressed_size , self . compressed_size))
    }
}
fn deflate_compression_level_range() -> std::ops::RangeInclusive<u32> {
    let min = { { flate2::Compression::fast().level() } };
    let max = { { flate2::Compression::best().level() } };
    min..=max
}
fn validate_value_in_range<T: Ord + Copy, U: Ord + Copy + TryFrom<T>>(
    value: T,
    range: std::ops::RangeInclusive<U>,
) -> Option<U> {
    let value: U = value.try_into().ok()?;
    if range.contains(&value) {
        Some(value)
    } else {
        None
    }
}
impl ZipFileData {
    pub(crate) fn update_local_file_header<T: Write + Seek>(
        &mut self,
        writer: &mut T,
        file_name_raw: &[u8],
    ) -> ZipResult<()> {
        writer.seek(SeekFrom::Start(
            self.header_start + (size_of::<Magic>() + offset_of!(ZipLocalEntryBlock, crc32)) as u64,
        ))?;
        writer.write_all(&self.crc32.to_le_bytes())?;
        if self.large_file {
            debug_assert!(matches!(
                self.extra_fields.local_extra_fields().next(),
                Some(ExtraField::Zip64ExtendedInformation(_))
            ));
            writer.write_all(&ZIP64_BYTES_THR_U32.to_le_bytes())?;
            writer.write_all(&ZIP64_BYTES_THR_U32.to_le_bytes())?;
            self.update_local_zip64_extra_field(writer, file_name_raw)?;
        } else {
            if self.compressed_size >= ZIP64_BYTES_THR {
                return Err(ZipError::Io(std::io::Error::other(
                    "large_file(true) option has not been set",
                )));
            }
            debug_assert!(
                !self
                    .extra_fields
                    .local_extra_fields()
                    .any(|field| matches!(field, ExtraField::Zip64ExtendedInformation(_)))
            );
            writer.write_all(&(self.compressed_size as u32).to_le_bytes())?;
            writer.write_all(&(self.uncompressed_size as u32).to_le_bytes())?;
        }
        Ok(())
    }
    fn update_local_zip64_extra_field<T: Write + Seek>(
        &mut self,
        writer: &mut T,
        file_name_raw: &[u8],
    ) -> ZipResult<()> {
        let zip64_block = Zip64ExtendedInformation::local_header(
            self.large_file,
            self.uncompressed_size,
            self.compressed_size,
        )
        .ok_or(invalid!(
            "Attempted to update a nonexistent ZIP64 extra field"
        ))?;
        let zip64_extra_field_start = self.header_start
            + (size_of::<Magic>() + size_of::<ZipLocalEntryBlock>()) as u64
            + file_name_raw.len() as u64;
        writer.seek(SeekFrom::Start(zip64_extra_field_start))?;
        zip64_block.write(writer, true)?;
        Ok(())
    }
    pub(crate) fn write_central_directory_header<T: Write>(
        &mut self,
        writer: &mut T,
        file_name_raw: &[u8],
    ) -> ZipResult<()> {
        self.extra_fields
            .inner
            .retain(|field| !matches!(field, ExtraField::Zip64ExtendedInformation(_)));
        if let Some(zip64_block) = Zip64ExtendedInformation::central_header(
            self.large_file,
            self.uncompressed_size,
            self.compressed_size,
            self.header_start,
        ) {
            self.extra_fields
                .inner
                .insert(0, ExtraField::Zip64ExtendedInformation(zip64_block));
        }
        let sizes_in_zip64 = self . extra_fields . inner . iter () . any (| field | matches ! (field , ExtraField :: Zip64ExtendedInformation (z) if z . sizes . is_some ()) ,) ;
        let central_extra_fields = self.extra_fields.central_extra_fields();
        let extra_field_len: usize = self
            .extra_fields
            .central_extra_fields()
            .map(|x| x.size(false))
            .sum();
        let compressed_size = if self.large_file || sizes_in_zip64 {
            ZIP64_BYTES_THR as u32
        } else {
            self.compressed_size
                .min(ZIP64_BYTES_THR)
                .try_into()
                .map_err(std::io::Error::other)?
        };
        let uncompressed_size = if self.large_file || sizes_in_zip64 {
            ZIP64_BYTES_THR as u32
        } else {
            self.uncompressed_size
                .min(ZIP64_BYTES_THR)
                .try_into()
                .map_err(std::io::Error::other)?
        };
        let offset = self
            .header_start
            .min(ZIP64_BYTES_THR)
            .try_into()
            .map_err(std::io::Error::other)?;
        let last_modified_time = self
            .last_modified_time
            .unwrap_or_else(DateTime::default_for_write);
        let version_to_extract = self.version_needed();
        let version_made_by = u16::from(self.version_made_by).max(version_to_extract);
        let compression_method = self.extern_compression();
        let extra_field_length = u16::try_from(extra_field_len)
            .map_err(|_| invalid!("Extra field length in central directory exceeds 64KiB"))?;
        let block = ZipCentralEntryBlock {
            version_made_by: ((self.system as u16) << 8) | version_made_by,
            version_to_extract,
            flags: self.flags(file_name_raw),
            compression_method,
            last_mod_time: last_modified_time.timepart(),
            last_mod_date: last_modified_time.datepart(),
            crc32: self.crc32,
            compressed_size,
            uncompressed_size,
            file_name_length: file_name_raw
                .len()
                .try_into()
                .map_err(std::io::Error::other)?,
            extra_field_length,
            file_comment_length: self
                .file_comment
                .len()
                .try_into()
                .map_err(std::io::Error::other)?,
            disk_number: 0,
            internal_file_attributes: 0,
            external_file_attributes: self.external_attributes,
            offset,
        };
        block.write(writer)?;
        writer.write_all(file_name_raw)?;
        for one_extra_field in central_extra_fields {
            one_extra_field.write(writer, false)?;
        }
        writer.write_all(self.file_comment.as_bytes())?;
        Ok(())
    }
    pub(crate) fn write_local_header<T: Write>(
        &mut self,
        header_start: u64,
        writer: &mut T,
        file_name_raw: &[u8],
        alignment_opt: Option<(u16, u16)>,
    ) -> ZipResult<()> {
        self.extra_fields
            .inner
            .retain(|field| !matches!(field, ExtraField::Zip64ExtendedInformation { .. }));
        if let Some(zip64) = Zip64ExtendedInformation::local_header(
            self.large_file,
            self.uncompressed_size,
            self.compressed_size,
        ) {
            self.extra_fields
                .inner
                .insert(0, ExtraField::Zip64ExtendedInformation(zip64));
        }
        let (compressed_size, uncompressed_size) = if self.is_using_data_descriptor() {
            (0, 0)
        } else {
            (
                self.clamp_size_field(self.compressed_size)?,
                self.clamp_size_field(self.uncompressed_size)?,
            )
        };
        let mut extra_field_len: usize = self
            .extra_fields
            .local_extra_fields()
            .map(|x| x.size(true))
            .sum();
        let last_modified_time = self
            .last_modified_time
            .unwrap_or_else(DateTime::default_for_write);
        let compression_method = self.extern_compression();
        let header_end = header_start
            + (size_of::<Magic>() + size_of::<ZipLocalEntryBlock>()) as u64
            + file_name_raw.len() as u64;
        let opt_alignment_field = if let Some((alignment, header_id)) = alignment_opt
            && alignment > 1
        {
            let extra_field_end = header_end + extra_field_len as u64;
            let align = u64::from(alignment);
            let unaligned_header_bytes = extra_field_end % align;
            if unaligned_header_bytes != 0 {
                let mut pad_length = (align - unaligned_header_bytes) as usize;
                while pad_length < 6 {
                    pad_length += align as usize;
                }
                extra_field_len = self
                    .extra_fields
                    .local_extra_fields()
                    .map(|x| x.size(true))
                    .sum();
                let new_len = extra_field_len + pad_length;
                if new_len > u16::MAX as usize {
                    None
                } else {
                    let pad_length_u16: Result<u16, core::num::TryFromIntError> =
                        pad_length.try_into();
                    if let Ok(pad_length) = pad_length_u16
                        && let Some(alignment_extra_field) = DataStreamAlignment::new_with_header_id(
                            header_id, pad_length, alignment,
                        )
                    {
                        extra_field_len = self
                            .extra_fields
                            .local_extra_fields()
                            .map(|x| x.size(true))
                            .sum::<usize>()
                            + alignment_extra_field.full_size(true);
                        debug_assert_eq!((extra_field_len as u64 + header_end) % align, 0);
                        Some(alignment_extra_field)
                    } else {
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        let data_start = header_end + extra_field_len as u64;
        self.data_start.take();
        self.data_start.get_or_init(|| data_start);
        let block = ZipLocalEntryBlock {
            version_made_by: self.version_needed(),
            flags: self.flags(file_name_raw),
            compression_method,
            last_mod_time: last_modified_time.timepart(),
            last_mod_date: last_modified_time.datepart(),
            crc32: self.crc32,
            compressed_size,
            uncompressed_size,
            file_name_length: file_name_raw
                .len()
                .try_into()
                .map_err(std::io::Error::other)?,
            extra_field_length: extra_field_len
                .try_into()
                .map_err(|_| invalid!("Extra data field is too large"))?,
        };
        block.write(writer)?;
        writer.write_all(file_name_raw)?;
        let local_extra_fields = self.extra_fields.local_extra_fields_mut();
        for one_extra_field in local_extra_fields {
            one_extra_field.write(&mut *writer, true)?;
        }
        if let Some(alignment_extra_field) = opt_alignment_field {
            alignment_extra_field.write(writer, true)?;
        }
        Ok(())
    }
}
pub struct StreamWriter<W: Write> {
    inner: W,
    bytes_written: u64,
}
impl<W: Write> StreamWriter<W> {
    pub fn new(inner: W) -> StreamWriter<W> {
        Self {
            inner,
            bytes_written: 0,
        }
    }
    pub fn get_ref(&self) -> &W {
        &self.inner
    }
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }
    pub fn into_inner(self) -> W {
        self.inner
    }
}
impl<W: Write> Write for StreamWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = self.inner.write(buf)?;
        self.bytes_written += bytes_written as u64;
        Ok(bytes_written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
impl<W: Write> Seek for StreamWriter<W> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Current(0) | SeekFrom::End(0) => return Ok(self.bytes_written),
            SeekFrom::Start(x) if x == self.bytes_written => {
                return Ok(self.bytes_written);
            }
            _ => {}
        }
        Err(io::Error::new(
            ErrorKind::Unsupported,
            "seek is not supported",
        ))
    }
}
