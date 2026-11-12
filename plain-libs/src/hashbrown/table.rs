use crate::hashbrown::{
    TryReserveError,
    alloc::{Allocator, Global},
    control::Tag,
    raw::{
        Bucket, FullBucketsIndices, RawDrain, RawExtractIf, RawIntoIter, RawIter, RawIterHash,
        RawIterHashIndices, RawTable,
    },
};
use core::{fmt, iter::FusedIterator, marker::PhantomData, ptr::NonNull};
pub struct HashTable<T, A = Global>
where
    A: Allocator,
{
    pub(crate) raw: RawTable<T, A>,
}
impl<T> HashTable<T, Global> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            raw: RawTable::new(),
        }
    }
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            raw: RawTable::with_capacity(capacity),
        }
    }
}
impl<T, A> HashTable<T, A>
where
    A: Allocator,
{
    #[must_use]
    pub const fn new_in(alloc: A) -> Self {
        Self {
            raw: RawTable::new_in(alloc),
        }
    }
    #[must_use]
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self {
            raw: RawTable::with_capacity_in(capacity, alloc),
        }
    }
    pub fn allocator(&self) -> &A {
        self.raw.allocator()
    }
    pub fn find(&self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<&T> {
        self.raw.get(hash, eq)
    }
    #[inline(always)]
    pub fn find_mut(&mut self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<&mut T> {
        self.raw.get_mut(hash, eq)
    }
    pub fn find_entry(
        &mut self,
        hash: u64,
        eq: impl FnMut(&T) -> bool,
    ) -> Result<OccupiedEntry<'_, T, A>, AbsentEntry<'_, T, A>> {
        match self.raw.find(hash, eq) {
            Some(bucket) => Ok(OccupiedEntry {
                bucket,
                table: self,
            }),
            None => Err(AbsentEntry { table: self }),
        }
    }
    pub fn find_bucket_index(&self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<usize> {
        match self.raw.find(hash, eq) {
            Some(bucket) => Some(unsafe { self.raw.bucket_index(&bucket) }),
            None => None,
        }
    }
    #[inline(always)]
    pub fn entry(
        &mut self,
        hash: u64,
        eq: impl FnMut(&T) -> bool,
        hasher: impl Fn(&T) -> u64,
    ) -> Entry<'_, T, A> {
        match self.raw.find_or_find_insert_index(hash, eq, hasher) {
            Ok(bucket) => Entry::Occupied(OccupiedEntry {
                bucket,
                table: self,
            }),
            Err(insert_index) => Entry::Vacant(VacantEntry {
                tag: Tag::full(hash),
                index: insert_index,
                table: self,
            }),
        }
    }
    #[inline]
    pub fn get_bucket_entry(
        &mut self,
        index: usize,
    ) -> Result<OccupiedEntry<'_, T, A>, AbsentEntry<'_, T, A>> {
        match self.raw.checked_bucket(index) {
            Some(bucket) => Ok(OccupiedEntry {
                bucket,
                table: self,
            }),
            None => Err(AbsentEntry { table: self }),
        }
    }

    #[inline]
    pub fn get_bucket(&self, index: usize) -> Option<&T> {
        self.raw.get_bucket(index)
    }

    #[inline]
    pub fn get_bucket_mut(&mut self, index: usize) -> Option<&mut T> {
        self.raw.get_bucket_mut(index)
    }

    pub fn insert_unique(
        &mut self,
        hash: u64,
        value: T,
        hasher: impl Fn(&T) -> u64,
    ) -> OccupiedEntry<'_, T, A> {
        let bucket = self.raw.insert(hash, value, hasher);
        OccupiedEntry {
            bucket,
            table: self,
        }
    }
    pub fn clear(&mut self) {
        self.raw.clear();
    }
    pub fn shrink_to_fit(&mut self, hasher: impl Fn(&T) -> u64) {
        self.raw.shrink_to(self.len(), hasher);
    }
    pub fn shrink_to(&mut self, min_capacity: usize, hasher: impl Fn(&T) -> u64) {
        self.raw.shrink_to(min_capacity, hasher);
    }
    pub fn reserve(&mut self, additional: usize, hasher: impl Fn(&T) -> u64) {
        self.raw.reserve(additional, hasher);
    }
    pub fn try_reserve(
        &mut self,
        additional: usize,
        hasher: impl Fn(&T) -> u64,
    ) -> Result<(), TryReserveError> {
        self.raw.try_reserve(additional, hasher)
    }
    pub fn num_buckets(&self) -> usize {
        self.raw.num_buckets()
    }
    pub fn capacity(&self) -> usize {
        self.raw.capacity()
    }
    pub fn len(&self) -> usize {
        self.raw.len()
    }
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            inner: unsafe { self.raw.iter() },
            marker: PhantomData,
        }
    }
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            inner: unsafe { self.raw.iter() },
            marker: PhantomData,
        }
    }

    pub fn iter_hash(&self, hash: u64) -> IterHash<'_, T> {
        IterHash {
            inner: unsafe { self.raw.iter_hash(hash) },
            marker: PhantomData,
        }
    }

    pub fn iter_hash_buckets(&self, hash: u64) -> IterHashBuckets<'_, T> {
        IterHashBuckets {
            inner: unsafe { self.raw.iter_hash_buckets(hash) },
            marker: PhantomData,
        }
    }
    pub fn retain(&mut self, mut f: impl FnMut(&mut T) -> bool) {
        unsafe {
            for item in self.raw.iter() {
                if !f(item.as_mut()) {
                    self.raw.erase(item);
                }
            }
        }
    }
    pub fn drain(&mut self) -> Drain<'_, T, A> {
        Drain {
            inner: self.raw.drain(),
        }
    }
    pub fn extract_if<F>(&mut self, f: F) -> ExtractIf<'_, T, F, A>
    where
        F: FnMut(&mut T) -> bool,
    {
        ExtractIf {
            f,
            inner: RawExtractIf {
                iter: unsafe { self.raw.iter() },
                table: &mut self.raw,
            },
        }
    }
    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        hashes: [u64; N],
        eq: impl FnMut(usize, &T) -> bool,
    ) -> [Option<&'_ mut T>; N] {
        self.raw.get_disjoint_mut(hashes, eq)
    }
    pub unsafe fn get_disjoint_unchecked_mut<const N: usize>(
        &mut self,
        hashes: [u64; N],
        eq: impl FnMut(usize, &T) -> bool,
    ) -> [Option<&'_ mut T>; N] {
        unsafe { self.raw.get_disjoint_unchecked_mut(hashes, eq) }
    }
    #[inline]
    pub fn allocation_size(&self) -> usize {
        self.raw.allocation_size()
    }
}
impl<T, A> IntoIterator for HashTable<T, A>
where
    A: Allocator,
{
    type Item = T;
    type IntoIter = IntoIter<T, A>;
    fn into_iter(self) -> IntoIter<T, A> {
        IntoIter {
            inner: self.raw.into_iter(),
        }
    }
}
impl<'a, T, A> IntoIterator for &'a HashTable<T, A>
where
    A: Allocator,
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}
impl<'a, T, A> IntoIterator for &'a mut HashTable<T, A>
where
    A: Allocator,
{
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}
impl<T, A> Default for HashTable<T, A>
where
    A: Allocator + Default,
{
    fn default() -> Self {
        Self {
            raw: Default::default(),
        }
    }
}
impl<T, A> Clone for HashTable<T, A>
where
    T: Clone,
    A: Allocator + Clone,
{
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
        }
    }
    fn clone_from(&mut self, source: &Self) {
        self.raw.clone_from(&source.raw);
    }
}
impl<T, A> fmt::Debug for HashTable<T, A>
where
    T: fmt::Debug,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
