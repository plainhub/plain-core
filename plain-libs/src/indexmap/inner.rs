mod entry;
mod extract;
use crate::hashbrown::hash_table;
use crate::indexmap::util::{assert_index_le, assert_index_lt, simplify_range};
use crate::indexmap::{Bucket, Equivalent, HashValue, TryReserveError};
use alloc::vec::{self, Vec};
use core::mem;
use core::ops::RangeBounds;
type Indices = hash_table::HashTable<usize>;
type Entries<K, V> = Vec<Bucket<K, V>>;
pub use entry::{OccupiedEntry, VacantEntry};
pub(crate) use extract::ExtractCore;
#[cfg_attr(any(), derive(Debug))]
pub(crate) struct Core<K, V> {
    indices: Indices,
    entries: Entries<K, V>,
}
#[inline(always)]
fn get_hash<K, V>(entries: &[Bucket<K, V>]) -> impl Fn(&usize) -> u64 + use<'_, K, V> {
    move |&i| entries[i].hash.get()
}
#[inline]
fn equal<'a, K: Eq, V>(
    key: &'a K,
    entries: &'a [Bucket<K, V>],
) -> impl Fn(&usize) -> bool + use<'a, K, V> {
    move |&i| K::eq(key, &entries[i].key)
}
#[inline]
fn equivalent<'a, K, V, Q: ?Sized + Equivalent<K>>(
    key: &'a Q,
    entries: &'a [Bucket<K, V>],
) -> impl Fn(&usize) -> bool + use<'a, K, V, Q> {
    move |&i| Q::equivalent(key, &entries[i].key)
}
#[inline]
fn erase_index(table: &mut Indices, hash: HashValue, index: usize) {
    if let Ok(entry) = table.find_entry(hash.get(), move |&i| i == index) {
        entry.remove();
    } else if cfg!(debug_assertions) {
        panic!("index not found");
    }
}
#[inline]
fn update_index(table: &mut Indices, hash: HashValue, old: usize, new: usize) {
    let index = table
        .find_mut(hash.get(), move |&i| i == old)
        .expect("index not found");
    *index = new;
}
fn insert_bulk_no_grow<K, V>(indices: &mut Indices, entries: &[Bucket<K, V>]) {
    assert!(indices.capacity() - indices.len() >= entries.len());
    for entry in entries {
        indices.insert_unique(entry.hash.get(), indices.len(), |_| unreachable!());
    }
}
impl<K, V> Clone for Core<K, V>
where
    K: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        let mut new = Self::new();
        new.clone_from(self);
        new
    }
    fn clone_from(&mut self, other: &Self) {
        self.indices.clone_from(&other.indices);
        if self.entries.capacity() < other.entries.len() {
            let additional = other.entries.len() - self.entries.len();
            self.reserve_entries(additional);
        }
        self.entries.clone_from(&other.entries);
    }
}
impl<K, V> Core<K, V> {
    const MAX_ENTRIES_CAPACITY: usize = (isize::MAX as usize) / size_of::<Bucket<K, V>>();
    #[inline]
    pub(crate) const fn new() -> Self {
        Core {
            indices: Indices::new(),
            entries: Vec::new(),
        }
    }
    #[inline]
    pub(crate) fn with_capacity(n: usize) -> Self {
        Core {
            indices: Indices::with_capacity(n),
            entries: Vec::with_capacity(n),
        }
    }
    #[inline]
    pub(crate) fn into_entries(self) -> Entries<K, V> {
        self.entries
    }
    #[inline]
    pub(crate) fn as_entries(&self) -> &[Bucket<K, V>] {
        &self.entries
    }
    #[inline]
    pub(crate) fn as_entries_mut(&mut self) -> &mut [Bucket<K, V>] {
        &mut self.entries
    }
    pub(crate) fn with_entries<F>(&mut self, f: F)
    where
        F: FnOnce(&mut [Bucket<K, V>]),
    {
        f(&mut self.entries);
        self.rebuild_hash_table();
    }
    #[inline]
    pub(crate) fn len(&self) -> usize {
        debug_assert_eq!(self.entries.len(), self.indices.len());
        self.indices.len()
    }
    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        Ord::min(self.indices.capacity(), self.entries.capacity())
    }
    pub(crate) fn clear(&mut self) {
        self.indices.clear();
        self.entries.clear();
    }
    pub(crate) fn truncate(&mut self, len: usize) {
        if len < self.len() {
            self.erase_indices(len, self.entries.len());
            self.entries.truncate(len);
        }
    }
    #[track_caller]
    pub(crate) fn drain<R>(&mut self, range: R) -> vec::Drain<'_, Bucket<K, V>>
    where
        R: RangeBounds<usize>,
    {
        let range = simplify_range(range, self.entries.len());
        self.erase_indices(range.start, range.end);
        self.entries.drain(range)
    }
    #[track_caller]
    pub(crate) fn split_off(&mut self, at: usize) -> Self {
        assert_index_le(at, self.len());
        self.erase_indices(at, self.entries.len());
        let entries = self.entries.split_off(at);
        let mut indices = Indices::with_capacity(entries.len());
        insert_bulk_no_grow(&mut indices, &entries);
        Self { indices, entries }
    }
    #[track_caller]
    pub(crate) fn split_splice<R>(&mut self, range: R) -> (Self, vec::IntoIter<Bucket<K, V>>)
    where
        R: RangeBounds<usize>,
    {
        let range = simplify_range(range, self.len());
        self.erase_indices(range.start, self.entries.len());
        let entries = self.entries.split_off(range.end);
        let drained = self.entries.split_off(range.start);
        let mut indices = Indices::with_capacity(entries.len());
        insert_bulk_no_grow(&mut indices, &entries);
        (Self { indices, entries }, drained.into_iter())
    }
    pub(crate) fn append_unchecked(&mut self, other: &mut Self) {
        self.reserve(other.len());
        insert_bulk_no_grow(&mut self.indices, &other.entries);
        self.entries.append(&mut other.entries);
        other.indices.clear();
    }
    pub(crate) fn reserve(&mut self, additional: usize) {
        self.indices.reserve(additional, get_hash(&self.entries));
        if additional > self.entries.capacity() - self.entries.len() {
            self.reserve_entries(additional);
        }
    }
    pub(crate) fn reserve_exact(&mut self, additional: usize) {
        self.indices.reserve(additional, get_hash(&self.entries));
        self.entries.reserve_exact(additional);
    }
    pub(crate) fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.indices
            .try_reserve(additional, get_hash(&self.entries))
            .map_err(TryReserveError::from_hashbrown)?;
        if additional > self.entries.capacity() - self.entries.len() {
            self.try_reserve_entries(additional)
        } else {
            Ok(())
        }
    }
    fn try_reserve_entries(&mut self, additional: usize) -> Result<(), TryReserveError> {
        let new_capacity = Ord::min(self.indices.capacity(), Self::MAX_ENTRIES_CAPACITY);
        let try_add = new_capacity - self.entries.len();
        if try_add > additional && self.entries.try_reserve_exact(try_add).is_ok() {
            return Ok(());
        }
        self.entries
            .try_reserve_exact(additional)
            .map_err(TryReserveError::from_alloc)
    }
    pub(crate) fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.indices
            .try_reserve(additional, get_hash(&self.entries))
            .map_err(TryReserveError::from_hashbrown)?;
        self.entries
            .try_reserve_exact(additional)
            .map_err(TryReserveError::from_alloc)
    }
    pub(crate) fn shrink_to(&mut self, min_capacity: usize) {
        self.indices
            .shrink_to(min_capacity, get_hash(&self.entries));
        self.entries.shrink_to(min_capacity);
    }
    pub(crate) fn pop(&mut self) -> Option<(K, V)> {
        if let Some(entry) = self.entries.pop() {
            let last = self.entries.len();
            erase_index(&mut self.indices, entry.hash, last);
            Some((entry.key, entry.value))
        } else {
            None
        }
    }
    pub(crate) fn get_index_of<Q>(&self, hash: HashValue, key: &Q) -> Option<usize>
    where
        Q: ?Sized + Equivalent<K>,
    {
        let eq = equivalent(key, &self.entries);
        self.indices.find(hash.get(), eq).copied()
    }
    pub(crate) fn get_index_of_raw<F>(&self, hash: HashValue, mut is_match: F) -> Option<usize>
    where
        F: FnMut(&K) -> bool,
    {
        let eq = move |&i: &usize| is_match(&self.entries[i].key);
        self.indices.find(hash.get(), eq).copied()
    }
    fn push_entry(&mut self, hash: HashValue, key: K, value: V) {
        if self.entries.len() == self.entries.capacity() {
            self.reserve_entries(1);
        }
        self.entries.push(Bucket { hash, key, value });
    }
    pub(crate) fn insert_full(&mut self, hash: HashValue, key: K, value: V) -> (usize, Option<V>)
    where
        K: Eq,
    {
        let eq = equal(&key, &self.entries);
        let hasher = get_hash(&self.entries);
        match self.indices.entry(hash.get(), eq, hasher) {
            hash_table::Entry::Occupied(entry) => {
                let i = *entry.get();
                (i, Some(mem::replace(&mut self.entries[i].value, value)))
            }
            hash_table::Entry::Vacant(entry) => {
                let i = self.entries.len();
                entry.insert(i);
                self.push_entry(hash, key, value);
                debug_assert_eq!(self.indices.len(), self.entries.len());
                (i, None)
            }
        }
    }
    pub(crate) fn replace_full(
        &mut self,
        hash: HashValue,
        key: K,
        value: V,
    ) -> (usize, Option<(K, V)>)
    where
        K: Eq,
    {
        let eq = equal(&key, &self.entries);
        let hasher = get_hash(&self.entries);
        match self.indices.entry(hash.get(), eq, hasher) {
            hash_table::Entry::Occupied(entry) => {
                let i = *entry.get();
                let entry = &mut self.entries[i];
                let kv = (
                    mem::replace(&mut entry.key, key),
                    mem::replace(&mut entry.value, value),
                );
                (i, Some(kv))
            }
            hash_table::Entry::Vacant(entry) => {
                let i = self.entries.len();
                entry.insert(i);
                self.push_entry(hash, key, value);
                debug_assert_eq!(self.indices.len(), self.entries.len());
                (i, None)
            }
        }
    }
    pub(crate) fn shift_remove_full<Q>(&mut self, hash: HashValue, key: &Q) -> Option<(usize, K, V)>
    where
        Q: ?Sized + Equivalent<K>,
    {
        let eq = equivalent(key, &self.entries);
        let (index, _) = self.indices.find_entry(hash.get(), eq).ok()?.remove();
        let (key, value) = self.shift_remove_finish(index);
        Some((index, key, value))
    }
    pub(crate) fn swap_remove_full<Q>(&mut self, hash: HashValue, key: &Q) -> Option<(usize, K, V)>
    where
        Q: ?Sized + Equivalent<K>,
    {
        let eq = equivalent(key, &self.entries);
        let (index, _) = self.indices.find_entry(hash.get(), eq).ok()?.remove();
        let (key, value) = self.swap_remove_finish(index);
        Some((index, key, value))
    }
    fn erase_indices(&mut self, start: usize, end: usize) {
        let (init, shifted_entries) = self.entries.split_at(end);
        let (start_entries, erased_entries) = init.split_at(start);
        let erased = erased_entries.len();
        let shifted = shifted_entries.len();
        let half_capacity = self.indices.capacity() / 2;
        if erased == 0 {
        } else if start + shifted < half_capacity && start < erased {
            self.indices.clear();
            insert_bulk_no_grow(&mut self.indices, start_entries);
            insert_bulk_no_grow(&mut self.indices, shifted_entries);
        } else if erased + shifted < half_capacity {
            for (i, entry) in (start..).zip(erased_entries) {
                erase_index(&mut self.indices, entry.hash, i);
            }
            for ((new, old), entry) in (start..).zip(end..).zip(shifted_entries) {
                update_index(&mut self.indices, entry.hash, old, new);
            }
        } else {
            let offset = end - start;
            self.indices.retain(move |i| {
                if *i >= end {
                    *i -= offset;
                    true
                } else {
                    *i < start
                }
            });
        }
        debug_assert_eq!(self.indices.len(), start + shifted);
    }
    pub(crate) fn retain_in_order<F>(&mut self, mut keep: F)
    where
        F: FnMut(&mut K, &mut V) -> bool,
    {
        self.entries
            .retain_mut(|entry| keep(&mut entry.key, &mut entry.value));
        if self.entries.len() < self.indices.len() {
            self.rebuild_hash_table();
        }
    }
    fn rebuild_hash_table(&mut self) {
        self.indices.clear();
        insert_bulk_no_grow(&mut self.indices, &self.entries);
    }
    pub(crate) fn reverse(&mut self) {
        self.entries.reverse();
        let len = self.entries.len();
        for i in &mut self.indices {
            *i = len - *i - 1;
        }
    }
    #[inline]
    fn reserve_entries(&mut self, additional: usize) {
        let try_capacity = Ord::min(self.indices.capacity(), Self::MAX_ENTRIES_CAPACITY);
        let try_add = try_capacity - self.entries.len();
        if try_add > additional && self.entries.try_reserve_exact(try_add).is_ok() {
            return;
        }
        self.entries.reserve_exact(additional);
    }
    pub(super) fn insert_unique(&mut self, hash: HashValue, key: K, value: V) -> &mut Bucket<K, V> {
        let i = self.indices.len();
        debug_assert_eq!(i, self.entries.len());
        self.indices
            .insert_unique(hash.get(), i, get_hash(&self.entries));
        self.push_entry(hash, key, value);
        &mut self.entries[i]
    }
    #[track_caller]
    pub(crate) fn replace_index_unique(&mut self, index: usize, hash: HashValue, key: K) -> K {
        erase_index(&mut self.indices, self.entries[index].hash, index);
        self.indices
            .insert_unique(hash.get(), index, get_hash(&self.entries));
        let entry = &mut self.entries[index];
        entry.hash = hash;
        mem::replace(&mut entry.key, key)
    }
    pub(crate) fn shift_insert_unique(
        &mut self,
        index: usize,
        hash: HashValue,
        key: K,
        value: V,
    ) -> &mut Bucket<K, V> {
        let end = self.indices.len();
        assert!(index <= end);
        self.shift_indices::<true, false>(index, end);
        let entries = &*self.entries;
        self.indices.insert_unique(hash.get(), index, move |&i| {
            debug_assert_ne!(i, index);
            let i = if i < index { i } else { i - 1 };
            entries[i].hash.get()
        });
        if self.entries.len() == self.entries.capacity() {
            self.reserve_entries(1);
        }
        self.entries.insert(index, Bucket { hash, key, value });
        &mut self.entries[index]
    }
    pub(crate) fn shift_remove_index(&mut self, index: usize) -> Option<(K, V)> {
        match self.entries.get(index) {
            Some(entry) => {
                erase_index(&mut self.indices, entry.hash, index);
                Some(self.shift_remove_finish(index))
            }
            None => None,
        }
    }
    fn shift_remove_finish(&mut self, index: usize) -> (K, V) {
        self.shift_indices::<false, true>(index + 1, self.entries.len());
        let entry = self.entries.remove(index);
        (entry.key, entry.value)
    }
    pub(crate) fn swap_remove_index(&mut self, index: usize) -> Option<(K, V)> {
        match self.entries.get(index) {
            Some(entry) => {
                erase_index(&mut self.indices, entry.hash, index);
                Some(self.swap_remove_finish(index))
            }
            None => None,
        }
    }
    fn swap_remove_finish(&mut self, index: usize) -> (K, V) {
        let entry = self.entries.swap_remove(index);
        if let Some(entry) = self.entries.get(index) {
            let last = self.entries.len();
            update_index(&mut self.indices, entry.hash, last, index);
        }
        (entry.key, entry.value)
    }

    #[inline]
    // Keep the upstream threshold; balanced suffixes favor point lookups.
    fn use_bulk_shift(&self, shifted: usize) -> bool {
        #[cfg(test)]
        crate::indexmap::tests::record_bulk_choice();
        shifted > self.indices.capacity() / 2
    }
    #[inline(always)]
    fn shift_matching_indices<const INCREMENT: bool>(&mut self, matches: impl Fn(usize) -> bool) {
        self.indices.iter_mut().for_each(|index| {
            if matches(*index) {
                // Caller ranges and the removed first slot make the shift exact.
                *index = if INCREMENT {
                    index.wrapping_add(1)
                } else {
                    index.wrapping_sub(1)
                };
            }
        });
    }
    #[inline]
    fn shift_indices<const INCREMENT: bool, const REMOVED: bool>(
        &mut self,
        start: usize,
        end: usize,
    ) {
        let shifted_entries = &self.entries[start..end];
        if self.use_bulk_shift(shifted_entries.len()) {
            if end == self.entries.len() {
                if REMOVED && start == 1 {
                    #[cfg(test)]
                    crate::indexmap::tests::record_full_shift();
                    self.shift_matching_indices::<INCREMENT>(|_| true);
                } else {
                    #[cfg(test)]
                    crate::indexmap::tests::record_suffix_shift();
                    self.shift_matching_indices::<INCREMENT>(|index| start <= index);
                }
            } else {
                self.shift_matching_indices::<INCREMENT>(|index| start <= index && index < end);
            }
        } else if INCREMENT {
            let mut index = end;
            for entry in shifted_entries.iter().rev() {
                index = index.wrapping_sub(1);
                update_index(&mut self.indices, entry.hash, index, index.wrapping_add(1));
            }
        } else {
            // The validated slice and caller ranges bound both the counter and shifted index.
            let mut index = start;
            for entry in shifted_entries {
                update_index(&mut self.indices, entry.hash, index, index.wrapping_sub(1));
                index = index.wrapping_add(1);
            }
        }
    }
    #[track_caller]
    pub(super) fn move_index(&mut self, from: usize, to: usize) {
        assert_index_lt(from, self.len());
        let from_hash = self.entries[from].hash;
        if from != to {
            assert_index_lt(to, self.len());
            let bucket = self
                .indices
                .find_bucket_index(from_hash.get(), move |&i| i == from)
                .expect("index not found");
            self.move_index_inner(from, to);
            *self.indices.get_bucket_mut(bucket).unwrap() = to;
        }
    }
    fn move_index_inner(&mut self, from: usize, to: usize) {
        if from < to {
            self.shift_indices::<false, false>(from + 1, to + 1);
            self.entries[from..=to].rotate_left(1);
        } else if to < from {
            self.shift_indices::<true, false>(to, from);
            self.entries[to..=from].rotate_right(1);
        }
    }
    #[track_caller]
    pub(crate) fn swap_indices(&mut self, a: usize, b: usize) {
        assert_index_lt(a, self.len());
        if a == b {
            return;
        }
        assert_index_lt(b, self.len());
        match self.indices.get_disjoint_mut(
            [self.entries[a].hash.get(), self.entries[b].hash.get()],
            move |i, &x| if i == 0 { x == a } else { x == b },
        ) {
            [Some(ref_a), Some(ref_b)] => {
                mem::swap(ref_a, ref_b);
                self.entries.swap(a, b);
            }
            _ => panic!("indices not found"),
        }
    }
}
