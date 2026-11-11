use super::{Bucket, IndexSet, IntoIter, Iter};
use crate::indexmap::util::{slice_eq, try_simplify_range};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::{self, Bound, Index, RangeBounds};
#[repr(transparent)]
pub struct Slice<T> {
    pub(crate) entries: [Bucket<T>],
}
impl<T> Slice<T> {
    pub(super) const fn from_slice(entries: &[Bucket<T>]) -> &Self {
        unsafe { &*(entries as *const [Bucket<T>] as *const Self) }
    }
    pub(super) fn from_boxed(entries: Box<[Bucket<T>]>) -> Box<Self> {
        unsafe { Box::from_raw(Box::into_raw(entries) as *mut Self) }
    }
    fn into_boxed(self: Box<Self>) -> Box<[Bucket<T>]> {
        unsafe { Box::from_raw(Box::into_raw(self) as *mut [Bucket<T>]) }
    }
}
impl<T> Slice<T> {
    pub(crate) fn into_entries(self: Box<Self>) -> Vec<Bucket<T>> {
        self.into_boxed().into_vec()
    }
    pub const fn new<'a>() -> &'a Self {
        Self::from_slice(&[])
    }
    pub const fn len(&self) -> usize {
        self.entries.len()
    }
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn get_index(&self, index: usize) -> Option<&T> {
        self.entries.get(index).map(Bucket::key_ref)
    }
    pub fn get_range<R: RangeBounds<usize>>(&self, range: R) -> Option<&Self> {
        let range = try_simplify_range(range, self.entries.len())?;
        self.entries.get(range).map(Self::from_slice)
    }
    pub const fn first(&self) -> Option<&T> {
        if let [first, ..] = &self.entries {
            Some(&first.key)
        } else {
            None
        }
    }
    pub const fn last(&self) -> Option<&T> {
        if let [.., last] = &self.entries {
            Some(&last.key)
        } else {
            None
        }
    }
    #[track_caller]
    pub const fn split_at(&self, index: usize) -> (&Self, &Self) {
        let (first, second) = self.entries.split_at(index);
        (Self::from_slice(first), Self::from_slice(second))
    }
    pub const fn split_at_checked(&self, index: usize) -> Option<(&Self, &Self)> {
        if let Some((first, second)) = self.entries.split_at_checked(index) {
            Some((Self::from_slice(first), Self::from_slice(second)))
        } else {
            None
        }
    }
    pub const fn split_first(&self) -> Option<(&T, &Self)> {
        if let [first, rest @ ..] = &self.entries {
            Some((&first.key, Self::from_slice(rest)))
        } else {
            None
        }
    }
    pub const fn split_last(&self) -> Option<(&T, &Self)> {
        if let [rest @ .., last] = &self.entries {
            Some((&last.key, Self::from_slice(rest)))
        } else {
            None
        }
    }
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(&self.entries)
    }
    pub fn binary_search(&self, x: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        self.binary_search_by(|p| p.cmp(x))
    }
    #[inline]
    pub fn binary_search_by<'a, F>(&'a self, mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> Ordering,
    {
        self.entries.binary_search_by(move |a| f(&a.key))
    }
    #[inline]
    pub fn binary_search_by_key<'a, B, F>(&'a self, b: &B, mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> B,
        B: Ord,
    {
        self.binary_search_by(|k| f(k).cmp(b))
    }
    #[inline]
    pub fn is_sorted(&self) -> bool
    where
        T: PartialOrd,
    {
        self.entries.is_sorted_by(|a, b| a.key <= b.key)
    }
    #[inline]
    pub fn is_sorted_by<'a, F>(&'a self, mut cmp: F) -> bool
    where
        F: FnMut(&'a T, &'a T) -> bool,
    {
        self.entries.is_sorted_by(move |a, b| cmp(&a.key, &b.key))
    }
    #[inline]
    pub fn is_sorted_by_key<'a, F, K>(&'a self, mut sort_key: F) -> bool
    where
        F: FnMut(&'a T) -> K,
        K: PartialOrd,
    {
        self.entries.is_sorted_by_key(move |a| sort_key(&a.key))
    }
    #[must_use]
    pub fn partition_point<P>(&self, mut pred: P) -> usize
    where
        P: FnMut(&T) -> bool,
    {
        self.entries.partition_point(move |a| pred(&a.key))
    }
}
impl<'a, T> IntoIterator for &'a Slice<T> {
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<T> IntoIterator for Box<Slice<T>> {
    type IntoIter = IntoIter<T>;
    type Item = T;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self.into_entries())
    }
}
impl<T> Default for &'_ Slice<T> {
    fn default() -> Self {
        Slice::from_slice(&[])
    }
}
impl<T> Default for Box<Slice<T>> {
    fn default() -> Self {
        Slice::from_boxed(Box::default())
    }
}
impl<T: Clone> Clone for Box<Slice<T>> {
    fn clone(&self) -> Self {
        Slice::from_boxed(self.entries.to_vec().into_boxed_slice())
    }
}
impl<T: Copy> From<&Slice<T>> for Box<Slice<T>> {
    fn from(slice: &Slice<T>) -> Self {
        Slice::from_boxed(Box::from(&slice.entries))
    }
}
impl<T: fmt::Debug> fmt::Debug for Slice<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self).finish()
    }
}
impl<T, U> PartialEq<Slice<U>> for Slice<T>
where
    T: PartialEq<U>,
{
    fn eq(&self, other: &Slice<U>) -> bool {
        slice_eq(&self.entries, &other.entries, |b1, b2| b1.key == b2.key)
    }
}
impl<T, U> PartialEq<[U]> for Slice<T>
where
    T: PartialEq<U>,
{
    fn eq(&self, other: &[U]) -> bool {
        slice_eq(&self.entries, other, |b, o| b.key == *o)
    }
}
impl<T, U> PartialEq<Slice<U>> for [T]
where
    T: PartialEq<U>,
{
    fn eq(&self, other: &Slice<U>) -> bool {
        slice_eq(self, &other.entries, |o, b| *o == b.key)
    }
}
impl<T, U, const N: usize> PartialEq<[U; N]> for Slice<T>
where
    T: PartialEq<U>,
{
    fn eq(&self, other: &[U; N]) -> bool {
        <Self as PartialEq<[U]>>::eq(self, other)
    }
}
impl<T, const N: usize, U> PartialEq<Slice<U>> for [T; N]
where
    T: PartialEq<U>,
{
    fn eq(&self, other: &Slice<U>) -> bool {
        <[T] as PartialEq<Slice<U>>>::eq(self, other)
    }
}
impl<T: Eq> Eq for Slice<T> {}
impl<T: PartialOrd> PartialOrd for Slice<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other)
    }
}
impl<T: Ord> Ord for Slice<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other)
    }
}
impl<T: Hash> Hash for Slice<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for value in self {
            value.hash(state);
        }
    }
}
impl<T> Index<usize> for Slice<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index].key
    }
}
macro_rules! impl_index { ($($range : ty) ,*) => { $(impl < T , S > Index <$range > for IndexSet < T , S > { type Output = Slice < T >; fn index (& self , range : $range) -> & Self::Output { Slice::from_slice (& self . as_entries () [range]) } } impl < T > Index <$range > for Slice < T > { type Output = Self ; fn index (& self , range : $range) -> & Self::Output { Slice::from_slice (& self . entries [range]) } }) * } }
impl_index!(
    ops::Range<usize>,
    ops::RangeFrom<usize>,
    ops::RangeFull,
    ops::RangeInclusive<usize>,
    ops::RangeTo<usize>,
    ops::RangeToInclusive<usize>,
    (Bound<usize>, Bound<usize>)
);
