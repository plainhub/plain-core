mod disjoint;
mod entry;
mod iter;
mod mutable;
pub mod raw_entry_v1;
pub mod serde_seq;
mod slice;
pub use self::entry::{Entry, IndexedEntry};
pub use self::iter::{
    Drain, ExtractIf, IntoIter, IntoKeys, IntoValues, Iter, IterMut, IterMut2, Keys, Splice,
    Values, ValuesMut,
};
pub use self::mutable::MutableEntryKey;
pub use self::mutable::MutableKeys;
pub use self::raw_entry_v1::RawEntryApiV1;
pub use self::slice::Slice;
use crate::indexmap::inner::Core;
pub use crate::indexmap::inner::{OccupiedEntry, VacantEntry};
use crate::indexmap::util::{assert_index_le, assert_index_lt, third, try_simplify_range};
use crate::indexmap::{Bucket, Equivalent, GetDisjointMutError, HashValue, TryReserveError};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::mem;
use core::ops::{Index, IndexMut, RangeBounds};
use std::hash::RandomState;
pub struct IndexMap<K, V, S = RandomState> {
    pub(crate) core: Core<K, V>,
    hash_builder: S,
}
impl<K, V, S> Clone for IndexMap<K, V, S>
where
    K: Clone,
    V: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        IndexMap {
            core: self.core.clone(),
            hash_builder: self.hash_builder.clone(),
        }
    }
    fn clone_from(&mut self, other: &Self) {
        self.core.clone_from(&other.core);
        self.hash_builder.clone_from(&other.hash_builder);
    }
}
impl<K, V, S> fmt::Debug for IndexMap<K, V, S>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
impl<K, V> IndexMap<K, V> {
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }
    #[inline]
    pub fn with_capacity(n: usize) -> Self {
        Self::with_capacity_and_hasher(n, <_>::default())
    }
}
impl<K, V, S> IndexMap<K, V, S> {
    #[inline]
    pub fn with_capacity_and_hasher(n: usize, hash_builder: S) -> Self {
        if n == 0 {
            Self::with_hasher(hash_builder)
        } else {
            IndexMap {
                core: Core::with_capacity(n),
                hash_builder,
            }
        }
    }
    pub const fn with_hasher(hash_builder: S) -> Self {
        IndexMap {
            core: Core::new(),
            hash_builder,
        }
    }
    #[inline]
    pub(crate) fn into_entries(self) -> Vec<Bucket<K, V>> {
        self.core.into_entries()
    }
    #[inline]
    pub(crate) fn as_entries(&self) -> &[Bucket<K, V>] {
        self.core.as_entries()
    }
    #[inline]
    pub(crate) fn as_entries_mut(&mut self) -> &mut [Bucket<K, V>] {
        self.core.as_entries_mut()
    }
    pub(crate) fn with_entries<F>(&mut self, f: F)
    where
        F: FnOnce(&mut [Bucket<K, V>]),
    {
        self.core.with_entries(f);
    }
    pub fn capacity(&self) -> usize {
        self.core.capacity()
    }
    pub fn hasher(&self) -> &S {
        &self.hash_builder
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.core.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter::new(self.as_entries())
    }
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut::new(self.as_entries_mut())
    }
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys::new(self.as_entries())
    }
    pub fn into_keys(self) -> IntoKeys<K, V> {
        IntoKeys::new(self.into_entries())
    }
    pub fn values(&self) -> Values<'_, K, V> {
        Values::new(self.as_entries())
    }
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut::new(self.as_entries_mut())
    }
    pub fn into_values(self) -> IntoValues<K, V> {
        IntoValues::new(self.into_entries())
    }
    pub fn clear(&mut self) {
        self.core.clear();
    }
    pub fn truncate(&mut self, len: usize) {
        self.core.truncate(len);
    }
    #[track_caller]
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, K, V>
    where
        R: RangeBounds<usize>,
    {
        Drain::new(self.core.drain(range))
    }
    #[track_caller]
    pub fn extract_if<F, R>(&mut self, range: R, pred: F) -> ExtractIf<'_, K, V, F>
    where
        F: FnMut(&K, &mut V) -> bool,
        R: RangeBounds<usize>,
    {
        ExtractIf::new(&mut self.core, range, pred)
    }
    #[track_caller]
    pub fn split_off(&mut self, at: usize) -> Self
    where
        S: Clone,
    {
        Self {
            core: self.core.split_off(at),
            hash_builder: self.hash_builder.clone(),
        }
    }
    pub fn reserve(&mut self, additional: usize) {
        self.core.reserve(additional);
    }
    pub fn reserve_exact(&mut self, additional: usize) {
        self.core.reserve_exact(additional);
    }
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.core.try_reserve(additional)
    }
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.core.try_reserve_exact(additional)
    }
    pub fn shrink_to_fit(&mut self) {
        self.core.shrink_to(0);
    }
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.core.shrink_to(min_capacity);
    }
}
impl<K, V, S> IndexMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert_full(key, value).1
    }
    pub fn insert_full(&mut self, key: K, value: V) -> (usize, Option<V>) {
        let hash = self.hash(&key);
        self.core.insert_full(hash, key, value)
    }
    pub fn insert_sorted(&mut self, key: K, value: V) -> (usize, Option<V>)
    where
        K: Ord,
    {
        match self.binary_search_keys(&key) {
            Ok(i) => (i, Some(mem::replace(&mut self[i], value))),
            Err(i) => self.insert_before(i, key, value),
        }
    }
    pub fn insert_sorted_by<F>(&mut self, key: K, value: V, mut cmp: F) -> (usize, Option<V>)
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        let (Ok(i) | Err(i)) = self.binary_search_by(|k, v| cmp(k, v, &key, &value));
        self.insert_before(i, key, value)
    }
    pub fn insert_sorted_by_key<B, F>(
        &mut self,
        key: K,
        value: V,
        mut sort_key: F,
    ) -> (usize, Option<V>)
    where
        B: Ord,
        F: FnMut(&K, &V) -> B,
    {
        let search_key = sort_key(&key, &value);
        let (Ok(i) | Err(i)) = self.binary_search_by_key(&search_key, sort_key);
        self.insert_before(i, key, value)
    }
    #[track_caller]
    pub fn insert_before(&mut self, mut index: usize, key: K, value: V) -> (usize, Option<V>) {
        assert_index_le(index, self.len());
        match self.entry(key) {
            Entry::Occupied(mut entry) => {
                if index > entry.index() {
                    index -= 1;
                }
                let old = mem::replace(entry.get_mut(), value);
                entry.move_index(index);
                (index, Some(old))
            }
            Entry::Vacant(entry) => {
                entry.shift_insert(index, value);
                (index, None)
            }
        }
    }
    #[track_caller]
    pub fn shift_insert(&mut self, index: usize, key: K, value: V) -> Option<V> {
        let len = self.len();
        match self.entry(key) {
            Entry::Occupied(mut entry) => {
                assert_index_lt(index, len);
                let old = mem::replace(entry.get_mut(), value);
                entry.move_index(index);
                Some(old)
            }
            Entry::Vacant(entry) => {
                assert_index_le(index, len);
                entry.shift_insert(index, value);
                None
            }
        }
    }
    #[track_caller]
    pub fn replace_index(&mut self, index: usize, key: K) -> Result<K, (usize, K)> {
        assert_index_lt(index, self.len());
        let entry = &mut self.as_entries_mut()[index];
        if key == entry.key {
            return Ok(mem::replace(&mut entry.key, key));
        }
        let hash = self.hash(&key);
        if let Some(i) = self.core.get_index_of(hash, &key) {
            debug_assert_ne!(i, index);
            return Err((i, key));
        }
        Ok(self.core.replace_index_unique(index, hash, key))
    }
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        let hash = self.hash(&key);
        Entry::new(&mut self.core, hash, key)
    }
    #[track_caller]
    pub fn splice<R, I>(&mut self, range: R, replace_with: I) -> Splice<'_, I::IntoIter, K, V, S>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = (K, V)>,
    {
        Splice::new(self, range, replace_with.into_iter())
    }
    pub fn append<S2>(&mut self, other: &mut IndexMap<K, V, S2>) {
        self.extend(other.drain(..));
    }
}
impl<K, V, S> IndexMap<K, V, S>
where
    S: BuildHasher,
{
    pub(crate) fn hash<Q: ?Sized + Hash>(&self, key: &Q) -> HashValue {
        let h = self.hash_builder.hash_one(key);
        HashValue(h as usize)
    }
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.get_index_of(key).is_some()
    }
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &self.as_entries()[i];
            Some(&entry.value)
        } else {
            None
        }
    }
    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, &V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &self.as_entries()[i];
            Some((&entry.key, &entry.value))
        } else {
            None
        }
    }
    pub fn get_full<Q>(&self, key: &Q) -> Option<(usize, &K, &V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &self.as_entries()[i];
            Some((i, &entry.key, &entry.value))
        } else {
            None
        }
    }
    pub fn get_index_of<Q>(&self, key: &Q) -> Option<usize>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        match self.as_entries() {
            [] => None,
            [x] => key.equivalent(&x.key).then_some(0),
            _ => {
                let hash = self.hash(key);
                self.core.get_index_of(hash, key)
            }
        }
    }
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &mut self.as_entries_mut()[i];
            Some(&mut entry.value)
        } else {
            None
        }
    }
    pub fn get_key_value_mut<Q>(&mut self, key: &Q) -> Option<(&K, &mut V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &mut self.as_entries_mut()[i];
            Some((&entry.key, &mut entry.value))
        } else {
            None
        }
    }

    #[track_caller]
    pub fn get_disjoint_mut<Q, const N: usize>(&mut self, keys: [&Q; N]) -> [Option<&mut V>; N]
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        let indices = keys.map(|key| self.get_index_of(key));
        disjoint::get_disjoint_opt_mut(self.as_entries_mut(), indices)
            .map(|opt| opt.map(Bucket::value_mut))
    }
    pub fn swap_remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.swap_remove_full(key).map(third)
    }
    pub fn swap_remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        match self.swap_remove_full(key) {
            Some((_, key, value)) => Some((key, value)),
            None => None,
        }
    }
    pub fn swap_remove_full<Q>(&mut self, key: &Q) -> Option<(usize, K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        match self.as_entries() {
            [x] if key.equivalent(&x.key) => {
                let (k, v) = self.core.pop()?;
                Some((0, k, v))
            }
            [_] | [] => None,
            _ => {
                let hash = self.hash(key);
                self.core.swap_remove_full(hash, key)
            }
        }
    }
    pub fn shift_remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.shift_remove_full(key).map(third)
    }
    pub fn shift_remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        match self.shift_remove_full(key) {
            Some((_, key, value)) => Some((key, value)),
            None => None,
        }
    }
    pub fn shift_remove_full<Q>(&mut self, key: &Q) -> Option<(usize, K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        match self.as_entries() {
            [x] if key.equivalent(&x.key) => {
                let (k, v) = self.core.pop()?;
                Some((0, k, v))
            }
            [_] | [] => None,
            _ => {
                let hash = self.hash(key);
                self.core.shift_remove_full(hash, key)
            }
        }
    }
}
impl<K, V, S> IndexMap<K, V, S> {
    pub fn pop(&mut self) -> Option<(K, V)> {
        self.core.pop()
    }
    pub fn pop_if(&mut self, predicate: impl FnOnce(&K, &mut V) -> bool) -> Option<(K, V)> {
        let (last_key, last_value) = self.last_mut()?;
        if predicate(last_key, last_value) {
            self.core.pop()
        } else {
            None
        }
    }
    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        self.core.retain_in_order(move |k, v| keep(k, v));
    }
    pub fn sort_keys(&mut self)
    where
        K: Ord,
    {
        self.with_entries(move |entries| {
            entries.sort_by(move |a, b| K::cmp(&a.key, &b.key));
        });
    }
    pub fn sort_by<F>(&mut self, mut cmp: F)
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        self.with_entries(move |entries| {
            entries.sort_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        });
    }
    pub fn sorted_by<F>(self, mut cmp: F) -> IntoIter<K, V>
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        let mut entries = self.into_entries();
        entries.sort_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        IntoIter::new(entries)
    }
    pub fn sort_by_key<T, F>(&mut self, mut sort_key: F)
    where
        T: Ord,
        F: FnMut(&K, &V) -> T,
    {
        self.with_entries(move |entries| {
            entries.sort_by_key(move |a| sort_key(&a.key, &a.value));
        });
    }
    pub fn sort_unstable_keys(&mut self)
    where
        K: Ord,
    {
        self.with_entries(move |entries| {
            entries.sort_unstable_by(move |a, b| K::cmp(&a.key, &b.key));
        });
    }
    pub fn sort_unstable_by<F>(&mut self, mut cmp: F)
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        self.with_entries(move |entries| {
            entries.sort_unstable_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        });
    }
    #[inline]
    pub fn sorted_unstable_by<F>(self, mut cmp: F) -> IntoIter<K, V>
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        let mut entries = self.into_entries();
        entries.sort_unstable_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        IntoIter::new(entries)
    }
    pub fn sort_unstable_by_key<T, F>(&mut self, mut sort_key: F)
    where
        T: Ord,
        F: FnMut(&K, &V) -> T,
    {
        self.with_entries(move |entries| {
            entries.sort_unstable_by_key(move |a| sort_key(&a.key, &a.value));
        });
    }
    pub fn sort_by_cached_key<T, F>(&mut self, mut sort_key: F)
    where
        T: Ord,
        F: FnMut(&K, &V) -> T,
    {
        self.with_entries(move |entries| {
            entries.sort_by_cached_key(move |a| sort_key(&a.key, &a.value));
        });
    }
    pub fn binary_search_keys(&self, x: &K) -> Result<usize, usize>
    where
        K: Ord,
    {
        self.as_slice().binary_search_keys(x)
    }
    #[inline]
    pub fn binary_search_by<'a, F>(&'a self, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a K, &'a V) -> Ordering,
    {
        self.as_slice().binary_search_by(f)
    }
    #[inline]
    pub fn binary_search_by_key<'a, B, F>(&'a self, b: &B, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a K, &'a V) -> B,
        B: Ord,
    {
        self.as_slice().binary_search_by_key(b, f)
    }
    #[inline]
    pub fn is_sorted(&self) -> bool
    where
        K: PartialOrd,
    {
        self.as_slice().is_sorted()
    }
    #[inline]
    pub fn is_sorted_by<'a, F>(&'a self, cmp: F) -> bool
    where
        F: FnMut(&'a K, &'a V, &'a K, &'a V) -> bool,
    {
        self.as_slice().is_sorted_by(cmp)
    }
    #[inline]
    pub fn is_sorted_by_key<'a, F, T>(&'a self, sort_key: F) -> bool
    where
        F: FnMut(&'a K, &'a V) -> T,
        T: PartialOrd,
    {
        self.as_slice().is_sorted_by_key(sort_key)
    }
    #[must_use]
    pub fn partition_point<P>(&self, pred: P) -> usize
    where
        P: FnMut(&K, &V) -> bool,
    {
        self.as_slice().partition_point(pred)
    }
    pub fn reverse(&mut self) {
        self.core.reverse()
    }
    pub fn as_slice(&self) -> &Slice<K, V> {
        Slice::from_slice(self.as_entries())
    }
    pub fn as_mut_slice(&mut self) -> &mut Slice<K, V> {
        Slice::from_mut_slice(self.as_entries_mut())
    }
    pub fn into_boxed_slice(self) -> Box<Slice<K, V>> {
        Slice::from_boxed(self.into_entries().into_boxed_slice())
    }
    pub fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        self.as_entries().get(index).map(Bucket::refs)
    }
    pub fn get_index_mut(&mut self, index: usize) -> Option<(&K, &mut V)> {
        self.as_entries_mut().get_mut(index).map(Bucket::ref_mut)
    }
    pub fn get_index_entry(&mut self, index: usize) -> Option<IndexedEntry<'_, K, V>> {
        IndexedEntry::new(&mut self.core, index)
    }

    pub fn get_range<R: RangeBounds<usize>>(&self, range: R) -> Option<&Slice<K, V>> {
        let entries = self.as_entries();
        let range = try_simplify_range(range, entries.len())?;
        entries.get(range).map(Slice::from_slice)
    }
    pub fn get_range_mut<R: RangeBounds<usize>>(&mut self, range: R) -> Option<&mut Slice<K, V>> {
        let entries = self.as_entries_mut();
        let range = try_simplify_range(range, entries.len())?;
        entries.get_mut(range).map(Slice::from_mut_slice)
    }
    pub fn first(&self) -> Option<(&K, &V)> {
        self.as_entries().first().map(Bucket::refs)
    }
    pub fn first_mut(&mut self) -> Option<(&K, &mut V)> {
        self.as_entries_mut().first_mut().map(Bucket::ref_mut)
    }

    pub fn last(&self) -> Option<(&K, &V)> {
        self.as_entries().last().map(Bucket::refs)
    }
    pub fn last_mut(&mut self) -> Option<(&K, &mut V)> {
        self.as_entries_mut().last_mut().map(Bucket::ref_mut)
    }

    pub fn swap_remove_index(&mut self, index: usize) -> Option<(K, V)> {
        self.core.swap_remove_index(index)
    }
    pub fn shift_remove_index(&mut self, index: usize) -> Option<(K, V)> {
        self.core.shift_remove_index(index)
    }
    #[track_caller]
    pub fn move_index(&mut self, from: usize, to: usize) {
        self.core.move_index(from, to)
    }
    #[track_caller]
    pub fn swap_indices(&mut self, a: usize, b: usize) {
        self.core.swap_indices(a, b)
    }
}
impl<K, V, Q: ?Sized, S> Index<&Q> for IndexMap<K, V, S>
where
    Q: Hash + Equivalent<K>,
    S: BuildHasher,
{
    type Output = V;
    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("no entry found for key")
    }
}
impl<K, V, Q: ?Sized, S> IndexMut<&Q> for IndexMap<K, V, S>
where
    Q: Hash + Equivalent<K>,
    S: BuildHasher,
{
    fn index_mut(&mut self, key: &Q) -> &mut V {
        self.get_mut(key).expect("no entry found for key")
    }
}
impl<K, V, S> Index<usize> for IndexMap<K, V, S> {
    type Output = V;
    fn index(&self, index: usize) -> &V {
        assert_index_lt(index, self.len());
        &self.as_entries()[index].value
    }
}
impl<K, V, S> IndexMut<usize> for IndexMap<K, V, S> {
    fn index_mut(&mut self, index: usize) -> &mut V {
        assert_index_lt(index, self.len());
        &mut self.as_entries_mut()[index].value
    }
}
impl<K, V, S> FromIterator<(K, V)> for IndexMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher + Default,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iterable: I) -> Self {
        let iter = iterable.into_iter();
        let (low, _) = iter.size_hint();
        let mut map = Self::with_capacity_and_hasher(low, <_>::default());
        map.extend(iter);
        map
    }
}
impl<K, V, const N: usize> From<[(K, V); N]> for IndexMap<K, V, RandomState>
where
    K: Hash + Eq,
{
    fn from(arr: [(K, V); N]) -> Self {
        Self::from_iter(arr)
    }
}
impl<K, V, S> Extend<(K, V)> for IndexMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iterable: I) {
        let iter = iterable.into_iter();
        let (lower_len, _) = iter.size_hint();
        let reserve = if self.is_empty() {
            lower_len
        } else {
            lower_len.div_ceil(2)
        };
        self.reserve(reserve);
        iter.for_each(move |(k, v)| {
            self.insert(k, v);
        });
    }
}
impl<'a, K, V, S> Extend<(&'a K, &'a V)> for IndexMap<K, V, S>
where
    K: Hash + Eq + Copy,
    V: Copy,
    S: BuildHasher,
{
    fn extend<I: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iterable: I) {
        self.extend(iterable.into_iter().map(|(&key, &value)| (key, value)));
    }
}
impl<K, V, S> Default for IndexMap<K, V, S>
where
    S: Default,
{
    fn default() -> Self {
        Self::with_capacity_and_hasher(0, S::default())
    }
}
impl<K, V1, S1, V2, S2> PartialEq<IndexMap<K, V2, S2>> for IndexMap<K, V1, S1>
where
    K: Hash + Eq,
    V1: PartialEq<V2>,
    S1: BuildHasher,
    S2: BuildHasher,
{
    fn eq(&self, other: &IndexMap<K, V2, S2>) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter()
            .all(|(key, value)| other.get(key).map_or(false, |v| *value == *v))
    }
}
impl<K, V, S> Eq for IndexMap<K, V, S>
where
    K: Eq + Hash,
    V: Eq,
    S: BuildHasher,
{
}
