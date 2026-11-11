use crate::zip::format::blocks::{
    CentralDirectoryEndInfo, DataAndPosition, Zip32CentralDirectoryEnd, Zip64CDELocatorBlock,
    Zip64CentralDirectoryEnd, Zip64CentralDirectoryEndLocator, ZipCentralEntryBlock,
};
use crate::zip::format::magic::Magic;
use crate::zip::read::ArchiveOffset;
use crate::zip::read::magic_finder::{Backwards, Forward, MagicFinder};
use crate::zip::result::{ZipResult, invalid};
use core::mem;
use std::io::{Read, Seek, SeekFrom};
const MAGIC_LENGTH: usize = mem::size_of::<Magic>();
fn probe_magic_at<R: Read + Seek + ?Sized>(
    reader: &mut R,
    pos: u64,
    magic: &[u8; MAGIC_LENGTH],
) -> ZipResult<bool> {
    reader.seek(SeekFrom::Start(pos))?;
    let mut buf = [0u8; MAGIC_LENGTH];
    match reader.read_exact(&mut buf) {
        Ok(()) => Ok(buf == *magic),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e.into()),
    }
}
#[derive(Debug)]
pub(crate) struct MagicPositionCache {
    positions: Vec<u64>,
    scanned_ranges: Vec<(u64, u64)>,
    magic_bytes: [u8; MAGIC_LENGTH],
}
impl MagicPositionCache {
    pub(crate) fn new(magic_bytes: [u8; MAGIC_LENGTH]) -> Self {
        Self {
            positions: Vec::new(),
            scanned_ranges: Vec::new(),
            magic_bytes,
        }
    }
    pub(crate) fn ensure_scanned_range<R: Read + Seek + ?Sized>(
        &mut self,
        reader: &mut R,
        start: u64,
        end: u64,
    ) -> ZipResult<()> {
        if start >= end {
            return Ok(());
        }
        let mut uncovered = Vec::new();
        let mut cur = start;
        for &(r_start, r_end) in &self.scanned_ranges {
            if cur >= end {
                break;
            }
            if cur < r_start {
                let scan_end = end.min(r_start);
                uncovered.push((cur, scan_end));
                cur = scan_end;
            }
            if cur < r_end {
                cur = r_end;
            }
        }
        if cur < end {
            uncovered.push((cur, end));
        }
        for (u_start, u_end) in uncovered {
            self.scan_uncovered(reader, u_start, u_end)?;
        }
        self.merge_scanned_range(start, end);
        Ok(())
    }
    fn scan_uncovered<R: Read + Seek + ?Sized>(
        &mut self,
        reader: &mut R,
        start: u64,
        end: u64,
    ) -> ZipResult<()> {
        let magic_overlap = MAGIC_LENGTH.saturating_sub(1) as u64;
        let scan_start = start.saturating_sub(magic_overlap);
        let mut finder = MagicFinder::<Forward<'_>>::new(&self.magic_bytes, scan_start, end);
        while let Some(pos) = finder.next(reader)? {
            if let Err(idx) = self.positions.binary_search(&pos) {
                self.positions.insert(idx, pos);
            }
        }
        Ok(())
    }
    fn merge_scanned_range(&mut self, start: u64, end: u64) {
        self.scanned_ranges.push((start, end));
        self.scanned_ranges.sort_unstable_by_key(|&(s, _)| s);
        let mut merged: Vec<(u64, u64)> = Vec::with_capacity(self.scanned_ranges.len());
        for (s, e) in self.scanned_ranges.drain(..) {
            if let Some(last) = merged.last_mut()
                && s <= last.1
            {
                last.1 = last.1.max(e);
                continue;
            }
            merged.push((s, e));
        }
        self.scanned_ranges = merged;
    }
    pub(crate) fn first_in_range<R: Read + Seek + ?Sized>(
        &mut self,
        reader: &mut R,
        start: u64,
        end: u64,
    ) -> ZipResult<Option<u64>> {
        self.ensure_scanned_range(reader, start, end)?;
        let idx = self.positions.partition_point(|&p| p < start);
        if idx < self.positions.len() && self.positions[idx] < end {
            Ok(Some(self.positions[idx]))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn contains<R: Read + Seek + ?Sized>(
        &mut self,
        reader: &mut R,
        pos: u64,
        _end: u64,
    ) -> ZipResult<bool> {
        if self.positions.binary_search(&pos).is_ok() {
            return Ok(true);
        }
        let found = probe_magic_at(reader, pos, &self.magic_bytes)?;
        if found {
            let idx = self.positions.partition_point(|&p| p < pos);
            self.positions.insert(idx, pos);
        }
        Ok(found)
    }
}
pub(crate) fn find_central_directory_end<R: Read + Seek + ?Sized>(
    reader: &mut R,
    archive_offset: ArchiveOffset,
    file_len: u64,
    eocd_finder: &mut MagicFinder<Backwards<'static>>,
    cdfh_cache: &mut MagicPositionCache,
    eocd64_cache: &mut MagicPositionCache,
) -> ZipResult<CentralDirectoryEndInfo> {
    const EOCD_FIXED_SIZE: u64 = 22;
    const CDFH_SIG_BYTES: [u8; mem::size_of::<Magic>()] =
        Magic::CENTRAL_DIRECTORY_HEADER_SIGNATURE.to_le_bytes();
    let mut parsing_error = None;
    while let Some(eocd_offset) = eocd_finder.next(reader)? {
        let eocd = match Zip32CentralDirectoryEnd::parse(reader) {
            Ok(eocd) => eocd,
            Err(e) => {
                if parsing_error.is_none() {
                    parsing_error = Some(e);
                }
                continue;
            }
        };
        if eocd.zip_file_comment.len() as u64 + eocd_offset + EOCD_FIXED_SIZE > file_len {
            parsing_error = Some(invalid!("Invalid EOCD comment length"));
            continue;
        }
        let zip64_metadata = if eocd.may_be_zip64() {
            fn try_read_eocd64_locator(
                reader: &mut (impl Read + Seek + ?Sized),
                eocd_offset: u64,
            ) -> ZipResult<(u64, Zip64CentralDirectoryEndLocator)> {
                if eocd_offset
                    < (mem::size_of::<Magic>() + mem::size_of::<Zip64CDELocatorBlock>()) as u64
                {
                    return Err(invalid!("EOCD64 Locator does not fit in file"));
                }
                let locator64_offset = eocd_offset
                    - (mem::size_of::<Magic>() + mem::size_of::<Zip64CDELocatorBlock>()) as u64;
                reader.seek(SeekFrom::Start(locator64_offset))?;
                let locator64 = Zip64CentralDirectoryEndLocator::parse(reader);
                Ok((locator64_offset, locator64?))
            }
            try_read_eocd64_locator(reader, eocd_offset).ok()
        } else {
            None
        };
        let Some((locator64_offset, locator64)) = zip64_metadata else {
            if eocd.disk_number != 0
                || eocd.disk_with_central_directory != 0
                || eocd.number_of_files_on_this_disk != eocd.number_of_files
            {
                parsing_error = Some(invalid!("Multi-disk ZIP files are not supported"));
                continue;
            }
            let cd_offset_from_eocd = u64::from(eocd.central_directory_offset);
            if eocd.number_of_files == 0 {
                if cd_offset_from_eocd > eocd_offset {
                    parsing_error = Some(invalid!("Invalid CDFH offset in EOCD"));
                    continue;
                }
                return Ok(CentralDirectoryEndInfo {
                    eocd: (eocd, eocd_offset).into(),
                    eocd64: None,
                    archive_offset: eocd_offset.saturating_sub(cd_offset_from_eocd),
                });
            }
            if cd_offset_from_eocd >= eocd_offset {
                parsing_error = Some(invalid!("Invalid CDFH offset in EOCD"));
                continue;
            }
            let cd_result = match archive_offset {
                ArchiveOffset::Known(n) => {
                    let guess = cd_offset_from_eocd.saturating_add(n).min(eocd_offset);
                    if guess + MAGIC_LENGTH as u64 <= eocd_offset
                        && probe_magic_at(reader, guess, &CDFH_SIG_BYTES)?
                    {
                        Some(guess)
                    } else {
                        None
                    }
                }
                _ => {
                    let abutting = eocd_offset
                        .checked_sub(u64::from(eocd.central_directory_size))
                        .filter(|&start| start >= cd_offset_from_eocd);
                    match abutting {
                        Some(start) if cdfh_cache.contains(reader, start, eocd_offset)? => {
                            Some(start)
                        }
                        _ => cdfh_cache.first_in_range(reader, cd_offset_from_eocd, eocd_offset)?,
                    }
                }
            };
            if let Some(cd_offset) = cd_result {
                let archive_offset = cd_offset - cd_offset_from_eocd;
                return Ok(CentralDirectoryEndInfo {
                    eocd: (eocd, eocd_offset).into(),
                    eocd64: None,
                    archive_offset,
                });
            }
            parsing_error = Some(invalid!("No CDFH found"));
            continue;
        };
        if locator64.end_of_central_directory_offset >= locator64_offset {
            parsing_error = Some(invalid!("Invalid EOCD64 Locator CD offset"));
            continue;
        }
        if locator64.number_of_disks > 1 {
            parsing_error = Some(invalid!("Multi-disk ZIP files are not supported"));
            continue;
        }
        fn try_read_eocd64<R: Read + Seek + ?Sized>(
            reader: &mut R,
            disk_with_central_directory: u32,
            expected_length: u64,
        ) -> ZipResult<Zip64CentralDirectoryEnd> {
            let z64 = Zip64CentralDirectoryEnd::parse(reader, expected_length)?;
            if z64.disk_with_central_directory != disk_with_central_directory {
                return Err(invalid!("Invalid EOCD64: inconsistency with Locator data"));
            }
            if z64.disk_number != 0 || z64.number_of_files_on_this_disk != z64.number_of_files {
                return Err(invalid!("Multi-disk ZIP files are not supported"));
            }
            if z64.record_size + Zip64CentralDirectoryEnd::RECORD_OVERHEAD != expected_length {
                return Err(invalid!("Invalid EOCD64: inconsistent length"));
            }
            Ok(z64)
        }
        let search_start = locator64.end_of_central_directory_offset;
        let search_end = locator64_offset;
        let guess = match archive_offset {
            ArchiveOffset::Known(n) => search_start.saturating_add(n).min(search_end),
            ArchiveOffset::Detect => search_start,
        };
        let guess_result = if guess + MAGIC_LENGTH as u64 <= search_end
            && probe_magic_at(reader, guess, &eocd64_cache.magic_bytes)?
        {
            Some(guess)
        } else {
            None
        };
        let mut local_error = None;
        fn try_candidate<R: Read + Seek + ?Sized>(
            reader: &mut R,
            eocd_offset: u64,
            eocd64_offset: u64,
            disk_with_central_directory: u32,
            locator64_offset: u64,
        ) -> ZipResult<Option<(DataAndPosition<Zip64CentralDirectoryEnd>, u64)>> {
            let Some(archive_offset) = eocd64_offset.checked_sub(eocd_offset) else {
                return Ok(None);
            };
            reader.seek(SeekFrom::Start(eocd64_offset))?;
            let eocd64 = try_read_eocd64(
                reader,
                disk_with_central_directory,
                locator64_offset.saturating_sub(eocd64_offset),
            )?;
            if eocd64_offset
                < eocd64
                    .number_of_files
                    .saturating_mul(
                        (mem::size_of::<Magic>() + mem::size_of::<ZipCentralEntryBlock>()) as u64,
                    )
                    .saturating_add(eocd64.central_directory_offset)
            {
                return Ok(None);
            }
            Ok(Some(((eocd64, eocd64_offset).into(), archive_offset)))
        }
        if let Some(guess_pos) = guess_result {
            match try_candidate(
                reader,
                locator64.end_of_central_directory_offset,
                guess_pos,
                locator64.disk_with_central_directory,
                locator64_offset,
            ) {
                Ok(Some((eocd64, archive_offset))) => {
                    return Ok(CentralDirectoryEndInfo {
                        eocd: (eocd, eocd_offset).into(),
                        eocd64: Some(eocd64),
                        archive_offset,
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    local_error = Some(e);
                }
            }
            if matches!(archive_offset, ArchiveOffset::Known(_)) {
                parsing_error = local_error.or(Some(invalid!("Could not find EOCD64")));
                continue;
            }
        }
        if !matches!(archive_offset, ArchiveOffset::Known(_)) {
            eocd64_cache.ensure_scanned_range(reader, search_start, search_end)?;
            let start_idx = eocd64_cache
                .positions
                .partition_point(|&p| p < search_start);
            let end_idx = eocd64_cache.positions.partition_point(|&p| p < search_end);
            for &eocd64_offset in &eocd64_cache.positions[start_idx..end_idx] {
                match try_candidate(
                    reader,
                    locator64.end_of_central_directory_offset,
                    eocd64_offset,
                    locator64.disk_with_central_directory,
                    locator64_offset,
                ) {
                    Ok(Some((eocd64, archive_offset))) => {
                        return Ok(CentralDirectoryEndInfo {
                            eocd: (eocd, eocd_offset).into(),
                            eocd64: Some(eocd64),
                            archive_offset,
                        });
                    }
                    Ok(None) => {
                        if local_error.is_none() {
                            local_error =
                                Some(invalid!("Invalid EOCD64: inconsistent number of files"));
                        }
                    }
                    Err(e) => {
                        local_error = Some(e);
                    }
                }
            }
        }
        parsing_error = local_error.or(Some(invalid!("Could not find EOCD64")));
    }
    Err(parsing_error.unwrap_or(invalid!("Could not find EOCD")))
}
