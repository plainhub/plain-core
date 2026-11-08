use crate::compound_file::internal::Version;
use std::cmp;
use std::io::{self, Read, Seek, SeekFrom};

// ========================================================================= //

/// A wrapper around the underlying file of a CompoundFile struct, providing
/// access to individual sectors of the file.
pub struct Sectors<F> {
    inner: F,
    version: Version,
    num_sectors: u32,
}

impl<F> Sectors<F> {
    pub fn new(version: Version, inner_len: u64, inner: F) -> Sectors<F> {
        let sector_len = version.sector_len() as u64;
        debug_assert!(inner_len >= sector_len);
        let num_sectors = inner_len.div_ceil(sector_len) as u32 - 1;
        Sectors {
            inner,
            version,
            num_sectors,
        }
    }

    pub fn sector_len(&self) -> usize {
        self.version.sector_len()
    }

    pub fn num_sectors(&self) -> u32 {
        self.num_sectors
    }
}

impl<F: Seek> Sectors<F> {
    /// Seeks to `offset_within_sector` bytes into sector `sector_id`, having
    /// checked that the `len` bytes from there stay within the file.
    fn seek_contiguous(
        &mut self,
        sector_id: u32,
        offset_within_sector: u64,
        len: usize,
    ) -> io::Result<()> {
        let sector_len = self.sector_len() as u64;
        debug_assert!(offset_within_sector <= sector_len);
        let end = sector_id as u64 * sector_len + offset_within_sector + len as u64;
        let last_sector = end.saturating_sub(1) / sector_len;
        if last_sector >= self.num_sectors as u64 {
            invalid_data!(
                "Tried to access sector {}, but sector count is only {}",
                last_sector,
                self.num_sectors
            );
        }
        self.inner.seek(SeekFrom::Start(
            (sector_id + 1) as u64 * sector_len + offset_within_sector,
        ))?;
        Ok(())
    }

    pub fn seek_to_sector(&mut self, sector_id: u32) -> io::Result<Sector<'_, F>> {
        self.seek_within_sector(sector_id, 0)
    }

    pub fn seek_within_sector(
        &mut self,
        sector_id: u32,
        offset_within_sector: u64,
    ) -> io::Result<Sector<'_, F>> {
        debug_assert!(offset_within_sector <= self.sector_len() as u64);
        if sector_id >= self.num_sectors {
            invalid_data!(
                "Tried to seek to sector {}, but sector count is only {}",
                sector_id,
                self.num_sectors
            );
        }
        let sector_len = self.sector_len();
        self.inner.seek(SeekFrom::Start(
            (sector_id + 1) as u64 * sector_len as u64 + offset_within_sector,
        ))?;
        Ok(Sector {
            inner: &mut self.inner,
            sector_len,
            offset_within_sector: offset_within_sector as usize,
        })
    }
}

impl<F: Read + Seek> Sectors<F> {
    /// Reads into `buf` starting `offset_within_sector` bytes into sector
    /// `sector_id` and continuing through the sectors that follow it in the
    /// file, which the caller has established are the next ones of the
    /// chain being read.  Makes a single read of the underlying file, so
    /// may read fewer bytes than `buf` holds.
    pub fn read_contiguous(
        &mut self,
        sector_id: u32,
        offset_within_sector: u64,
        buf: &mut [u8],
    ) -> io::Result<usize> {
        self.seek_contiguous(sector_id, offset_within_sector, buf.len())?;
        self.inner.read(buf)
    }
}

pub struct Sector<'a, F: 'a> {
    inner: &'a mut F,
    sector_len: usize,
    offset_within_sector: usize,
}

impl<'a, F> Sector<'a, F> {
    /// Returns the total length of this sector.
    pub fn len(&self) -> usize {
        self.sector_len
    }

    fn remaining(&self) -> usize {
        debug_assert!(self.offset_within_sector <= self.len());
        self.len() - self.offset_within_sector
    }
}

impl<'a, F: Read> Read for Sector<'a, F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let max_len = cmp::min(buf.len(), self.remaining());
        if max_len == 0 {
            return Ok(0);
        }
        let bytes_read = self.inner.read(&mut buf[0..max_len])?;
        self.offset_within_sector += bytes_read;
        debug_assert!(self.offset_within_sector <= self.len());
        Ok(bytes_read)
    }
}

impl<'a, F: Seek> Seek for Sector<'a, F> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let old_offset = self.offset_within_sector as i64;
        let new_offset = match pos {
            SeekFrom::Start(delta) => delta as i64,
            SeekFrom::End(delta) => self.len() as i64 + delta,
            SeekFrom::Current(delta) => self.offset_within_sector as i64 + delta,
        };
        if new_offset < 0 || new_offset > self.len() as i64 {
            panic!("Internal error: cannot seek outside of sector");
        }
        self.inner
            .seek(SeekFrom::Current(new_offset - old_offset))?;
        self.offset_within_sector = new_offset as usize;
        Ok(new_offset as u64)
    }
}

// ========================================================================= //
