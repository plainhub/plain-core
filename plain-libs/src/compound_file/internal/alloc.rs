use crate::compound_file::internal::{Chain, Sector, Sectors, consts};
use fnv::FnvHashSet;
use std::io::{self, Read, Seek};

//===========================================================================//

macro_rules! malformed {
    ($e:expr) => { invalid_data!("Malformed FAT ({})", $e) };
    ($fmt:expr, $($arg:tt)+) => {
        invalid_data!("Malformed FAT ({})", format!($fmt, $($arg)+))
    };
}

//===========================================================================//

/// A wrapper around the sectors of a compound file, providing sector
/// allocation via the FAT and DIFAT.
pub struct Allocator<F> {
    sectors: Sectors<F>,
    difat_sector_ids: Vec<u32>,
    difat: Vec<u32>,
    fat: Vec<u32>,
    /// The sector IDs of the most recently used chain, keyed by its start
    /// sector.  A `Stream` opens its chain anew for every buffer refill,
    /// and walking a long FAT chain from the start each time made reading
    /// or writing a stream of `n` sectors cost `O(n^2)`.  The list is
    /// handed to the next `Chain` opened for the same start sector, which
    /// gives it back when dropped; any FAT change discards it.
    chain_cache: Option<(u32, Vec<u32>)>,
}

impl<F> Allocator<F> {
    pub fn new(
        sectors: Sectors<F>,
        difat_sector_ids: Vec<u32>,
        difat: Vec<u32>,
        fat: Vec<u32>,
    ) -> io::Result<Allocator<F>> {
        let mut alloc = Allocator {
            sectors,
            difat_sector_ids,
            difat,
            fat,
            chain_cache: None,
        };
        alloc.validate()?;
        Ok(alloc)
    }

    pub fn sector_len(&self) -> usize {
        self.sectors.sector_len()
    }

    pub fn next(&self, sector_id: u32) -> io::Result<u32> {
        let index = sector_id as usize;
        if index >= self.fat.len() {
            invalid_data!(
                "Found reference to sector {}, but FAT has only {} entries",
                index,
                self.fat.len()
            );
        }
        let next_id = self.fat[index];
        if next_id != consts::END_OF_CHAIN
            && (next_id > consts::MAX_REGULAR_SECTOR || next_id as usize >= self.fat.len())
        {
            invalid_data!("next_id ({}) is invalid", next_id);
        }
        Ok(next_id)
    }

    pub fn open_chain(&mut self, start_sector_id: u32) -> io::Result<Chain<'_, F>> {
        match self.chain_cache.take() {
            Some((cached_start, sector_ids)) if cached_start == start_sector_id => {
                Ok(Chain::from_sector_ids(self, sector_ids))
            }
            _ => Chain::new(self, start_sector_id),
        }
    }

    /// Remembers the sector IDs of the chain starting at `start_sector_id`
    /// so that the next `open_chain` for it need not walk the FAT again.
    /// The IDs must reflect the current FAT.
    pub(crate) fn cache_chain(&mut self, start_sector_id: u32, sector_ids: Vec<u32>) {
        debug_assert_eq!(sector_ids.first(), Some(&start_sector_id));
        self.chain_cache = Some((start_sector_id, sector_ids));
    }

    fn validate(&mut self) -> io::Result<()> {
        if self.fat.len() > self.sectors.num_sectors() as usize {
            malformed!(
                "FAT has {} entries, but file has only {} sectors",
                self.fat.len(),
                self.sectors.num_sectors()
            );
        }
        for &difat_sector in self.difat_sector_ids.iter() {
            let difat_sector_index = difat_sector as usize;
            let Some(sector) = self.fat.get_mut(difat_sector_index) else {
                malformed!(
                    "FAT has {} entries, but DIFAT lists {} as a DIFAT sector",
                    self.fat.len(),
                    difat_sector
                );
            };
            *sector = consts::DIFAT_SECTOR;
        }
        for &fat_sector in self.difat.iter() {
            let fat_sector_index = fat_sector as usize;
            let Some(sector) = self.fat.get_mut(fat_sector_index) else {
                malformed!(
                    "FAT has {} entries, but DIFAT lists {} as a FAT sector",
                    self.fat.len(),
                    fat_sector
                );
            };
            *sector = consts::FAT_SECTOR;
        }
        let mut pointees = FnvHashSet::default();
        for from_sector in 0..self.fat.len() {
            let to_sector = self.fat[from_sector];
            if to_sector <= consts::MAX_REGULAR_SECTOR {
                if to_sector as usize >= self.fat.len() {
                    // End the chain here instead, so that only a chain
                    // that actually runs through this sector is affected
                    // (it gets truncated), rather than the whole file.
                    self.fat[from_sector] = consts::END_OF_CHAIN;
                    continue;
                }
                if pointees.contains(&to_sector) {
                    malformed!("sector {} pointed to twice", to_sector);
                }
                pointees.insert(to_sector);
            } else if to_sector == consts::INVALID_SECTOR {
                malformed!("0x{:08X} is not a valid FAT entry", to_sector);
            }
        }

        Ok(())
    }
}

impl<F: Read + Seek> Allocator<F> {
    /// Reads from `offset_within_sector` bytes into sector `sector_id` and
    /// on through the sectors after it in the file; see
    /// `Sectors::read_contiguous`.
    pub fn read_contiguous(
        &mut self,
        sector_id: u32,
        offset_within_sector: u64,
        buf: &mut [u8],
    ) -> io::Result<usize> {
        self.sectors
            .read_contiguous(sector_id, offset_within_sector, buf)
    }
}

impl<F: Seek> Allocator<F> {
    pub fn seek_to_sector(&mut self, sector_id: u32) -> io::Result<Sector<'_, F>> {
        self.sectors.seek_to_sector(sector_id)
    }

    pub fn seek_within_sector(
        &mut self,
        sector_id: u32,
        offset_within_sector: u64,
    ) -> io::Result<Sector<'_, F>> {
        self.sectors
            .seek_within_sector(sector_id, offset_within_sector)
    }
}