pub enum Entry<'a, T, A = Global>
where
    A: Allocator,
{
    Occupied(OccupiedEntry<'a, T, A>),
    Vacant(VacantEntry<'a, T, A>),
}
impl<T: fmt::Debug, A: Allocator> fmt::Debug for Entry<'_, T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Entry::Vacant(ref v) => f.debug_tuple("Entry").field(v).finish(),
            Entry::Occupied(ref o) => f.debug_tuple("Entry").field(o).finish(),
        }
    }
}
impl<'a, T, A> Entry<'a, T, A>
where
    A: Allocator,
{
    pub fn insert(self, value: T) -> OccupiedEntry<'a, T, A> {
        match self {
            Entry::Occupied(mut entry) => {
                *entry.get_mut() = value;
                entry
            }
            Entry::Vacant(entry) => entry.insert(value),
        }
    }
    pub fn or_insert(self, default: T) -> OccupiedEntry<'a, T, A> {
        match self {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(entry) => entry.insert(default),
        }
    }
    pub fn or_insert_with(self, default: impl FnOnce() -> T) -> OccupiedEntry<'a, T, A> {
        match self {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }
    pub fn and_modify(self, f: impl FnOnce(&mut T)) -> Self {
        match self {
            Entry::Occupied(mut entry) => {
                f(entry.get_mut());
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }
    pub fn into_table(self) -> &'a mut HashTable<T, A> {
        match self {
            Entry::Occupied(entry) => entry.table,
            Entry::Vacant(entry) => entry.table,
        }
    }
}
pub struct OccupiedEntry<'a, T, A = Global>
where
    A: Allocator,
{
    bucket: Bucket<T>,
    table: &'a mut HashTable<T, A>,
}
unsafe impl<T, A> Send for OccupiedEntry<'_, T, A>
where
    T: Send,
    A: Send + Allocator,
{
}
unsafe impl<T, A> Sync for OccupiedEntry<'_, T, A>
where
    T: Sync,
    A: Sync + Allocator,
{
}
impl<T: fmt::Debug, A: Allocator> fmt::Debug for OccupiedEntry<'_, T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("value", self.get())
            .finish()
    }
}
impl<'a, T, A> OccupiedEntry<'a, T, A>
where
    A: Allocator,
{
    pub fn remove(self) -> (T, VacantEntry<'a, T, A>) {
        let (val, index, tag) = unsafe { self.table.raw.remove_tagged(self.bucket) };
        (
            val,
            VacantEntry {
                tag,
                index,
                table: self.table,
            },
        )
    }
    #[inline]
    pub fn get(&self) -> &T {
        unsafe { self.bucket.as_ref() }
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { self.bucket.as_mut() }
    }
    pub fn into_mut(self) -> &'a mut T {
        unsafe { self.bucket.as_mut() }
    }
    pub fn into_table(self) -> &'a mut HashTable<T, A> {
        self.table
    }
    pub fn bucket_index(&self) -> usize {
        unsafe { self.table.raw.bucket_index(&self.bucket) }
    }
}
pub struct VacantEntry<'a, T, A = Global>
where
    A: Allocator,
{
    tag: Tag,
    index: usize,
    table: &'a mut HashTable<T, A>,
}
impl<T: fmt::Debug, A: Allocator> fmt::Debug for VacantEntry<'_, T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VacantEntry")
    }
}
impl<'a, T, A> VacantEntry<'a, T, A>
where
    A: Allocator,
{
    #[inline]
    pub fn insert(self, value: T) -> OccupiedEntry<'a, T, A> {
        let bucket = unsafe {
            self.table
                .raw
                .insert_tagged_at_index(self.tag, self.index, value)
        };
        OccupiedEntry {
            bucket,
            table: self.table,
        }
    }
    pub fn into_table(self) -> &'a mut HashTable<T, A> {
        self.table
    }
}
pub struct AbsentEntry<'a, T, A = Global>
where
    A: Allocator,
{
    table: &'a mut HashTable<T, A>,
}
impl<T: fmt::Debug, A: Allocator> fmt::Debug for AbsentEntry<'_, T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AbsentEntry")
    }
}
impl<'a, T, A> AbsentEntry<'a, T, A>
where
    A: Allocator,
{
    pub fn into_table(self) -> &'a mut HashTable<T, A> {
        self.table
    }
}
pub struct Iter<'a, T> {
    inner: RawIter<T>,
    marker: PhantomData<&'a T>,
}
impl<T> Default for Iter<'_, T> {
    fn default() -> Self {
        Iter {
            inner: Default::default(),
            marker: PhantomData,
        }
    }
}
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(bucket) => Some(unsafe { bucket.as_ref() }),
            None => None,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner
            .fold(init, |acc, bucket| unsafe { f(acc, bucket.as_ref()) })
    }
}
impl<T> ExactSizeIterator for Iter<'_, T> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<T> FusedIterator for Iter<'_, T> {}
impl<'a, T> Clone for Iter<'a, T> {
    fn clone(&self) -> Iter<'a, T> {
        Iter {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}
impl<T: fmt::Debug> fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
pub struct IterMut<'a, T> {
    inner: RawIter<T>,
    marker: PhantomData<&'a mut T>,
}
impl<'a, T> IterMut<'a, T> {
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}
impl<T> Default for IterMut<'_, T> {
    fn default() -> Self {
        IterMut {
            inner: Default::default(),
            marker: PhantomData,
        }
    }
}
impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        #[cfg(test)]
        crate::hashbrown::tests::record_mut_next();
        match self.inner.next() {
            Some(bucket) => Some(unsafe { bucket.as_mut() }),
            None => None,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        #[cfg(test)]
        crate::hashbrown::tests::record_mut_fold();
        self.inner
            .fold(init, |acc, bucket| unsafe { f(acc, bucket.as_mut()) })
    }
}
impl<T> ExactSizeIterator for IterMut<'_, T> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<T> FusedIterator for IterMut<'_, T> {}
impl<T> fmt::Debug for IterMut<'_, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
pub struct UnsafeIter<'a, T> {
    inner: RawIter<T>,
    marker: PhantomData<&'a ()>,
}
impl<'a, T> UnsafeIter<'a, T> {
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}
impl<T> Default for UnsafeIter<'_, T> {
    fn default() -> Self {
        UnsafeIter {
            inner: Default::default(),
            marker: PhantomData,
        }
    }
}
impl<'a, T> Iterator for UnsafeIter<'a, T> {
    type Item = NonNull<T>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(bucket) => Some(unsafe { NonNull::new_unchecked(bucket.as_ptr()) }),
            None => None,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, bucket| unsafe {
            f(acc, NonNull::new_unchecked(bucket.as_ptr()))
        })
    }
}
impl<T> ExactSizeIterator for UnsafeIter<'_, T> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<T> FusedIterator for UnsafeIter<'_, T> {}
impl<T> fmt::Debug for UnsafeIter<'_, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
pub struct IterBuckets<'a, T> {
    inner: FullBucketsIndices,
    marker: PhantomData<&'a T>,
}
impl<T> Clone for IterBuckets<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}
impl<T> Default for IterBuckets<'_, T> {
    #[inline]
    fn default() -> Self {
        Self {
            inner: Default::default(),
            marker: PhantomData,
        }
    }
}
impl<T> Iterator for IterBuckets<'_, T> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        self.inner.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
impl<T> ExactSizeIterator for IterBuckets<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<T> FusedIterator for IterBuckets<'_, T> {}
impl<T> fmt::Debug for IterBuckets<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
pub struct IterHash<'a, T> {
    inner: RawIterHash<T>,
    marker: PhantomData<&'a T>,
}
impl<T> Default for IterHash<'_, T> {
    fn default() -> Self {
        IterHash {
            inner: Default::default(),
            marker: PhantomData,
        }
    }
}
impl<'a, T> Iterator for IterHash<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(bucket) => Some(unsafe { bucket.as_ref() }),
            None => None,
        }
    }
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner
            .fold(init, |acc, bucket| unsafe { f(acc, bucket.as_ref()) })
    }
}
impl<T> FusedIterator for IterHash<'_, T> {}
impl<'a, T> Clone for IterHash<'a, T> {
    fn clone(&self) -> IterHash<'a, T> {
        IterHash {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}
impl<T> fmt::Debug for IterHash<'_, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
pub struct IterHashMut<'a, T> {
    inner: RawIterHash<T>,
    marker: PhantomData<&'a mut T>,
}
impl<T> Default for IterHashMut<'_, T> {
    fn default() -> Self {
        IterHashMut {
            inner: Default::default(),
            marker: PhantomData,
        }
    }
}
impl<'a, T> Iterator for IterHashMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(bucket) => Some(unsafe { bucket.as_mut() }),
            None => None,
        }
    }
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner
            .fold(init, |acc, bucket| unsafe { f(acc, bucket.as_mut()) })
    }
}
impl<T> FusedIterator for IterHashMut<'_, T> {}
impl<T> fmt::Debug for IterHashMut<'_, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(IterHash {
                inner: self.inner.clone(),
                marker: PhantomData,
            })
            .finish()
    }
}
pub struct IterHashBuckets<'a, T> {
    inner: RawIterHashIndices,
    marker: PhantomData<&'a T>,
}
impl<T> Clone for IterHashBuckets<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}
impl<T> Default for IterHashBuckets<'_, T> {
    #[inline]
    fn default() -> Self {
        Self {
            inner: Default::default(),
            marker: PhantomData,
        }
    }
}
impl<T> Iterator for IterHashBuckets<'_, T> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
impl<T> FusedIterator for IterHashBuckets<'_, T> {}
impl<T> fmt::Debug for IterHashBuckets<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
pub struct IntoIter<T, A = Global>
where
    A: Allocator,
{
    inner: RawIntoIter<T, A>,
}
impl<T, A> IntoIter<T, A>
where
    A: Allocator,
{
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            inner: self.inner.iter(),
            marker: PhantomData,
        }
    }
}
impl<T, A: Allocator> Default for IntoIter<T, A> {
    fn default() -> Self {
        IntoIter {
            inner: Default::default(),
        }
    }
}
impl<T, A> Iterator for IntoIter<T, A>
where
    A: Allocator,
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, f)
    }
}
impl<T, A> ExactSizeIterator for IntoIter<T, A>
where
    A: Allocator,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<T, A> FusedIterator for IntoIter<T, A> where A: Allocator {}
