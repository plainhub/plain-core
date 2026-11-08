use crate::compound_file::internal::{Allocator, consts};
use std::cmp;
use std::io::{self, Read, Seek, SeekFrom};
use std::mem;

//===========================================================================//

pub struct Chain<'a, F: 'a> {
    allocator: &'a mut Allocator<F>,
    sector_ids: Vec<u32>,
    offset_from_start: u64,
}

impl<'a, F> Chain<'a, F> {
    pub fn new(allocator: &'a mut Allocator<F>, start_sector_id: u32) -> io::Result<Chain<'a, F>> {
        let mut sector_ids = Vec::<u32>::new();
        let mut current_sector_id = start_sector_id;
        let first_sector_id = start_sector_id;
        while current_sector_id != consts::END_OF_CHAIN {
            sector_ids.push(current_sector_id);
            current_sector_id = allocator.next(current_sector_id)?;
            if current_sector_id == first_sector_id {
                invalid_data!("Chain contained duplicate sector id {}", current_sector_id);
            }
        }
        Ok(Chain {
            allocator,
            sector_ids,
            offset_from_start: 0,
        })
    }

    /// Creates a chain from sector IDs that are known to match the FAT.
    pub(crate) fn from_sector_ids(
        allocator: &'a mut Allocator<F>,
        sector_ids: Vec<u32>,
    ) -> Chain<'a, F> {
        Chain {
            allocator,
            sector_ids,
            offset_from_start: 0,
        }
    }

    pub fn sector_ids(&self) -> &[u32] {
        &self.sector_ids
    }

    pub fn len(&self) -> u64 {
        (self.allocator.sector_len() as u64) * (self.sector_ids.len() as u64)
    }
}

impl<'a, F> Chain<'a, F> {
    /// How many sectors from `index` on are consecutive in the file (and so
    /// can be read or written in one go), looking no further than needed
    /// to cover `wanted` bytes from `offset_within_sector` into the first.
    fn run_len(&self, index: usize, offset_within_sector: u64, wanted: usize) -> usize {
        let sector_len = self.allocator.sector_len() as u64;
        let mut run = 1;
        while index + run < self.sector_ids.len()
            && self.sector_ids[index + run] == self.sector_ids[index + run - 1] + 1
            && run as u64 * sector_len - offset_within_sector < wanted as u64
        {
            run += 1;
        }
        run
    }
}

impl<'a, F> Drop for Chain<'a, F> {
    fn drop(&mut self) {
        // The list is kept in step with every change this chain made to the
        // FAT, so the next chain opened for the same start sector can reuse
        // it instead of walking the FAT again.
        if !self.sector_ids.is_empty() {
            let start_sector_id = self.sector_ids[0];
            self.allocator
                .cache_chain(start_sector_id, mem::take(&mut self.sector_ids));
        }
    }
}

impl<'a, F> Seek for Chain<'a, F> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let length = self.len();
        let new_offset = match pos {
            SeekFrom::Start(delta) => delta as i64,
            SeekFrom::End(delta) => delta + length as i64,
            SeekFrom::Current(delta) => delta + self.offset_from_start as i64,
        };
        if new_offset < 0 || (new_offset as u64) > length {
            invalid_input!(
                "Cannot seek to {}, chain length is {} bytes",
                new_offset,
                length
            );
        }
        self.offset_from_start = new_offset as u64;
        Ok(self.offset_from_start)
    }
}

impl<'a, F: Read + Seek> Read for Chain<'a, F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let total_len = self.len();
        debug_assert!(self.offset_from_start <= total_len);
        let remaining_in_chain = total_len - self.offset_from_start;
        let max_len = cmp::min(buf.len() as u64, remaining_in_chain) as usize;
        if max_len == 0 {
            return Ok(0);
        }
        let sector_len = self.allocator.sector_len() as u64;
        let current_sector_index = (self.offset_from_start / sector_len) as usize;
        debug_assert!(current_sector_index < self.sector_ids.len());
        let current_sector_id = self.sector_ids[current_sector_index];
        let offset_within_sector = self.offset_from_start % sector_len;
        // Read through as many consecutive sectors as the buffer covers.
        let run = self.run_len(current_sector_index, offset_within_sector, max_len);
        let run_len = run as u64 * sector_len - offset_within_sector;
        let max_len = max_len.min(run_len as usize);
        let bytes_read = self.allocator.read_contiguous(
            current_sector_id,
            offset_within_sector,
            &mut buf[0..max_len],
        )?;
        self.offset_from_start += bytes_read as u64;
        debug_assert!(self.offset_from_start <= total_len);
        Ok(bytes_read)
    }
}
