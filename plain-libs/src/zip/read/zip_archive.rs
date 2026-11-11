use crate::zip::compression::CompressionMethod;
use crate::zip::format::MagicPositionCache;
use crate::zip::format::blocks::{FixedSizeBlock, Zip32CentralDirectoryEnd, ZipCentralEntryBlock};
use crate::zip::format::find_central_directory_end;
use crate::zip::format::magic::Magic;
use crate::zip::read::config::Config;
use crate::zip::read::magic_finder::MagicFinder;
use crate::zip::read::readers::{ZipFileReader, ZipFileSeekReader};
use crate::zip::read::zipfile::{ZipFileEntry, ZipFileEntryWithDataDescriptor};
use crate::zip::read::{
    ArchiveOffset, CentralDirectoryInfo, ZipFile, ZipFileSeek, ZipReadOptions,
    central_header_to_zip_file_inner,
};
use crate::zip::result::{ZipError, ZipResult};
use crate::zip::types::ZipFileData;
use crate::zip::unstable::path_to_string;
use core::ops::Range;
use crate::indexmap::IndexMap;
use std::borrow::Cow;
use std::io::{Read, Seek, SeekFrom};
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[derive(Debug)]
pub struct ZipArchiveMetadata {
    pub(crate) files: IndexMap<Box<[u8]>, ZipFileData>,
    pub(crate) raw_file_count: usize,
    pub(crate) offset: u64,
    pub(crate) dir_start: u64,
    pub(crate) config: Config,
    pub(crate) comment: Box<[u8]>,
    pub(crate) zip64_extensible_data_sector: Option<Box<[u8]>>,
}
impl ZipArchiveMetadata {
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn entry(&self, index: usize) -> ZipResult<ZipFileEntry<'_>> {
        let (file_name_raw, data) = self.files.get_index(index).ok_or(ZipError::FileNotFound)?;
        Ok(ZipFileEntry {
            file_name_raw: Cow::Borrowed(file_name_raw),
            data: Cow::Borrowed(data),
        })
    }
    #[must_use]
    pub fn has_duplicate_names(&self) -> bool {
        self.raw_file_count > self.files.len()
    }
}
#[derive(Debug)]
pub(crate) struct SharedBuilder {
    pub(crate) files: IndexMap<Box<[u8]>, ZipFileData>,
    pub(crate) raw_file_count: usize,
    pub(super) offset: u64,
    pub(super) dir_start: u64,
    pub(super) config: Config,
}
impl SharedBuilder {
    pub(crate) fn build(
        self,
        comment: Box<[u8]>,
        zip64_extensible_data_sector: Option<Box<[u8]>>,
    ) -> ZipArchiveMetadata {
        ZipArchiveMetadata {
            files: self.files,
            raw_file_count: self.raw_file_count,
            offset: self.offset,
            dir_start: self.dir_start,
            config: self.config,
            comment,
            zip64_extensible_data_sector,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ZipArchive<R> {
    pub(super) reader: R,
    pub(crate) shared: Arc<ZipArchiveMetadata>,
}
impl<R> ZipArchive<R> {
    pub(crate) fn from_finalized_writer(
        files: IndexMap<Box<[u8]>, ZipFileData>,
        comment: Box<[u8]>,
        zip64_extensible_data_sector: Option<Box<[u8]>>,
        reader: R,
        central_start: u64,
    ) -> Self {
        let initial_offset = match files.first() {
            Some((_, file)) => file.header_start,
            None => central_start,
        };
        let raw_file_count = files.len();
        let shared = Arc::new(ZipArchiveMetadata {
            files,
            raw_file_count,
            offset: initial_offset,
            dir_start: central_start,
            config: Config {
                archive_offset: ArchiveOffset::Known(initial_offset),
            },
            comment,
            zip64_extensible_data_sector,
        });
        Self { reader, shared }
    }
}
use crate::zip::result::invalid;
#[derive(Debug, Clone)]
struct FailedRun {
    valid: usize,
    error: ZipError,
}
#[derive(Debug, Default)]
struct CentralDirectoryChains {
    runs: std::collections::HashMap<u64, FailedRun>,
}
impl CentralDirectoryChains {
    fn fail(&mut self, chain: &[u64], tail: FailedRun) -> ZipError {
        for (i, &central_header_start) in chain.iter().enumerate() {
            let run = FailedRun {
                valid: chain.len() - i + tail.valid,
                error: tail.error.clone(),
            };
            self.runs.insert(central_header_start, run);
        }
        tail.error
    }
}
impl<R: Read + Seek> ZipArchive<R> {
    pub(crate) fn get_metadata(config: Config, reader: &mut R) -> ZipResult<ZipArchiveMetadata> {
        let file_len = reader.seek(SeekFrom::End(0))?;
        let mut last_err = None;
        let mut cd_chains = CentralDirectoryChains::default();
        const EOCD_SIG_BYTES: [u8; mem::size_of::<Magic>()] =
            Magic::CENTRAL_DIRECTORY_END_SIGNATURE.to_le_bytes();
        const EOCD64_SIG_BYTES: [u8; mem::size_of::<Magic>()] =
            Magic::ZIP64_CENTRAL_DIRECTORY_END_SIGNATURE.to_le_bytes();
        const CDFH_SIG_BYTES: [u8; mem::size_of::<Magic>()] =
            Magic::CENTRAL_DIRECTORY_HEADER_SIGNATURE.to_le_bytes();
        let min_eocd_position = file_len.saturating_sub(
            size_of::<Zip32CentralDirectoryEnd>() as u64 + size_of::<Magic>() as u64 + (1 << 16),
        );
        let mut eocd_finder = MagicFinder::new(&EOCD_SIG_BYTES, min_eocd_position, file_len);
        let mut cdfh_cache = MagicPositionCache::new(CDFH_SIG_BYTES);
        let mut eocd64_cache = MagicPositionCache::new(EOCD64_SIG_BYTES);
        loop {
            let cde = match find_central_directory_end(
                reader,
                config.archive_offset,
                file_len,
                &mut eocd_finder,
                &mut cdfh_cache,
                &mut eocd64_cache,
            ) {
                Ok(cde) => cde,
                Err(e) => {
                    return Err(last_err.unwrap_or(e));
                }
            };
            match CentralDirectoryInfo::try_from(&cde)
                .and_then(|info| Self::read_central_header(&info, config, reader, &mut cd_chains))
            {
                Ok(shared) => {
                    let zip64_extensible_data_sector = if let Some(eocd64) = cde.eocd64 {
                        eocd64.data.zip64_extensible_data_sector
                    } else {
                        None
                    };
                    return Ok(
                        shared.build(cde.eocd.data.zip_file_comment, zip64_extensible_data_sector)
                    );
                }
                Err(e) => {
                    if cde.central_directory_abuts_end() {
                        return Err(e);
                    }
                    last_err = Some(e);
                }
            }
            continue;
        }
    }
    fn read_one_central_header(
        reader: &mut R,
        central_header_start: u64,
    ) -> ZipResult<(Box<[u8]>, ZipFileData, u64)> {
        let block = ZipCentralEntryBlock::parse(reader)?;
        let next = central_header_start
            + (size_of::<Magic>() + size_of::<ZipCentralEntryBlock>()) as u64
            + u64::from(block.file_name_length)
            + u64::from(block.extra_field_length)
            + u64::from(block.file_comment_length);
        let (file, file_name_raw) =
            central_header_to_zip_file_inner(reader, 0, central_header_start, block)?;
        if file
            .header_start
            .checked_add(central_header_start)
            .is_none()
        {
            return Err(invalid!("Archive header is too large"));
        }
        Ok((file_name_raw.into_boxed_slice(), file, next))
    }
    fn read_central_header(
        dir_info: &CentralDirectoryInfo,
        config: Config,
        reader: &mut R,
        cd_chains: &mut CentralDirectoryChains,
    ) -> Result<SharedBuilder, ZipError> {
        let max_files = usize::try_from(dir_info.directory_size / 46).unwrap_or(usize::MAX);
        let dir_start_bound = usize::try_from(dir_info.directory_start).unwrap_or(usize::MAX);
        let file_capacity = if dir_info.number_of_files > dir_start_bound {
            0
        } else {
            dir_info.number_of_files.min(max_files)
        };
        if dir_info.disk_number != dir_info.disk_with_central_directory {
            return Err(ZipError::UnsupportedArchive(
                "Support for multi-disk files is not implemented",
            ));
        }
        if file_capacity.saturating_mul(size_of::<ZipFileData>()) > isize::MAX as usize {
            return Err(ZipError::UnsupportedArchive("Oversized central directory"));
        }
        let mut files = IndexMap::with_capacity(file_capacity);
        let mut chain = Vec::with_capacity(file_capacity);
        let mut central_header_start = dir_info.directory_start;
        reader.seek(SeekFrom::Start(central_header_start))?;
        while chain.len() < dir_info.number_of_files {
            if let Some(run) = cd_chains.runs.get(&central_header_start)
                && chain.len() + run.valid < dir_info.number_of_files
            {
                let run = run.clone();
                return Err(cd_chains.fail(&chain, run));
            }
            match Self::read_one_central_header(reader, central_header_start) {
                Ok((file_name_raw, file, next)) => {
                    chain.push(central_header_start);
                    files.insert(file_name_raw, file);
                    central_header_start = next;
                }
                Err(e) => {
                    let failed_here = FailedRun { valid: 0, error: e };
                    cd_chains
                        .runs
                        .insert(central_header_start, failed_here.clone());
                    return Err(cd_chains.fail(&chain, failed_here));
                }
            }
        }
        for file in files.values_mut() {
            file.header_start = file
                .header_start
                .checked_add(dir_info.archive_offset)
                .ok_or_else(|| invalid!("Archive header is too large"))?;
        }
        Ok(SharedBuilder {
            files,
            raw_file_count: chain.len(),
            offset: dir_info.archive_offset,
            dir_start: dir_info.directory_start,
            config,
        })
    }
    pub fn new(reader: R) -> ZipResult<ZipArchive<R>> {
        Self::with_config(Config::default(), reader)
    }
    pub fn metadata(&self) -> Arc<ZipArchiveMetadata> {
        self.shared.clone()
    }

    pub fn with_config(config: Config, mut reader: R) -> ZipResult<ZipArchive<R>> {
        let shared = Self::get_metadata(config, &mut reader)?;
        Ok(ZipArchive {
            reader,
            shared: Arc::new(shared),
        })
    }
    pub fn len(&self) -> usize {
        self.shared.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[must_use]
    pub fn has_duplicate_names(&self) -> bool {
        self.shared.has_duplicate_names()
    }
    pub fn offset(&self) -> u64 {
        self.shared.offset
    }
    pub fn comment(&self) -> &[u8] {
        &self.shared.comment
    }

    pub fn file_names(&self) -> impl Iterator<Item = ZipResult<Cow<'_, str>>> {
        self.shared
            .files
            .iter()
            .map(|(file_name_raw, data)| data.name(file_name_raw))
    }
    pub fn file_names_raw(&self) -> impl Iterator<Item = &[u8]> {
        self.shared
            .files
            .iter()
            .map(|(file_name_raw, _data)| file_name_raw.as_ref())
    }

    pub fn has_overlapping_entries(&mut self) -> ZipResult<bool> {
        self.has_overlap(true)
    }
    fn has_overlap(&mut self, include_header: bool) -> ZipResult<bool> {
        let mut ranges = Vec::<Range<u64>>::with_capacity(self.shared.files.len());
        for file in self.shared.files.values() {
            if !include_header && file.compressed_size == 0 {
                continue;
            }
            let data_start = file.data_start(&mut self.reader)?;
            let start = if include_header {
                file.header_start
            } else {
                data_start
            };
            let end =
                data_start
                    .checked_add(file.compressed_size)
                    .ok_or(ZipError::InvalidArchive(Cow::Borrowed(
                        "File data end offset overflows u64",
                    )))?;
            ranges.push(start..end);
        }
        ranges.sort_unstable_by_key(|r| (r.start, r.end));
        let overlaps_central_directory = ranges.iter().any(|r| r.end > self.shared.dir_start);
        Ok(overlaps_central_directory || ranges.windows(2).any(|w| w[0].end > w[1].start))
    }

    pub fn by_name(&mut self, name: &str) -> ZipResult<ZipFile<'_, R>> {
        self.by_name_with_optional_password(name, None)
    }
    #[inline]
    pub fn index_for_name(&self, name: &str) -> Option<usize> {
        self.shared.files.get_index_of(name.as_bytes()).or_else(|| {
            self.shared
                .files
                .iter()
                .position(|(raw, data)| data.name(raw).is_ok_and(|decoded| decoded == name))
        })
    }

    pub fn by_path<T: AsRef<Path>>(&mut self, path: T) -> ZipResult<ZipFile<'_, R>> {
        self.index_for_path(path)
            .ok_or(ZipError::FileNotFound)
            .and_then(|index| self.by_index_with_options(index, ZipReadOptions::new()))
    }
    #[inline(always)]
    pub fn index_for_path<T: AsRef<Path>>(&self, path: T) -> Option<usize> {
        let path_as_string = &path_to_string(path).ok()?;
        self.index_for_name(path_as_string)
    }

    pub fn by_name_seek(&mut self, name: &str) -> ZipResult<ZipFileSeek<'_, R>> {
        self.by_index_seek(self.index_for_name(name).ok_or(ZipError::FileNotFound)?)
    }
    pub fn by_index_seek(&mut self, index: usize) -> ZipResult<ZipFileSeek<'_, R>> {
        let reader = &mut self.reader;
        self.shared
            .files
            .get_index(index)
            .ok_or(ZipError::FileNotFound)
            .and_then(move |(file_name_raw, data)| {
                let seek_reader = match data.compression_method {
                    CompressionMethod::Stored => {
                        ZipFileSeekReader::Raw(data.find_content_seek(reader)?)
                    }
                    _ => {
                        return Err(ZipError::UnsupportedArchive(
                            "Seekable compressed files are not yet supported",
                        ));
                    }
                };
                Ok(ZipFileSeek {
                    file_name_raw: Cow::Borrowed(file_name_raw),
                    reader: seek_reader,
                    data: Cow::Borrowed(data),
                })
            })
    }
    fn by_name_with_optional_password<'a>(
        &'a mut self,
        name: &str,
        password: Option<&[u8]>,
    ) -> ZipResult<ZipFile<'a, R>> {
        let Some(index) = self.index_for_name(name) else {
            return Err(ZipError::FileNotFound);
        };
        self.by_index_with_options(index, ZipReadOptions::new().password(password))
    }

    pub fn by_index(&mut self, file_number: usize) -> ZipResult<ZipFile<'_, R>> {
        self.by_index_with_options(file_number, ZipReadOptions::new())
    }

    pub fn by_index_raw(&mut self, file_number: usize) -> ZipResult<ZipFile<'_, R>> {
        let reader = &mut self.reader;
        let (file_name_raw, data) = self
            .shared
            .files
            .get_index(file_number)
            .ok_or(ZipError::FileNotFound)?;
        Ok(ZipFile {
            file_name_raw: Cow::Borrowed(file_name_raw),
            reader: ZipFileReader::Raw(data.find_content(reader)?),
            data: Cow::Borrowed(data),
        })
    }

    pub fn by_index_with_options(
        &mut self,
        file_number: usize,
        options: ZipReadOptions<'_>,
    ) -> ZipResult<ZipFile<'_, R>> {
        let (file_name_raw, data) = self
            .shared
            .files
            .get_index(file_number)
            .ok_or(ZipError::FileNotFound)?;
        if options.enforce_header_consistency {
            if self.shared.has_duplicate_names() {
                return Err(invalid!(
                    "header inconsistency detected: duplicate file names in central directory"
                ));
            }
            data.validate_header_consistency(&mut self.reader, file_name_raw)?;
        }
        let limit_reader = data.find_content(&mut self.reader)?;
        let reader = data.make_reader(limit_reader, options)?;
        Ok(ZipFile {
            file_name_raw: Cow::Borrowed(file_name_raw),
            data: Cow::Borrowed(data),
            reader,
        })
    }
    pub fn validate_header_consistency(&mut self) -> ZipResult<()> {
        if self.shared.has_duplicate_names() {
            return Err(invalid!(
                "header inconsistency detected: duplicate file names in central directory"
            ));
        }
        for (file_name_raw, data) in &self.shared.files {
            data.validate_header_consistency(&mut self.reader, file_name_raw)?;
        }
        if self.has_overlapping_entries()? {
            return Err(invalid!(
                "header inconsistency detected: archive contains overlapping entries"
            ));
        }
        Ok(())
    }
    pub fn into_inner(self) -> R {
        self.reader
    }
}
