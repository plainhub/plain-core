use super::{Bucket, Core, equal, get_hash};
use crate::indexmap::HashValue;
use crate::indexmap::map::{Entry, IndexedEntry};
use crate::indexmap::util::assert_index_lt;
use core::cmp::Ordering;
use core::mem;
impl<'a, K, V> Entry<'a, K, V> {
    pub(crate) fn new(map: &'a mut Core<K, V>, hash: HashValue, key: K) -> Self
    where
        K: Eq,
    {
        let eq = equal(&key, &map.entries);
        match map.indices.find_entry(hash.get(), eq) {
            Ok(entry) => Entry::Occupied(OccupiedEntry {
                bucket: entry.bucket_index(),
                index: *entry.get(),
                map,
            }),
            Err(_) => Entry::Vacant(VacantEntry { map, hash, key }),
        }
    }
}
pub struct OccupiedEntry<'a, K, V> {
    map: &'a mut Core<K, V>,
    index: usize,
    bucket: usize,
}
impl<'a, K, V> OccupiedEntry<'a, K, V> {
    pub(crate) fn from_hash<F>(
        map: &'a mut Core<K, V>,
        hash: HashValue,
        mut is_match: F,
    ) -> Result<Self, &'a mut Core<K, V>>
    where
        F: FnMut(&K) -> bool,
    {
        let entries = &*map.entries;
        let eq = move |&i: &usize| is_match(&entries[i].key);
        match map.indices.find_entry(hash.get(), eq) {
            Ok(entry) => Ok(OccupiedEntry {
                bucket: entry.bucket_index(),
                index: *entry.get(),
                map,
            }),
            Err(_) => Err(map),
        }
    }
    pub(crate) fn into_core(self) -> &'a mut Core<K, V> {
        self.map
    }
    pub(crate) fn get_bucket(&self) -> &Bucket<K, V> {
        &self.map.entries[self.index]
    }
    pub(crate) fn get_bucket_mut(&mut self) -> &mut Bucket<K, V> {
        &mut self.map.entries[self.index]
    }
    pub(crate) fn into_bucket(self) -> &'a mut Bucket<K, V> {
        &mut self.map.entries[self.index]
    }
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn key(&self) -> &K {
        &self.get_bucket().key
    }
    pub fn get(&self) -> &V {
        &self.get_bucket().value
    }
    pub fn get_mut(&mut self) -> &mut V {
        &mut self.get_bucket_mut().value
    }
    pub fn into_mut(self) -> &'a mut V {
        &mut self.into_bucket().value
    }
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }
    pub fn swap_remove(self) -> V {
        self.swap_remove_entry().1
    }
    pub fn shift_remove(self) -> V {
        self.shift_remove_entry().1
    }
    pub fn swap_remove_entry(mut self) -> (K, V) {
        self.remove_index();
        self.map.swap_remove_finish(self.index)
    }
    pub fn shift_remove_entry(mut self) -> (K, V) {
        self.remove_index();
        self.map.shift_remove_finish(self.index)
    }
    fn remove_index(&mut self) {
        let entry = self.map.indices.get_bucket_entry(self.bucket).unwrap();
        debug_assert_eq!(*entry.get(), self.index);
        entry.remove();
    }
    #[track_caller]
    pub fn move_index(self, to: usize) {
        if self.index != to {
            assert_index_lt(to, self.map.len());
            self.map.move_index_inner(self.index, to);
            self.update_index(to);
        }
    }
    #[track_caller]
    pub fn swap_indices(self, other: usize) {
        if self.index != other {
            assert_index_lt(other, self.map.len());
            let hash = self.map.entries[other].hash;
            let other_mut = self.map.indices.find_mut(hash.get(), move |&i| i == other);
            *other_mut.expect("index not found") = self.index;
            self.map.entries.swap(self.index, other);
            self.update_index(other);
        }
    }
    fn update_index(self, to: usize) {
        let index = self.map.indices.get_bucket_mut(self.bucket).unwrap();
        debug_assert_eq!(*index, self.index);
        *index = to;
    }
}
impl<'a, K, V> From<IndexedEntry<'a, K, V>> for OccupiedEntry<'a, K, V> {
    fn from(other: IndexedEntry<'a, K, V>) -> Self {
        let index = other.index();
        let map = other.into_core();
        let hash = map.entries[index].hash;
        let bucket = map
            .indices
            .find_bucket_index(hash.get(), move |&i| i == index)
            .expect("index not found");
        Self { map, index, bucket }
    }
}
pub struct VacantEntry<'a, K, V> {
    map: &'a mut Core<K, V>,
    hash: HashValue,
    key: K,
}
impl<'a, K, V> VacantEntry<'a, K, V> {
    pub fn index(&self) -> usize {
        self.map.indices.len()
    }
    pub fn key(&self) -> &K {
        &self.key
    }
    pub(crate) fn key_mut(&mut self) -> &mut K {
        &mut self.key
    }
    pub fn into_key(self) -> K {
        self.key
    }
    pub fn insert(self, value: V) -> &'a mut V {
        let Self { map, hash, key } = self;
        map.insert_unique(hash, key, value).value_mut()
    }
    pub fn insert_entry(self, value: V) -> OccupiedEntry<'a, K, V> {
        let Self { map, hash, key } = self;
        let index = map.indices.len();
        debug_assert_eq!(index, map.entries.len());
        let bucket = map
            .indices
            .insert_unique(hash.get(), index, get_hash(&map.entries))
            .bucket_index();
        map.push_entry(hash, key, value);
        OccupiedEntry { map, index, bucket }
    }
    pub fn insert_sorted(self, value: V) -> (usize, &'a mut V)
    where
        K: Ord,
    {
        let slice = crate::indexmap::map::Slice::from_slice(&self.map.entries);
        let i = slice.binary_search_keys(&self.key).unwrap_err();
        (i, self.shift_insert(i, value))
    }
    pub fn insert_sorted_by<F>(self, value: V, mut cmp: F) -> (usize, &'a mut V)
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        let slice = crate::indexmap::map::Slice::from_slice(&self.map.entries);
        let (Ok(i) | Err(i)) = slice.binary_search_by(|k, v| cmp(k, v, &self.key, &value));
        (i, self.shift_insert(i, value))
    }
    pub fn insert_sorted_by_key<B, F>(self, value: V, mut sort_key: F) -> (usize, &'a mut V)
    where
        B: Ord,
        F: FnMut(&K, &V) -> B,
    {
        let search_key = sort_key(&self.key, &value);
        let slice = crate::indexmap::map::Slice::from_slice(&self.map.entries);
        let (Ok(i) | Err(i)) = slice.binary_search_by_key(&search_key, sort_key);
        (i, self.shift_insert(i, value))
    }
    #[track_caller]
    pub fn shift_insert(self, index: usize, value: V) -> &'a mut V {
        self.map
            .shift_insert_unique(index, self.hash, self.key, value)
            .value_mut()
    }
    #[track_caller]
    pub fn replace_index(self, index: usize) -> (K, OccupiedEntry<'a, K, V>) {
        let Self { map, hash, key } = self;
        assert_index_lt(index, map.len());
        let old_hash = map.entries[index].hash;
        map.indices
            .find_entry(old_hash.get(), move |&i| i == index)
            .expect("index not found")
            .remove();
        let bucket = map
            .indices
            .insert_unique(hash.get(), index, get_hash(&map.entries))
            .bucket_index();
        let entry = &mut map.entries[index];
        entry.hash = hash;
        let old_key = mem::replace(&mut entry.key, key);
        (old_key, OccupiedEntry { map, index, bucket })
    }
}
