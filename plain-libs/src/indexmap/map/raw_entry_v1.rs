use super::{Core, OccupiedEntry};
use crate::indexmap::{Equivalent, HashValue, IndexMap};
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::marker::PhantomData;
use core::mem;
pub trait RawEntryApiV1<K, V, S>: Sealed {
    fn raw_entry_v1(&self) -> RawEntryBuilder<'_, K, V, S>;
    fn raw_entry_mut_v1(&mut self) -> RawEntryBuilderMut<'_, K, V, S>;
}
impl<K, V, S> RawEntryApiV1<K, V, S> for IndexMap<K, V, S> {
    fn raw_entry_v1(&self) -> RawEntryBuilder<'_, K, V, S> {
        RawEntryBuilder { map: self }
    }
    fn raw_entry_mut_v1(&mut self) -> RawEntryBuilderMut<'_, K, V, S> {
        RawEntryBuilderMut { map: self }
    }
}
pub struct RawEntryBuilder<'a, K, V, S> {
    map: &'a IndexMap<K, V, S>,
}
impl<K, V, S> fmt::Debug for RawEntryBuilder<'_, K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawEntryBuilder").finish_non_exhaustive()
    }
}
impl<'a, K, V, S> RawEntryBuilder<'a, K, V, S> {
    pub fn from_key<Q>(self, key: &Q) -> Option<(&'a K, &'a V)>
    where
        S: BuildHasher,
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.map.get_key_value(key)
    }
    pub fn from_key_hashed_nocheck<Q>(self, hash: u64, key: &Q) -> Option<(&'a K, &'a V)>
    where
        Q: ?Sized + Equivalent<K>,
    {
        let hash = HashValue(hash as usize);
        let i = self.map.core.get_index_of(hash, key)?;
        self.map.get_index(i)
    }
    pub fn from_hash<F>(self, hash: u64, is_match: F) -> Option<(&'a K, &'a V)>
    where
        F: FnMut(&K) -> bool,
    {
        let map = self.map;
        let i = self.index_from_hash(hash, is_match)?;
        map.get_index(i)
    }

    pub fn index_from_hash<F>(self, hash: u64, is_match: F) -> Option<usize>
    where
        F: FnMut(&K) -> bool,
    {
        let hash = HashValue(hash as usize);
        self.map.core.get_index_of_raw(hash, is_match)
    }
}
pub struct RawEntryBuilderMut<'a, K, V, S> {
    map: &'a mut IndexMap<K, V, S>,
}
impl<K, V, S> fmt::Debug for RawEntryBuilderMut<'_, K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawEntryBuilderMut").finish_non_exhaustive()
    }
}
impl<'a, K, V, S> RawEntryBuilderMut<'a, K, V, S> {
    pub fn from_key<Q>(self, key: &Q) -> RawEntryMut<'a, K, V, S>
    where
        S: BuildHasher,
        Q: ?Sized + Hash + Equivalent<K>,
    {
        let hash = self.map.hash(key);
        self.from_key_hashed_nocheck(hash.get(), key)
    }
    pub fn from_key_hashed_nocheck<Q>(self, hash: u64, key: &Q) -> RawEntryMut<'a, K, V, S>
    where
        Q: ?Sized + Equivalent<K>,
    {
        self.from_hash(hash, |k| Q::equivalent(key, k))
    }
    pub fn from_hash<F>(self, hash: u64, is_match: F) -> RawEntryMut<'a, K, V, S>
    where
        F: FnMut(&K) -> bool,
    {
        let hash = HashValue(hash as usize);
        match OccupiedEntry::from_hash(&mut self.map.core, hash, is_match) {
            Ok(inner) => RawEntryMut::Occupied(RawOccupiedEntryMut {
                inner,
                hash_builder: PhantomData,
            }),
            Err(map) => RawEntryMut::Vacant(RawVacantEntryMut {
                map,
                hash_builder: &self.map.hash_builder,
            }),
        }
    }
}
pub enum RawEntryMut<'a, K, V, S> {
    Occupied(RawOccupiedEntryMut<'a, K, V, S>),
    Vacant(RawVacantEntryMut<'a, K, V, S>),
}
impl<K: fmt::Debug, V: fmt::Debug, S> fmt::Debug for RawEntryMut<'_, K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut tuple = f.debug_tuple("RawEntryMut");
        match self {
            Self::Vacant(v) => tuple.field(v),
            Self::Occupied(o) => tuple.field(o),
        };
        tuple.finish()
    }
}
impl<'a, K, V, S> RawEntryMut<'a, K, V, S> {
    #[inline]
    pub fn index(&self) -> usize {
        match self {
            Self::Occupied(entry) => entry.index(),
            Self::Vacant(entry) => entry.index(),
        }
    }
    pub fn or_insert(self, default_key: K, default_value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            Self::Occupied(entry) => entry.into_key_value_mut(),
            Self::Vacant(entry) => entry.insert(default_key, default_value),
        }
    }
    pub fn or_insert_with<F>(self, call: F) -> (&'a mut K, &'a mut V)
    where
        F: FnOnce() -> (K, V),
        K: Hash,
        S: BuildHasher,
    {
        match self {
            Self::Occupied(entry) => entry.into_key_value_mut(),
            Self::Vacant(entry) => {
                let (key, value) = call();
                entry.insert(key, value)
            }
        }
    }
    pub fn and_modify<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut K, &mut V),
    {
        if let Self::Occupied(entry) = &mut self {
            let (k, v) = entry.get_key_value_mut();
            f(k, v);
        }
        self
    }
}
pub struct RawOccupiedEntryMut<'a, K, V, S> {
    inner: OccupiedEntry<'a, K, V>,
    hash_builder: PhantomData<&'a S>,
}
impl<K: fmt::Debug, V: fmt::Debug, S> fmt::Debug for RawOccupiedEntryMut<'_, K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawOccupiedEntryMut")
            .field("key", self.key())
            .field("value", self.get())
            .finish_non_exhaustive()
    }
}
impl<'a, K, V, S> RawOccupiedEntryMut<'a, K, V, S> {
    #[inline]
    pub fn index(&self) -> usize {
        self.inner.index()
    }
    pub fn key(&self) -> &K {
        self.inner.key()
    }
    pub fn key_mut(&mut self) -> &mut K {
        &mut self.inner.get_bucket_mut().key
    }
    pub fn into_key(self) -> &'a mut K {
        &mut self.inner.into_bucket().key
    }
    pub fn get(&self) -> &V {
        self.inner.get()
    }
    pub fn get_mut(&mut self) -> &mut V {
        self.inner.get_mut()
    }
    pub fn into_mut(self) -> &'a mut V {
        self.inner.into_mut()
    }
    pub fn get_key_value(&self) -> (&K, &V) {
        self.inner.get_bucket().refs()
    }
    pub fn get_key_value_mut(&mut self) -> (&mut K, &mut V) {
        self.inner.get_bucket_mut().muts()
    }
    pub fn into_key_value_mut(self) -> (&'a mut K, &'a mut V) {
        self.inner.into_bucket().muts()
    }
    pub fn insert(&mut self, value: V) -> V {
        self.inner.insert(value)
    }

    pub fn swap_remove(self) -> V {
        self.inner.swap_remove()
    }
    pub fn shift_remove(self) -> V {
        self.inner.shift_remove()
    }
    pub fn swap_remove_entry(self) -> (K, V) {
        self.inner.swap_remove_entry()
    }
    pub fn shift_remove_entry(self) -> (K, V) {
        self.inner.shift_remove_entry()
    }
    #[track_caller]
    pub fn move_index(self, to: usize) {
        self.inner.move_index(to);
    }
    #[track_caller]
    pub fn swap_indices(self, other: usize) {
        self.inner.swap_indices(other);
    }
}
pub struct RawVacantEntryMut<'a, K, V, S> {
    map: &'a mut Core<K, V>,
    hash_builder: &'a S,
}
impl<K, V, S> fmt::Debug for RawVacantEntryMut<'_, K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawVacantEntryMut").finish_non_exhaustive()
    }
}
impl<'a, K, V, S> RawVacantEntryMut<'a, K, V, S> {
    pub fn index(&self) -> usize {
        self.map.len()
    }
    pub fn insert(self, key: K, value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        let h = self.hash_builder.hash_one(&key);
        self.insert_hashed_nocheck(h, key, value)
    }
    pub fn insert_hashed_nocheck(self, hash: u64, key: K, value: V) -> (&'a mut K, &'a mut V) {
        let hash = HashValue(hash as usize);
        self.map.insert_unique(hash, key, value).muts()
    }
    #[track_caller]
    pub fn shift_insert(self, index: usize, key: K, value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        let h = self.hash_builder.hash_one(&key);
        self.shift_insert_hashed_nocheck(index, h, key, value)
    }
    #[track_caller]
    pub fn shift_insert_hashed_nocheck(
        self,
        index: usize,
        hash: u64,
        key: K,
        value: V,
    ) -> (&'a mut K, &'a mut V) {
        let hash = HashValue(hash as usize);
        self.map.shift_insert_unique(index, hash, key, value).muts()
    }
}
trait Sealed {}
impl<K, V, S> Sealed for IndexMap<K, V, S> {}