impl<T, A> fmt::Debug for IntoIter<T, A>
where
    T: fmt::Debug,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
pub struct Drain<'a, T, A: Allocator = Global> {
    inner: RawDrain<'a, T, A>,
}
impl<'a, T, A: Allocator> Drain<'a, T, A> {
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            inner: self.inner.iter(),
            marker: PhantomData,
        }
    }
}
impl<T, A: Allocator> Iterator for Drain<'_, T, A> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.inner.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, f)
    }
}
impl<T, A: Allocator> ExactSizeIterator for Drain<'_, T, A> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<T, A: Allocator> FusedIterator for Drain<'_, T, A> {}
impl<T: fmt::Debug, A: Allocator> fmt::Debug for Drain<'_, T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
#[must_use = "Iterators are lazy unless consumed"]
pub struct ExtractIf<'a, T, F, A: Allocator = Global> {
    f: F,
    inner: RawExtractIf<'a, T, A>,
}
impl<T, F, A: Allocator> Iterator for ExtractIf<'_, T, F, A>
where
    F: FnMut(&mut T) -> bool,
{
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next(|val| (self.f)(val))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.inner.iter.size_hint().1)
    }
}
impl<T, F, A: Allocator> FusedIterator for ExtractIf<'_, T, F, A> where F: FnMut(&mut T) -> bool {}
