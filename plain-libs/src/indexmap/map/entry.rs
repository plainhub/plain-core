use crate::indexmap::Bucket;
use crate::indexmap::inner::{Core, OccupiedEntry, VacantEntry};
use core::{fmt, mem};
pub enum Entry<'a, K, V> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}
impl<'a, K, V> Entry<'a, K, V> {
    pub fn index(&self) -> usize {
        match self {
            Entry::Occupied(entry) => entry.index(),
            Entry::Vacant(entry) => entry.index(),
        }
    }
    pub fn insert_entry(self, value: V) -> OccupiedEntry<'a, K, V> {
        match self {
            Entry::Occupied(mut entry) => {
                entry.insert(value);
                entry
            }
            Entry::Vacant(entry) => entry.insert_entry(value),
        }
    }
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }
    pub fn or_insert_with<F>(self, call: F) -> &'a mut V
    where
        F: FnOnce() -> V,
    {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(call()),
        }
    }

    pub fn key(&self) -> &K {
        match *self {
            Entry::Occupied(ref entry) => entry.key(),
            Entry::Vacant(ref entry) => entry.key(),
        }
    }
    pub fn and_modify<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        if let Entry::Occupied(entry) = &mut self {
            f(entry.get_mut());
        }
        self
    }
    pub fn or_default(self) -> &'a mut V
    where
        V: Default,
    {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(V::default()),
        }
    }
}
impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Entry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut tuple = f.debug_tuple("Entry");
        match self {
            Entry::Vacant(v) => tuple.field(v),
            Entry::Occupied(o) => tuple.field(o),
        };
        tuple.finish()
    }
}
impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for OccupiedEntry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("key", self.key())
            .field("value", self.get())
            .finish()
    }
}
impl<K: fmt::Debug, V> fmt::Debug for VacantEntry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntry").field(self.key()).finish()
    }
}
pub struct IndexedEntry<'a, K, V> {
    map: &'a mut Core<K, V>,
    index: usize,
}
impl<'a, K, V> IndexedEntry<'a, K, V> {
    pub(crate) fn new(map: &'a mut Core<K, V>, index: usize) -> Option<Self> {
        if index < map.len() {
            Some(Self { map, index })
        } else {
            None
        }
    }
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }
    pub(crate) fn into_core(self) -> &'a mut Core<K, V> {
        self.map
    }
    fn get_bucket(&self) -> &Bucket<K, V> {
        &self.map.as_entries()[self.index]
    }
    fn get_bucket_mut(&mut self) -> &mut Bucket<K, V> {
        &mut self.map.as_entries_mut()[self.index]
    }
    fn into_bucket(self) -> &'a mut Bucket<K, V> {
        &mut self.map.as_entries_mut()[self.index]
    }
    pub fn key(&self) -> &K {
        &self.get_bucket().key
    }
    pub(super) fn key_mut(&mut self) -> &mut K {
        &mut self.get_bucket_mut().key
    }
    pub fn get(&self) -> &V {
        &self.get_bucket().value
    }
    pub fn get_mut(&mut self) -> &mut V {
        &mut self.get_bucket_mut().value
    }
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }
    pub fn into_mut(self) -> &'a mut V {
        &mut self.into_bucket().value
    }
    pub fn swap_remove_entry(self) -> (K, V) {
        self.map.swap_remove_index(self.index).unwrap()
    }
    pub fn shift_remove_entry(self) -> (K, V) {
        self.map.shift_remove_index(self.index).unwrap()
    }
    pub fn swap_remove(self) -> V {
        self.swap_remove_entry().1
    }
    pub fn shift_remove(self) -> V {
        self.shift_remove_entry().1
    }
    #[track_caller]
    pub fn move_index(self, to: usize) {
        self.map.move_index(self.index, to);
    }
    #[track_caller]
    pub fn swap_indices(self, other: usize) {
        self.map.swap_indices(self.index, other);
    }
}
impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for IndexedEntry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IndexedEntry")
            .field("index", &self.index)
            .field("key", self.key())
            .field("value", self.get())
            .finish()
    }
}
impl<'a, K, V> From<OccupiedEntry<'a, K, V>> for IndexedEntry<'a, K, V> {
    fn from(other: OccupiedEntry<'a, K, V>) -> Self {
        Self {
            index: other.index(),
            map: other.into_core(),
        }
    }
}
