use crate::compound_file::internal::{self, Allocator, Chain, DirEntry, ObjType, Sector, consts};
use crate::fnv::{FnvHashMap, FnvHashSet};
use std::cmp::Ordering;
use std::io::{self, Seek};

//===========================================================================//

macro_rules! malformed {
    ($e:expr) => { invalid_data!("Malformed directory ({})", $e) };
    ($fmt:expr, $($arg:tt)+) => {
        invalid_data!("Malformed directory ({})", format!($fmt, $($arg)+))
    };
}

//===========================================================================//

/// What points at the root of a subtree of a storage's sibling tree: either
/// the storage's own `child` field, or a sibling pointer of another node.

//===========================================================================//

/// A wrapper around the sector allocator that additionally provides management
/// of the CFB directory chain.
pub struct Directory<F> {
    allocator: Allocator<F>,
    dir_entries: Vec<DirEntry>,
    /// The sector IDs of the directory chain, in order. Locating the slot of
    /// a directory entry in the file only needs an index into this; walking
    /// the FAT chain from the start for every entry write made creating `n`
    /// entries cost `O(n²)`. The chain only ever grows (see
    /// `allocate_dir_entry`), so it is walked once and extended in place.
    /// Stream IDs of unallocated directory entries, available for reuse;
    /// the lowest one is on top.
    free_entries: Vec<u32>,
    /// Every entry's stream ID by `(parent stream ID, name key)`. Files
    /// written by other implementations often have unbalanced sibling trees,
    /// which would make a lookup by walking the tree cost `O(siblings)`; the
    /// index makes it `O(1)` whatever shape the tree is in.
    name_index: FnvHashMap<(u32, (usize, String)), u32>,
}

impl<F> Directory<F> {
    pub fn new(allocator: Allocator<F>, dir_entries: Vec<DirEntry>) -> io::Result<Directory<F>> {
        let mut directory = Directory {
            allocator,
            dir_entries,
            free_entries: Vec::new(),
            name_index: FnvHashMap::default(),
        };
        directory.validate()?;
        directory.free_entries = directory
            .dir_entries
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, entry)| entry.obj_type == ObjType::Unallocated)
            .map(|(stream_id, _)| stream_id as u32)
            .collect();
        directory.build_name_index();
        Ok(directory)
    }

    /// Fills `name_index` from the sibling trees, visiting every entry once.
    fn build_name_index(&mut self) {
        let mut index = FnvHashMap::default();
        let mut stack = vec![(consts::ROOT_STREAM_ID, consts::ROOT_STREAM_ID)];
        while let Some((stream_id, parent_id)) = stack.pop() {
            let dir_entry = self.dir_entry(stream_id);
            if stream_id != consts::ROOT_STREAM_ID {
                index.insert(
                    (parent_id, internal::path::name_key(&dir_entry.name)),
                    stream_id,
                );
            }
            for sibling in [dir_entry.left_sibling, dir_entry.right_sibling] {
                if sibling != consts::NO_STREAM {
                    stack.push((sibling, parent_id));
                }
            }
            if dir_entry.child != consts::NO_STREAM {
                stack.push((dir_entry.child, stream_id));
            }
        }
        self.name_index = index;
    }

    pub fn sector_len(&self) -> usize {
        self.allocator.sector_len()
    }

    pub fn stream_id_for_name_chain(&self, names: &[&str]) -> Option<u32> {
        let mut stream_id = consts::ROOT_STREAM_ID;
        for name in names.iter() {
            let key = (stream_id, internal::path::name_key(name));
            stream_id = *self.name_index.get(&key)?;
        }
        Some(stream_id)
    }

    pub fn open_chain(&mut self, start_sector_id: u32) -> io::Result<Chain<'_, F>> {
        self.allocator.open_chain(start_sector_id)
    }

    pub fn root_dir_entry(&self) -> &DirEntry {
        self.dir_entry(consts::ROOT_STREAM_ID)
    }

    pub fn dir_entry(&self, stream_id: u32) -> &DirEntry {
        &self.dir_entries[stream_id as usize]
    }

    fn validate(&self) -> io::Result<()> {
        if self.dir_entries.is_empty() {
            malformed!("root entry is missing");
        }
        let root_entry = self.root_dir_entry();
        if root_entry.stream_len % consts::MINI_SECTOR_LEN as u64 != 0 {
            malformed!(
                "root stream len is {}, but should be multiple of {}",
                root_entry.stream_len,
                consts::MINI_SECTOR_LEN
            );
        }
        let mut visited = FnvHashSet::default();
        let mut stack = vec![consts::ROOT_STREAM_ID];
        while let Some(stream_id) = stack.pop() {
            if visited.contains(&stream_id) {
                malformed!("loop in tree");
            }
            visited.insert(stream_id);
            let dir_entry = self.dir_entry(stream_id);
            if stream_id == consts::ROOT_STREAM_ID {
                if dir_entry.obj_type != ObjType::Root {
                    malformed!("root entry has object type {:?}", dir_entry.obj_type);
                }
            } else if dir_entry.obj_type != ObjType::Storage
                && dir_entry.obj_type != ObjType::Stream
            {
                malformed!("non-root entry with object type {:?}", dir_entry.obj_type);
            }
            let left_sibling = dir_entry.left_sibling;
            if left_sibling != consts::NO_STREAM {
                if left_sibling as usize >= self.dir_entries.len() {
                    malformed!(
                        "left sibling index is {}, but directory entry count \
                         is {}",
                        left_sibling,
                        self.dir_entries.len()
                    );
                }
                let entry = &self.dir_entry(left_sibling);
                if internal::path::compare_names(&entry.name, &dir_entry.name) != Ordering::Less {
                    malformed!("name ordering, {:?} vs {:?}", dir_entry.name, entry.name);
                }
                stack.push(left_sibling);
            }
            let right_sibling = dir_entry.right_sibling;
            if right_sibling != consts::NO_STREAM {
                if right_sibling as usize >= self.dir_entries.len() {
                    malformed!(
                        "right sibling index is {}, but directory entry count \
                         is {}",
                        right_sibling,
                        self.dir_entries.len()
                    );
                }
                let entry = &self.dir_entry(right_sibling);
                if internal::path::compare_names(&dir_entry.name, &entry.name) != Ordering::Less {
                    malformed!("name ordering, {:?} vs {:?}", dir_entry.name, entry.name);
                }
                stack.push(right_sibling);
            }
            let child = dir_entry.child;
            if child != consts::NO_STREAM {
                if child as usize >= self.dir_entries.len() {
                    malformed!(
                        "child index is {}, but directory entry count is {}",
                        child,
                        self.dir_entries.len()
                    );
                }
                stack.push(child);
            }
        }
        Ok(())
    }
}

impl<F: Seek> Directory<F> {
    pub fn seek_within_sector(
        &mut self,
        sector_id: u32,
        offset_within_sector: u64,
    ) -> io::Result<Sector<'_, F>> {
        self.allocator
            .seek_within_sector(sector_id, offset_within_sector)
    }
}
