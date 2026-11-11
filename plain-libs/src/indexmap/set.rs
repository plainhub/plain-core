mod iter;
mod mutable;
mod slice;
pub use self::iter::{
    Difference, Drain, ExtractIf, Intersection, IntoIter, Iter, Splice, SymmetricDifference, Union,
};
pub use self::mutable::MutableValues;
pub use self::slice::Slice;
use super::{Equivalent, IndexMap};
use crate::indexmap::TryReserveError;
use crate::indexmap::util::{assert_index_lt, try_simplify_range};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::ops::{BitAnd, BitOr, BitXor, Index, RangeBounds, Sub};
use std::hash::RandomState;
type Bucket<T> = super::Bucket<T, ()>;
pub struct IndexSet<T, S = RandomState> {
    pub(crate) map: IndexMap<T, (), S>,
}
impl<T, S> Clone for IndexSet<T, S>
where
    T: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        IndexSet {
            map: self.map.clone(),
        }
    }
    fn clone_from(&mut self, other: &Self) {
        self.map.clone_from(&other.map);
    }
}
impl<T, S> fmt::Debug for IndexSet<T, S>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
impl<T> IndexSet<T> {
    pub fn new() -> Self {
        IndexSet {
            map: IndexMap::new(),
        }
    }
    pub fn with_capacity(n: usize) -> Self {
        IndexSet {
            map: IndexMap::with_capacity(n),
        }
    }
}
impl<T, S> IndexSet<T, S> {
    pub fn with_capacity_and_hasher(n: usize, hash_builder: S) -> Self {
        IndexSet {
            map: IndexMap::with_capacity_and_hasher(n, hash_builder),
        }
    }
    pub const fn with_hasher(hash_builder: S) -> Self {
        IndexSet {
            map: IndexMap::with_hasher(hash_builder),
        }
    }
    #[inline]
    pub(crate) fn into_entries(self) -> Vec<Bucket<T>> {
        self.map.into_entries()
    }
    #[inline]
    pub(crate) fn as_entries(&self) -> &[Bucket<T>] {
        self.map.as_entries()
    }
    pub(crate) fn with_entries<F>(&mut self, f: F)
    where
        F: FnOnce(&mut [Bucket<T>]),
    {
        self.map.with_entries(f);
    }
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }
    pub fn hasher(&self) -> &S {
        self.map.hasher()
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(self.as_entries())
    }
    pub fn clear(&mut self) {
        self.map.clear();
    }
    pub fn truncate(&mut self, len: usize) {
        self.map.truncate(len);
    }
    #[track_caller]
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, T>
    where
        R: RangeBounds<usize>,
    {
        Drain::new(self.map.core.drain(range))
    }
    #[track_caller]
    pub fn extract_if<F, R>(&mut self, range: R, pred: F) -> ExtractIf<'_, T, F>
    where
        F: FnMut(&T) -> bool,
        R: RangeBounds<usize>,
    {
        ExtractIf::new(&mut self.map.core, range, pred)
    }
    #[track_caller]
    pub fn split_off(&mut self, at: usize) -> Self
    where
        S: Clone,
    {
        Self {
            map: self.map.split_off(at),
        }
    }
    pub fn reserve(&mut self, additional: usize) {
        self.map.reserve(additional);
    }
    pub fn reserve_exact(&mut self, additional: usize) {
        self.map.reserve_exact(additional);
    }
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.map.try_reserve(additional)
    }
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.map.try_reserve_exact(additional)
    }
    pub fn shrink_to_fit(&mut self) {
        self.map.shrink_to_fit();
    }
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.map.shrink_to(min_capacity);
    }
}
impl<T, S> IndexSet<T, S>
where
    T: Hash + Eq,
    S: BuildHasher,
{
    pub fn insert(&mut self, value: T) -> bool {
        self.map.insert(value, ()).is_none()
    }
    pub fn insert_full(&mut self, value: T) -> (usize, bool) {
        let (index, existing) = self.map.insert_full(value, ());
        (index, existing.is_none())
    }
    pub fn insert_sorted(&mut self, value: T) -> (usize, bool)
    where
        T: Ord,
    {
        let (index, existing) = self.map.insert_sorted(value, ());
        (index, existing.is_none())
    }
    pub fn insert_sorted_by<F>(&mut self, value: T, mut cmp: F) -> (usize, bool)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        let (index, existing) = self
            .map
            .insert_sorted_by(value, (), |a, (), b, ()| cmp(a, b));
        (index, existing.is_none())
    }
    pub fn insert_sorted_by_key<B, F>(&mut self, value: T, mut sort_key: F) -> (usize, bool)
    where
        B: Ord,
        F: FnMut(&T) -> B,
    {
        let (index, existing) = self.map.insert_sorted_by_key(value, (), |k, _| sort_key(k));
        (index, existing.is_none())
    }
    #[track_caller]
    pub fn insert_before(&mut self, index: usize, value: T) -> (usize, bool) {
        let (index, existing) = self.map.insert_before(index, value, ());
        (index, existing.is_none())
    }
    #[track_caller]
    pub fn shift_insert(&mut self, index: usize, value: T) -> bool {
        self.map.shift_insert(index, value, ()).is_none()
    }
    pub fn replace(&mut self, value: T) -> Option<T> {
        self.replace_full(value).1
    }
    pub fn replace_full(&mut self, value: T) -> (usize, Option<T>) {
        let hash = self.map.hash(&value);
        match self.map.core.replace_full(hash, value, ()) {
            (i, Some((replaced, ()))) => (i, Some(replaced)),
            (i, None) => (i, None),
        }
    }
    #[track_caller]
    pub fn replace_index(&mut self, index: usize, value: T) -> Result<T, (usize, T)> {
        self.map.replace_index(index, value)
    }
    pub fn difference<'a, S2>(&'a self, other: &'a IndexSet<T, S2>) -> Difference<'a, T, S2>
    where
        S2: BuildHasher,
    {
        Difference::new(self, other)
    }
    pub fn symmetric_difference<'a, S2>(
        &'a self,
        other: &'a IndexSet<T, S2>,
    ) -> SymmetricDifference<'a, T, S, S2>
    where
        S2: BuildHasher,
    {
        SymmetricDifference::new(self, other)
    }
    pub fn intersection<'a, S2>(&'a self, other: &'a IndexSet<T, S2>) -> Intersection<'a, T, S2>
    where
        S2: BuildHasher,
    {
        Intersection::new(self, other)
    }
    pub fn union<'a, S2>(&'a self, other: &'a IndexSet<T, S2>) -> Union<'a, T, S>
    where
        S2: BuildHasher,
    {
        Union::new(self, other)
    }
    #[track_caller]
    pub fn splice<R, I>(&mut self, range: R, replace_with: I) -> Splice<'_, I::IntoIter, T, S>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = T>,
    {
        Splice::new(self, range, replace_with.into_iter())
    }
    pub fn append<S2>(&mut self, other: &mut IndexSet<T, S2>) {
        self.map.append(&mut other.map);
    }
}
impl<T, S> IndexSet<T, S>
where
    S: BuildHasher,
{
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.contains_key(value)
    }
    pub fn get<Q>(&self, value: &Q) -> Option<&T>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.get_key_value(value).map(|(x, &())| x)
    }
    pub fn get_full<Q>(&self, value: &Q) -> Option<(usize, &T)>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.get_full(value).map(|(i, x, &())| (i, x))
    }
    pub fn get_index_of<Q>(&self, value: &Q) -> Option<usize>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.get_index_of(value)
    }
    pub fn swap_remove<Q>(&mut self, value: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.swap_remove(value).is_some()
    }
    pub fn shift_remove<Q>(&mut self, value: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.shift_remove(value).is_some()
    }

    pub fn swap_remove_full<Q>(&mut self, value: &Q) -> Option<(usize, T)>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.swap_remove_full(value).map(|(i, x, ())| (i, x))
    }
    pub fn shift_remove_full<Q>(&mut self, value: &Q) -> Option<(usize, T)>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.shift_remove_full(value).map(|(i, x, ())| (i, x))
    }
}
impl<T, S> IndexSet<T, S> {
    pub fn pop(&mut self) -> Option<T> {
        self.map.pop().map(|(x, ())| x)
    }
    pub fn pop_if(&mut self, predicate: impl FnOnce(&T) -> bool) -> Option<T> {
        let last = self.last()?;
        if predicate(last) { self.pop() } else { None }
    }
    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.map.retain(move |x, &mut ()| keep(x))
    }
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.map.sort_keys()
    }
    pub fn sort_by<F>(&mut self, mut cmp: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.map.sort_by(move |a, (), b, ()| cmp(a, b));
    }
    pub fn sorted_by<F>(self, mut cmp: F) -> IntoIter<T>
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        let mut entries = self.into_entries();
        entries.sort_by(move |a, b| cmp(&a.key, &b.key));
        IntoIter::new(entries)
    }
    pub fn sort_by_key<K, F>(&mut self, mut sort_key: F)
    where
        K: Ord,
        F: FnMut(&T) -> K,
    {
        self.with_entries(move |entries| {
            entries.sort_by_key(move |a| sort_key(&a.key));
        });
    }
    pub fn sort_unstable(&mut self)
    where
        T: Ord,
    {
        self.map.sort_unstable_keys()
    }
    pub fn sort_unstable_by<F>(&mut self, mut cmp: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.map.sort_unstable_by(move |a, _, b, _| cmp(a, b))
    }
    pub fn sorted_unstable_by<F>(self, mut cmp: F) -> IntoIter<T>
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        let mut entries = self.into_entries();
        entries.sort_unstable_by(move |a, b| cmp(&a.key, &b.key));
        IntoIter::new(entries)
    }
    pub fn sort_unstable_by_key<K, F>(&mut self, mut sort_key: F)
    where
        K: Ord,
        F: FnMut(&T) -> K,
    {
        self.with_entries(move |entries| {
            entries.sort_unstable_by_key(move |a| sort_key(&a.key));
        });
    }
    pub fn sort_by_cached_key<K, F>(&mut self, mut sort_key: F)
    where
        K: Ord,
        F: FnMut(&T) -> K,
    {
        self.with_entries(move |entries| {
            entries.sort_by_cached_key(move |a| sort_key(&a.key));
        });
    }
    pub fn binary_search(&self, x: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        self.as_slice().binary_search(x)
    }
    #[inline]
    pub fn binary_search_by<'a, F>(&'a self, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> Ordering,
    {
        self.as_slice().binary_search_by(f)
    }
    #[inline]
    pub fn binary_search_by_key<'a, B, F>(&'a self, b: &B, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> B,
        B: Ord,
    {
        self.as_slice().binary_search_by_key(b, f)
    }
    #[inline]
    pub fn is_sorted(&self) -> bool
    where
        T: PartialOrd,
    {
        self.as_slice().is_sorted()
    }
    #[inline]
    pub fn is_sorted_by<'a, F>(&'a self, cmp: F) -> bool
    where
        F: FnMut(&'a T, &'a T) -> bool,
    {
        self.as_slice().is_sorted_by(cmp)
    }
    #[inline]
    pub fn is_sorted_by_key<'a, F, K>(&'a self, sort_key: F) -> bool
    where
        F: FnMut(&'a T) -> K,
        K: PartialOrd,
    {
        self.as_slice().is_sorted_by_key(sort_key)
    }
    #[must_use]
    pub fn partition_point<P>(&self, pred: P) -> usize
    where
        P: FnMut(&T) -> bool,
    {
        self.as_slice().partition_point(pred)
    }
    pub fn reverse(&mut self) {
        self.map.reverse()
    }
    pub fn as_slice(&self) -> &Slice<T> {
        Slice::from_slice(self.as_entries())
    }
    pub fn into_boxed_slice(self) -> Box<Slice<T>> {
        Slice::from_boxed(self.into_entries().into_boxed_slice())
    }
    pub fn get_index(&self, index: usize) -> Option<&T> {
        self.as_entries().get(index).map(Bucket::key_ref)
    }
    pub fn get_range<R: RangeBounds<usize>>(&self, range: R) -> Option<&Slice<T>> {
        let entries = self.as_entries();
        let range = try_simplify_range(range, entries.len())?;
        entries.get(range).map(Slice::from_slice)
    }
    pub fn first(&self) -> Option<&T> {
        self.as_entries().first().map(Bucket::key_ref)
    }
    pub fn last(&self) -> Option<&T> {
        self.as_entries().last().map(Bucket::key_ref)
    }
    pub fn swap_remove_index(&mut self, index: usize) -> Option<T> {
        self.map.swap_remove_index(index).map(|(x, ())| x)
    }
    pub fn shift_remove_index(&mut self, index: usize) -> Option<T> {
        self.map.shift_remove_index(index).map(|(x, ())| x)
    }
    #[track_caller]
    pub fn move_index(&mut self, from: usize, to: usize) {
        self.map.move_index(from, to)
    }
    #[track_caller]
    pub fn swap_indices(&mut self, a: usize, b: usize) {
        self.map.swap_indices(a, b)
    }
}
impl<T, S> Index<usize> for IndexSet<T, S> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        assert_index_lt(index, self.len());
        &self.map.as_entries()[index].key
    }
}
impl<T, S> FromIterator<T> for IndexSet<T, S>
where
    T: Hash + Eq,
    S: BuildHasher + Default,
{
    fn from_iter<I: IntoIterator<Item = T>>(iterable: I) -> Self {
        let iter = iterable.into_iter().map(|x| (x, ()));
        IndexSet {
            map: IndexMap::from_iter(iter),
        }
    }
}
impl<T, const N: usize> From<[T; N]> for IndexSet<T, RandomState>
where
    T: Eq + Hash,
{
    fn from(arr: [T; N]) -> Self {
        Self::from_iter(arr)
    }
}
impl<T, S> Extend<T> for IndexSet<T, S>
where
    T: Hash + Eq,
    S: BuildHasher,
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iterable: I) {
        let iter = iterable.into_iter().map(|x| (x, ()));
        self.map.extend(iter);
    }
}
impl<'a, T, S> Extend<&'a T> for IndexSet<T, S>
where
    T: Hash + Eq + Copy + 'a,
    S: BuildHasher,
{
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iterable: I) {
        let iter = iterable.into_iter().copied();
        self.extend(iter);
    }
}
impl<T, S> Default for IndexSet<T, S>
where
    S: Default,
{
    fn default() -> Self {
        IndexSet {
            map: IndexMap::default(),
        }
    }
}
impl<T, S1, S2> PartialEq<IndexSet<T, S2>> for IndexSet<T, S1>
where
    T: Hash + Eq,
    S1: BuildHasher,
    S2: BuildHasher,
{
    fn eq(&self, other: &IndexSet<T, S2>) -> bool {
        self.len() == other.len() && self.is_subset(other)
    }
}
impl<T, S> Eq for IndexSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
}
impl<T, S> IndexSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    pub fn is_disjoint<S2>(&self, other: &IndexSet<T, S2>) -> bool
    where
        S2: BuildHasher,
    {
        if self.len() <= other.len() {
            self.iter().all(move |value| !other.contains(value))
        } else {
            other.iter().all(move |value| !self.contains(value))
        }
    }
    pub fn is_subset<S2>(&self, other: &IndexSet<T, S2>) -> bool
    where
        S2: BuildHasher,
    {
        self.len() <= other.len() && self.iter().all(move |value| other.contains(value))
    }
}
impl<T, S1, S2> BitAnd<&IndexSet<T, S2>> for &IndexSet<T, S1>
where
    T: Eq + Hash + Clone,
    S1: BuildHasher + Default,
    S2: BuildHasher,
{
    type Output = IndexSet<T, S1>;
    fn bitand(self, other: &IndexSet<T, S2>) -> Self::Output {
        self.intersection(other).cloned().collect()
    }
}
impl<T, S1, S2> BitOr<&IndexSet<T, S2>> for &IndexSet<T, S1>
where
    T: Eq + Hash + Clone,
    S1: BuildHasher + Default,
    S2: BuildHasher,
{
    type Output = IndexSet<T, S1>;
    fn bitor(self, other: &IndexSet<T, S2>) -> Self::Output {
        self.union(other).cloned().collect()
    }
}
impl<T, S1, S2> BitXor<&IndexSet<T, S2>> for &IndexSet<T, S1>
where
    T: Eq + Hash + Clone,
    S1: BuildHasher + Default,
    S2: BuildHasher,
{
    type Output = IndexSet<T, S1>;
    fn bitxor(self, other: &IndexSet<T, S2>) -> Self::Output {
        self.symmetric_difference(other).cloned().collect()
    }
}
impl<T, S1, S2> Sub<&IndexSet<T, S2>> for &IndexSet<T, S1>
where
    T: Eq + Hash + Clone,
    S1: BuildHasher + Default,
    S2: BuildHasher,
{
    type Output = IndexSet<T, S1>;
    fn sub(self, other: &IndexSet<T, S2>) -> Self::Output {
        self.difference(other).cloned().collect()
    }
}
