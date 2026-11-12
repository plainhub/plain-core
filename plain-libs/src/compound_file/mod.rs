// Read-only compound file parser adapted from cfb 0.15.0 (SOURCE.json).
use crate::fnv::FnvHashSet;
use std::{
    io::{self, Read, Seek, SeekFrom},
    mem::size_of,
    path::Path,
};

#[macro_use]
mod internal;

use internal::{Allocator, DirEntry, Directory, Header, MiniAllocator, ObjType, Sectors, consts};

pub struct CompoundFile<F> {
    minialloc: MiniAllocator<F>,
}

impl<F: Read + Seek> CompoundFile<F> {
    pub fn open(mut inner: F) -> io::Result<CompoundFile<F>> {
        let inner_len = inner.seek(SeekFrom::End(0))?;
        if inner_len < consts::HEADER_LEN as u64 {
            invalid_data!("Invalid CFB file ({} bytes is too small)", inner_len);
        }
        inner.seek(SeekFrom::Start(0))?;

        // 2.2 Compound File Header
        let header = Header::read_from(&mut inner)?;
        // Major Version
        let sector_len = header.version.sector_len();
        if inner_len > (consts::MAX_REGULAR_SECTOR as u64 + 1) * (sector_len as u64) {
            invalid_data!("Invalid CFB file ({} bytes is too large)", inner_len);
        }

        if inner_len < header.version.sector_len() as u64 {
            invalid_data!(
                "Invalid CFB file (length of {} < sector length of {})",
                inner_len,
                header.version.sector_len()
            );
        }
        let mut sectors = Sectors::new(header.version, inner_len, inner);
        let num_sectors = sectors.num_sectors();

        // Read in DIFAT.
        let mut difat = Vec::<u32>::new();
        difat.extend_from_slice(&header.initial_difat_entries);
        let mut seen_sector_ids = FnvHashSet::default();
        let mut difat_sector_ids = Vec::new();
        let mut current_difat_sector = header.first_difat_sector;
        while current_difat_sector != consts::END_OF_CHAIN
            && current_difat_sector != consts::FREE_SECTOR
        {
            if current_difat_sector > consts::MAX_REGULAR_SECTOR {
                invalid_data!(
                    "DIFAT chain includes invalid sector index {}",
                    current_difat_sector
                );
            } else if current_difat_sector >= num_sectors {
                invalid_data!(
                    "DIFAT chain includes sector index {}, but sector count \
                     is only {}",
                    current_difat_sector,
                    num_sectors
                );
            }
            if seen_sector_ids.contains(&current_difat_sector) {
                invalid_data!(
                    "DIFAT chain includes duplicate sector index {}",
                    current_difat_sector,
                );
            }
            seen_sector_ids.insert(current_difat_sector);
            difat_sector_ids.push(current_difat_sector);
            let mut sector = sectors.seek_to_sector(current_difat_sector)?;
            for _ in 0..(sector_len / size_of::<u32>() - 1) {
                let next = sector.read_le_u32()?;
                if next != consts::FREE_SECTOR && next > consts::MAX_REGULAR_SECTOR {
                    invalid_data!("DIFAT refers to invalid sector index {}", next);
                }
                difat.push(next);
            }
            current_difat_sector = sector.read_le_u32()?;
        }
        // The DIFAT should be padded with FREE_SECTOR, but DIFAT sectors
        // may instead instead be incorrectly zero padded (see
        // https://github.com/mdsteele/rust-cfb/issues/41).
        // In case num_fat_sectors is not reliable, only remove zeroes,
        // and don't remove sectors from the header DIFAT.
        while difat.len() > consts::NUM_DIFAT_ENTRIES_IN_HEADER
            && difat.len() > header.num_fat_sectors as usize
            && difat.last() == Some(&0)
        {
            difat.pop();
        }
        while difat.last() == Some(&consts::FREE_SECTOR) {
            difat.pop();
        }

        // Read in FAT.
        let mut fat = Vec::<u32>::new();
        for &sector_index in difat.iter() {
            if sector_index >= num_sectors {
                invalid_data!(
                    "DIFAT refers to sector {}, but sector count is only {}",
                    sector_index,
                    num_sectors
                );
            }
            let mut sector = sectors.seek_to_sector(sector_index)?;
            for _ in 0..(sector_len / size_of::<u32>()) {
                fat.push(sector.read_le_u32()?);
            }
        }
        // If the number of sectors in the file is not a multiple of the number
        // of FAT entries per sector, then the last FAT sector must be padded
        // with FREE_SECTOR entries (see MS-CFB section 2.3).  However, some
        // CFB implementations incorrectly pad the last FAT sector with zeros
        // (see https://github.com/mdsteele/rust-cfb/issues/8), so we allow
        // this under Permissive validation.  Since zero is normally a
        // meaningful FAT entry (referring to sector 0), we only want to strip
        // zeros from the end of the FAT if they are beyond the number of
        // sectors in the file.
        // Files have been seen with erroneous other types of sectors beyond
        // EOF, so strip those as well.
        while fat.len() > num_sectors as usize {
            if fat.last() == Some(&0)
                || fat.last() == Some(&consts::DIFAT_SECTOR)
                || fat.last() == Some(&consts::FAT_SECTOR)
                || fat.last() == Some(&consts::FREE_SECTOR)
            {
                fat.pop();
            } else {
                break;
            }
        }
        while fat.len() < num_sectors as usize {
            fat.push(consts::FREE_SECTOR);
        }

        let mut allocator = Allocator::new(sectors, difat_sector_ids, difat, fat)?;

        // Read in directory.
        let mut dir_entries = Vec::<DirEntry>::new();
        let mut seen_dir_sectors = FnvHashSet::default();
        let mut current_dir_sector = header.first_dir_sector;
        while current_dir_sector != consts::END_OF_CHAIN {
            if current_dir_sector > consts::MAX_REGULAR_SECTOR {
                invalid_data!(
                    "Directory chain includes invalid sector index {}",
                    current_dir_sector
                );
            } else if current_dir_sector >= num_sectors {
                invalid_data!(
                    "Directory chain includes sector index {}, but sector \
                     count is only {}",
                    current_dir_sector,
                    num_sectors
                );
            }
            if !seen_dir_sectors.insert(current_dir_sector) {
                invalid_data!(
                    "Directory chain includes duplicate sector index {}",
                    current_dir_sector,
                );
            }
            {
                let mut sector = allocator.seek_to_sector(current_dir_sector)?;
                for _ in 0..header.version.dir_entries_per_sector() {
                    dir_entries.push(DirEntry::read_from(&mut sector, header.version)?);
                }
            }
            current_dir_sector = allocator.next(current_dir_sector)?;
        }

        let mut directory = Directory::new(allocator, dir_entries)?;

        // Read in MiniFAT.
        let minifat = {
            let mut chain = directory.open_chain(header.first_minifat_sector)?;
            let num_minifat_entries = (chain.len() / 4) as usize;
            let mut minifat = Vec::<u32>::with_capacity(num_minifat_entries);
            for _ in 0..num_minifat_entries {
                minifat.push(chain.read_le_u32()?);
            }
            while minifat.last() == Some(&consts::FREE_SECTOR) {
                minifat.pop();
            }
            minifat
        };

        let minialloc = MiniAllocator::new(directory, minifat)?;

        Ok(CompoundFile { minialloc })
    }

    pub fn exists(&self, path: impl AsRef<Path>) -> bool {
        internal::path::name_chain_from_path(path.as_ref())
            .is_ok_and(|names| self.minialloc.stream_id_for_name_chain(&names).is_some())
    }

    pub fn open_stream(&mut self, path: impl AsRef<Path>) -> io::Result<impl Read + '_> {
        let path = path.as_ref();
        let names = internal::path::name_chain_from_path(path)?;
        let Some(stream_id) = self.minialloc.stream_id_for_name_chain(&names) else {
            not_found!("No such stream: {:?}", path);
        };
        if self.minialloc.dir_entry(stream_id).obj_type != ObjType::Stream {
            invalid_input!("Not a stream: {:?}", path);
        }
        Ok(internal::Stream::new(&mut self.minialloc, stream_id))
    }
}

trait ReadLeNumber: Read {
    fn read_le_u64(&mut self) -> Result<u64, std::io::Error> {
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    fn read_le_u32(&mut self) -> Result<u32, std::io::Error> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    fn read_le_u16(&mut self) -> Result<u16, std::io::Error> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
}
impl<T: Read> ReadLeNumber for T {}
