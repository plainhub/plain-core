use crate::compound_file::internal::{MiniAllocator, consts};
use std::io::{self, Read, Seek, SeekFrom};

//===========================================================================//

pub struct MiniChain<'a, F: 'a> {
    minialloc: &'a mut MiniAllocator<F>,
    sector_ids: Vec<u32>,
    offset_from_start: u64,
}

impl<'a, F> MiniChain<'a, F> {
    pub fn new(
        minialloc: &'a mut MiniAllocator<F>,
        start_sector_id: u32,
    ) -> io::Result<MiniChain<'a, F>> {
        let mut sector_ids = Vec::<u32>::new();
        let mut current_sector_id = start_sector_id;
        let first_sector_id = start_sector_id;
        while current_sector_id != consts::END_OF_CHAIN {
            sector_ids.push(current_sector_id);
            current_sector_id = minialloc.next_mini_sector(current_sector_id)?;
            if current_sector_id == first_sector_id {
                invalid_data!(
                    "Minichain contained duplicate sector id {}",
                    current_sector_id
                );
            }
        }
        Ok(MiniChain {
            minialloc,
            sector_ids,
            offset_from_start: 0,
        })
    }

    pub fn len(&self) -> u64 {
        (consts::MINI_SECTOR_LEN as u64) * (self.sector_ids.len() as u64)
    }

    /// How many mini sectors from `index` on are consecutive within one
    /// regular sector (and so can be read or written in one go), looking
    /// no further than needed to cover `wanted` bytes from
    /// `offset_within_sector` into the first.
    fn run_len(&self, index: usize, offset_within_sector: u64, wanted: usize) -> usize {
        let mini_sector_len = consts::MINI_SECTOR_LEN as u64;
        let per_sector = (self.minialloc.sector_len() / consts::MINI_SECTOR_LEN) as u32;
        let first = self.sector_ids[index];
        let mut run = 1;
        while index + run < self.sector_ids.len()
            && self.sector_ids[index + run] == self.sector_ids[index + run - 1] + 1
            && self.sector_ids[index + run] / per_sector == first / per_sector
            && run as u64 * mini_sector_len - offset_within_sector < wanted as u64
        {
            run += 1;
        }
        run
    }
}

impl<'a, F> Seek for MiniChain<'a, F> {
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

impl<'a, F: Read + Seek> Read for MiniChain<'a, F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let total_len = self.len();
        debug_assert!(self.offset_from_start <= total_len);
        let remaining_in_chain = total_len - self.offset_from_start;
        let max_len = remaining_in_chain.min(buf.len() as u64) as usize;
        if max_len == 0 {
            return Ok(0);
        }
        let sector_len = consts::MINI_SECTOR_LEN as u64;
        let current_sector_index = (self.offset_from_start / sector_len) as usize;
        debug_assert!(current_sector_index < self.sector_ids.len());
        let current_sector_id = self.sector_ids[current_sector_index];
        let offset_within_sector = self.offset_from_start % sector_len;
        // Read through as many consecutive mini sectors as the buffer
        // covers.
        let run = self.run_len(current_sector_index, offset_within_sector, max_len);
        let run_len = run as u64 * sector_len - offset_within_sector;
        let max_len = max_len.min(run_len as usize);
        let mut sector = self
            .minialloc
            .seek_within_mini_sector(current_sector_id, offset_within_sector)?;
        let bytes_read = sector.read(&mut buf[0..max_len])?;
        self.offset_from_start += bytes_read as u64;
        debug_assert!(self.offset_from_start <= total_len);
        Ok(bytes_read)
    }
}
