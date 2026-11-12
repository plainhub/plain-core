use std::io::{self, Seek};

use crate::fnv::FnvHashSet;

use crate::compound_file::internal::{Chain, DirEntry, Directory, MiniChain, Sector, consts};

//===========================================================================//

macro_rules! malformed {
    ($e:expr) => { invalid_data!("Malformed MiniFAT ({})", $e) };
    ($fmt:expr, $($arg:tt)+) => {
        invalid_data!("Malformed MiniFAT ({})", format!($fmt, $($arg)+))
    };
}

//===========================================================================//

/// A wrapper around the directory manager that additionally provides
/// mini-sector allocation via the MiniFAT.
pub struct MiniAllocator<F> {
    directory: Directory<F>,
    minifat: Vec<u32>,
    /// The sector IDs of the mini stream's chain, in order, once walked.
    /// Every access to a mini sector needs the regular sector it lives in;
    /// walking the FAT chain from the start on each access made reading or
    /// writing `n` small streams cost `O(n²)`. The chain only ever grows
    /// (see `append_mini_sector`), so it is walked once and extended in
    /// place.
    mini_stream_sectors: Option<Vec<u32>>,
}

impl<F> MiniAllocator<F> {
    pub fn new(directory: Directory<F>, minifat: Vec<u32>) -> io::Result<MiniAllocator<F>> {
        let mut minialloc = MiniAllocator {
            directory,
            minifat,
            mini_stream_sectors: None,
        };
        minialloc.validate()?;
        Ok(minialloc)
    }

    pub fn next_mini_sector(&self, sector_id: u32) -> io::Result<u32> {
        let index = sector_id as usize;
        if index >= self.minifat.len() {
            invalid_data!(
                "Found reference to mini sector {}, but MiniFAT has only {} \
                 entries",
                index,
                self.minifat.len()
            );
        }
        let next_id = self.minifat[index];
        if next_id != consts::END_OF_CHAIN
            && (next_id > consts::MAX_REGULAR_SECTOR || next_id as usize >= self.minifat.len())
        {
            invalid_data!("next_id ({}) is invalid", next_id);
        }
        Ok(next_id)
    }

    pub fn sector_len(&self) -> usize {
        self.directory.sector_len()
    }

    pub fn stream_id_for_name_chain(&self, names: &[&str]) -> Option<u32> {
        self.directory.stream_id_for_name_chain(names)
    }

    pub fn open_chain(&mut self, start_sector_id: u32) -> io::Result<Chain<'_, F>> {
        self.directory.open_chain(start_sector_id)
    }

    pub fn open_mini_chain(&mut self, start_sector_id: u32) -> io::Result<MiniChain<'_, F>> {
        MiniChain::new(self, start_sector_id)
    }

    pub fn dir_entry(&self, stream_id: u32) -> &DirEntry {
        self.directory.dir_entry(stream_id)
    }

    fn validate(&mut self) -> io::Result<()> {
        let root_entry = self.directory.root_dir_entry();
        let root_stream_mini_sectors = root_entry.stream_len / (consts::MINI_SECTOR_LEN as u64);
        if root_stream_mini_sectors < (self.minifat.len() as u64) {
            self.minifat.truncate(root_stream_mini_sectors as usize);
        }
        let mut pointees = FnvHashSet::default();
        for (from_mini_sector, &to_mini_sector) in self.minifat.iter().enumerate() {
            if to_mini_sector <= consts::MAX_REGULAR_SECTOR {
                if to_mini_sector as usize >= self.minifat.len() {
                    malformed!(
                        "MiniFAT has {} entries, but mini sector {} points to \
                         {}",
                        self.minifat.len(),
                        from_mini_sector,
                        to_mini_sector
                    );
                }
                if pointees.contains(&to_mini_sector) {
                    malformed!("mini sector {} pointed to twice", to_mini_sector);
                }
                pointees.insert(to_mini_sector);
            }
        }

        Ok(())
    }
}

impl<F: Seek> MiniAllocator<F> {
    /// Returns the sector IDs of the chain starting at `start_sector_id`,
    /// walking it once and caching the result in `cache`.
    fn cached_chain_sectors<'a>(
        directory: &mut Directory<F>,
        cache: &'a mut Option<Vec<u32>>,
        start_sector_id: u32,
    ) -> io::Result<&'a [u32]> {
        if cache.is_none() {
            let sector_ids = if start_sector_id == consts::END_OF_CHAIN {
                Vec::new()
            } else {
                directory.open_chain(start_sector_id)?.sector_ids().to_vec()
            };
            *cache = Some(sector_ids);
        }
        Ok(cache.as_deref().unwrap())
    }

    /// The sector IDs of the mini stream's chain, in order.
    fn mini_stream_sectors(&mut self) -> io::Result<&[u32]> {
        let start_sector = self.directory.root_dir_entry().start_sector;
        Self::cached_chain_sectors(
            &mut self.directory,
            &mut self.mini_stream_sectors,
            start_sector,
        )
    }

    /// Seeks to `offset_within_mini_sector` bytes into mini sector
    /// `mini_sector`.  The returned sector runs to the end of the regular
    /// sector the mini sector lives in, so consecutive mini sectors that
    /// share a regular sector can be read or written in one go.
    pub fn seek_within_mini_sector(
        &mut self,
        mini_sector: u32,
        offset_within_mini_sector: u64,
    ) -> io::Result<Sector<'_, F>> {
        debug_assert!(offset_within_mini_sector < consts::MINI_SECTOR_LEN as u64);
        let mini_sectors_per_sector =
            (self.directory.sector_len() / consts::MINI_SECTOR_LEN) as u32;
        let sector_index = (mini_sector / mini_sectors_per_sector) as usize;
        let mini_sector_within_sector = mini_sector % mini_sectors_per_sector;
        let sector_id = self
            .mini_stream_sectors()?
            .get(sector_index)
            .copied()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid sector id"))?;
        self.directory.seek_within_sector(
            sector_id,
            mini_sector_within_sector as u64 * consts::MINI_SECTOR_LEN as u64
                + offset_within_mini_sector,
        )
    }
}
