use crate::hashbrown::TryReserveError;
use crate::hashbrown::alloc::{Allocator, Global, do_alloc};
use crate::hashbrown::control::{BitMaskIter, Group, Tag, TagSliceExt};
use crate::hashbrown::scopeguard::{ScopeGuard, guard};
use crate::hashbrown::util::{invalid_mut, likely, unlikely};
use core::array;
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::mem;
use core::ptr;
use core::ptr::NonNull;
use core::slice;
use std::alloc::{Layout, handle_alloc_error};
#[inline]
unsafe fn offset_from<T>(to: *const T, from: *const T) -> usize {
    unsafe { unsafe { to.offset_from(from) as usize } }
}
#[derive(Copy, Clone)]
enum Fallibility {
    Fallible,
    Infallible,
}
impl Fallibility {
    fn capacity_overflow(self) -> TryReserveError {
        match self {
            Fallibility::Fallible => TryReserveError::CapacityOverflow,
            Fallibility::Infallible => panic!("Hash table capacity overflow"),
        }
    }
    fn alloc_err(self, layout: Layout) -> TryReserveError {
        match self {
            Fallibility::Fallible => TryReserveError::AllocError { layout },
            Fallibility::Infallible => handle_alloc_error(layout),
        }
    }
}
trait SizedTypeProperties: Sized {
    const IS_ZERO_SIZED: bool = mem::size_of::<Self>() == 0;
    const NEEDS_DROP: bool = mem::needs_drop::<Self>();
}
impl<T> SizedTypeProperties for T {}
#[inline]
fn h1(hash: u64) -> usize {
    hash as usize
}
#[derive(Clone)]
struct ProbeSeq {
    pos: usize,
    stride: usize,
}
impl ProbeSeq {
    #[inline]
    fn move_next(&mut self, bucket_mask: usize) {
        debug_assert!(
            self.stride <= bucket_mask,
            "Went past end of probe sequence"
        );
        self.stride += Group::WIDTH;
        self.pos += self.stride;
        self.pos &= bucket_mask;
    }
}
#[cfg_attr(target_os = "emscripten", inline(never))]
fn capacity_to_buckets(cap: usize, table_layout: TableLayout) -> Option<usize> {
    debug_assert_ne!(cap, 0);
    if cap < 15 {
        let min_cap = match (Group::WIDTH, table_layout.size) {
            (16, 0..=1) => 14,
            (16, 2..=3) | (8, 0..=1) => 7,
            _ => 3,
        };
        let cap = min_cap.max(cap);
        let buckets = if cap < 4 {
            4
        } else if cap < 8 {
            8
        } else {
            16
        };
        ensure_bucket_bytes_at_least_ctrl_align(table_layout, buckets);
        return Some(buckets);
    }
    let adjusted_cap = cap.checked_mul(8)? / 7;
    let buckets = adjusted_cap.next_power_of_two();
    ensure_bucket_bytes_at_least_ctrl_align(table_layout, buckets);
    Some(buckets)
}
#[inline]
fn ensure_bucket_bytes_at_least_ctrl_align(table_layout: TableLayout, buckets: usize) {
    if table_layout.size != 0 {
        let prod = table_layout.size.saturating_mul(buckets);
        debug_assert!(prod >= table_layout.ctrl_align);
    }
}
#[inline]
fn bucket_mask_to_capacity(bucket_mask: usize) -> usize {
    if bucket_mask < 8 {
        bucket_mask
    } else {
        ((bucket_mask + 1) / 8) * 7
    }
}
#[derive(Copy, Clone)]
struct TableLayout {
    size: usize,
    ctrl_align: usize,
}
impl TableLayout {
    #[inline]
    const fn new<T>() -> Self {
        let layout = Layout::new::<T>();
        Self {
            size: layout.size(),
            ctrl_align: if layout.align() > Group::WIDTH {
                layout.align()
            } else {
                Group::WIDTH
            },
        }
    }
    #[inline]
    fn calculate_layout_for(self, buckets: usize) -> Option<(Layout, usize)> {
        debug_assert!(buckets.is_power_of_two());
        let TableLayout { size, ctrl_align } = self;
        let ctrl_offset =
            size.checked_mul(buckets)?.checked_add(ctrl_align - 1)? & !(ctrl_align - 1);
        let len = ctrl_offset.checked_add(buckets + Group::WIDTH)?;
        if len > isize::MAX as usize - (ctrl_align - 1) {
            return None;
        }
        Some((
            unsafe { Layout::from_size_align_unchecked(len, ctrl_align) },
            ctrl_offset,
        ))
    }
}
pub(crate) struct Bucket<T> {
    ptr: NonNull<T>,
}
unsafe impl<T> Send for Bucket<T> {}
impl<T> Clone for Bucket<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self { ptr: self.ptr }
    }
}
impl<T> Bucket<T> {
    #[inline]
    unsafe fn from_base_index(base: NonNull<T>, index: usize) -> Self {
        let ptr = if T::IS_ZERO_SIZED {
            invalid_mut(index + 1)
        } else {
            unsafe { base.as_ptr().sub(index) }
        };
        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        }
    }
    #[inline]
    unsafe fn to_base_index(&self, base: NonNull<T>) -> usize {
        if T::IS_ZERO_SIZED {
            self.ptr.as_ptr() as usize - 1
        } else {
            unsafe { offset_from(base.as_ptr(), self.ptr.as_ptr()) }
        }
    }
    #[inline]
    pub(crate) fn as_ptr(&self) -> *mut T {
        if T::IS_ZERO_SIZED {
            invalid_mut(mem::align_of::<T>())
        } else {
            unsafe { self.ptr.as_ptr().sub(1) }
        }
    }
    #[inline]
    fn as_non_null(&self) -> NonNull<T> {
        unsafe { NonNull::new_unchecked(self.as_ptr()) }
    }
    #[inline]
    unsafe fn next_n(&self, offset: usize) -> Self {
        let ptr = if T::IS_ZERO_SIZED {
            invalid_mut(self.ptr.as_ptr() as usize + offset)
        } else {
            unsafe { self.ptr.as_ptr().sub(offset) }
        };
        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        }
    }
    pub(crate) unsafe fn drop(&self) {
        unsafe {
            self.as_ptr().drop_in_place();
        }
    }
    #[inline]
    pub(crate) unsafe fn read(&self) -> T {
        unsafe { self.as_ptr().read() }
    }
    #[inline]
    pub(crate) unsafe fn write(&self, val: T) {
        unsafe {
            self.as_ptr().write(val);
        }
    }
    #[inline]
    pub(crate) unsafe fn as_ref<'a>(&self) -> &'a T {
        unsafe { &*self.as_ptr() }
    }
    #[inline]
    pub(crate) unsafe fn as_mut<'a>(&self) -> &'a mut T {
        unsafe { &mut *self.as_ptr() }
    }
}
pub(crate) struct RawTable<T, A: Allocator = Global> {
    table: RawTableInner,
    alloc: A,
    marker: PhantomData<T>,
}
struct RawTableInner {
    bucket_mask: usize,
    ctrl: NonNull<u8>,
    growth_left: usize,
    items: usize,
}
impl<T> RawTable<T, Global> {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            table: RawTableInner::NEW,
            alloc: Global,
            marker: PhantomData,
        }
    }
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_in(capacity, Global)
    }
}
impl<T, A: Allocator> RawTable<T, A> {
    const TABLE_LAYOUT: TableLayout = TableLayout::new::<T>();
    #[inline]
    pub(crate) const fn new_in(alloc: A) -> Self {
        Self {
            table: RawTableInner::NEW,
            alloc,
            marker: PhantomData,
        }
    }
    unsafe fn new_uninitialized(
        alloc: A,
        buckets: usize,
        fallibility: Fallibility,
    ) -> Result<Self, TryReserveError> {
        debug_assert!(buckets.is_power_of_two());
        Ok(Self {
            table: unsafe {
                RawTableInner::new_uninitialized(&alloc, Self::TABLE_LAYOUT, buckets, fallibility)
            }?,
            alloc,
            marker: PhantomData,
        })
    }
    pub(crate) fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self {
            table: RawTableInner::with_capacity(&alloc, Self::TABLE_LAYOUT, capacity),
            alloc,
            marker: PhantomData,
        }
    }
    #[inline]
    pub(crate) fn allocator(&self) -> &A {
        &self.alloc
    }
    #[inline]
    pub(crate) fn data_end(&self) -> NonNull<T> {
        self.table.ctrl.cast()
    }
    #[inline]
    pub(crate) fn allocation_size(&self) -> usize {
        unsafe { self.table.allocation_size_or_zero(Self::TABLE_LAYOUT) }
    }
    #[inline]
    pub(crate) unsafe fn bucket_index(&self, bucket: &Bucket<T>) -> usize {
        unsafe { bucket.to_base_index(self.data_end()) }
    }
    #[inline]
    pub(crate) unsafe fn bucket(&self, index: usize) -> Bucket<T> {
        debug_assert_ne!(self.table.bucket_mask, 0);
        debug_assert!(index < self.num_buckets());
        unsafe { Bucket::from_base_index(self.data_end(), index) }
    }
    unsafe fn erase_no_drop(&mut self, item: &Bucket<T>) {
        unsafe {
            let index = self.bucket_index(item);
            self.table.erase(index);
        }
    }
    pub(crate) unsafe fn erase(&mut self, item: Bucket<T>) {
        unsafe {
            self.erase_no_drop(&item);
            item.drop();
        }
    }
    pub(crate) unsafe fn remove(&mut self, item: Bucket<T>) -> (T, usize) {
        unsafe {
            self.erase_no_drop(&item);
            (item.read(), self.bucket_index(&item))
        }
    }
    pub(crate) unsafe fn remove_tagged(&mut self, item: Bucket<T>) -> (T, usize, Tag) {
        unsafe {
            let index = self.bucket_index(&item);
            let tag = *self.table.ctrl(index);
            self.table.erase(index);
            (item.read(), index, tag)
        }
    }
    pub(crate) fn remove_entry(&mut self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<T> {
        match self.find(hash, eq) {
            Some(bucket) => Some(unsafe { self.remove(bucket).0 }),
            None => None,
        }
    }
    pub(crate) fn clear_no_drop(&mut self) {
        self.table.clear_no_drop();
    }
    pub(crate) fn clear(&mut self) {
        if self.is_empty() {
            return;
        }
        let mut self_ = guard(self, |self_| self_.clear_no_drop());
        unsafe {
            self_.table.drop_elements::<T>();
        }
    }
    pub(crate) fn shrink_to(&mut self, min_size: usize, hasher: impl Fn(&T) -> u64) {
        let min_size = usize::max(self.table.items, min_size);
        if min_size == 0 {
            let mut old_inner = mem::replace(&mut self.table, RawTableInner::NEW);
            unsafe {
                old_inner.drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
            }
            return;
        }
        let Some(min_buckets) = capacity_to_buckets(min_size, Self::TABLE_LAYOUT) else {
            return;
        };
        if min_buckets < self.num_buckets() {
            if self.table.items == 0 {
                let new_inner =
                    RawTableInner::with_capacity(&self.alloc, Self::TABLE_LAYOUT, min_size);
                let mut old_inner = mem::replace(&mut self.table, new_inner);
                unsafe {
                    old_inner.drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
                }
            } else {
                let result = unsafe { self.resize(min_size, hasher, Fallibility::Infallible) };
                unsafe { result.unwrap_unchecked() };
            }
        }
    }
    pub(crate) fn reserve(&mut self, additional: usize, hasher: impl Fn(&T) -> u64) {
        if unlikely(additional > self.table.growth_left) {
            let result =
                unsafe { self.reserve_rehash(additional, hasher, Fallibility::Infallible) };
            unsafe { result.unwrap_unchecked() };
        }
    }
    pub(crate) fn try_reserve(
        &mut self,
        additional: usize,
        hasher: impl Fn(&T) -> u64,
    ) -> Result<(), TryReserveError> {
        if additional > self.table.growth_left {
            unsafe { self.reserve_rehash(additional, hasher, Fallibility::Fallible) }
        } else {
            Ok(())
        }
    }
    #[cold]
    #[inline(never)]
    unsafe fn reserve_rehash(
        &mut self,
        additional: usize,
        hasher: impl Fn(&T) -> u64,
        fallibility: Fallibility,
    ) -> Result<(), TryReserveError> {
        unsafe {
            self.table.reserve_rehash_inner(
                &self.alloc,
                additional,
                &|table, index| hasher(table.bucket::<T>(index).as_ref()),
                fallibility,
                Self::TABLE_LAYOUT,
                if T::NEEDS_DROP {
                    Some(|ptr| ptr::drop_in_place(ptr.cast::<T>()))
                } else {
                    None
                },
            )
        }
    }
    unsafe fn resize(
        &mut self,
        capacity: usize,
        hasher: impl Fn(&T) -> u64,
        fallibility: Fallibility,
    ) -> Result<(), TryReserveError> {
        unsafe {
            self.table.resize_inner(
                &self.alloc,
                capacity,
                &|table, index| hasher(table.bucket::<T>(index).as_ref()),
                fallibility,
                Self::TABLE_LAYOUT,
            )
        }
    }
    pub(crate) fn insert(&mut self, hash: u64, value: T, hasher: impl Fn(&T) -> u64) -> Bucket<T> {
        unsafe {
            let mut index = self.table.find_insert_index(hash);
            let old_ctrl = *self.table.ctrl(index);
            if unlikely(self.table.growth_left == 0 && old_ctrl.special_is_empty()) {
                self.reserve(1, hasher);
                index = self.table.find_insert_index(hash);
            }
            self.insert_at_index(hash, index, value)
        }
    }
    pub(crate) fn insert_entry(
        &mut self,
        hash: u64,
        value: T,
        hasher: impl Fn(&T) -> u64,
    ) -> &mut T {
        unsafe { self.insert(hash, value, hasher).as_mut() }
    }
    pub(crate) unsafe fn replace_bucket_with<F>(&mut self, bucket: Bucket<T>, f: F) -> Option<Tag>
    where
        F: FnOnce(T) -> Option<T>,
    {
        unsafe {
            let index = self.bucket_index(&bucket);
            let old_ctrl = *self.table.ctrl(index);
            debug_assert!(self.is_bucket_full(index));
            let old_growth_left = self.table.growth_left;
            let item = self.remove(bucket).0;
            if let Some(new_item) = f(item) {
                self.table.growth_left = old_growth_left;
                self.table.set_ctrl(index, old_ctrl);
                self.table.items += 1;
                self.bucket(index).write(new_item);
                None
            } else {
                Some(old_ctrl)
            }
        }
    }
    #[inline]
    pub(crate) fn find_or_find_insert_index(
        &mut self,
        hash: u64,
        mut eq: impl FnMut(&T) -> bool,
        hasher: impl Fn(&T) -> u64,
    ) -> Result<Bucket<T>, usize> {
        self.reserve(1, hasher);
        unsafe {
            match self
                .table
                .find_or_find_insert_index_inner(hash, &mut |index| eq(self.bucket(index).as_ref()))
            {
                Ok(index) => Ok(self.bucket(index)),
                Err(index) => Err(index),
            }
        }
    }
    #[inline]
    pub(crate) unsafe fn insert_at_index(
        &mut self,
        hash: u64,
        index: usize,
        value: T,
    ) -> Bucket<T> {
        unsafe { self.insert_tagged_at_index(Tag::full(hash), index, value) }
    }
    #[inline]
    pub(crate) unsafe fn insert_tagged_at_index(
        &mut self,
        tag: Tag,
        index: usize,
        value: T,
    ) -> Bucket<T> {
        unsafe {
            let old_ctrl = *self.table.ctrl(index);
            self.table.record_item_insert_at(index, old_ctrl, tag);
            let bucket = self.bucket(index);
            bucket.write(value);
            bucket
        }
    }
    #[inline]
    pub(crate) fn find(&self, hash: u64, mut eq: impl FnMut(&T) -> bool) -> Option<Bucket<T>> {
        unsafe {
            let result = self
                .table
                .find_inner(hash, &mut |index| eq(self.bucket(index).as_ref()));
            match result {
                Some(index) => Some(self.bucket(index)),
                None => None,
            }
        }
    }
    #[inline]
    pub(crate) fn get(&self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<&T> {
        match self.find(hash, eq) {
            Some(bucket) => Some(unsafe { bucket.as_ref() }),
            None => None,
        }
    }
    #[inline]
    pub(crate) fn get_mut(&mut self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<&mut T> {
        match self.find(hash, eq) {
            Some(bucket) => Some(unsafe { bucket.as_mut() }),
            None => None,
        }
    }
    #[inline]
    pub(crate) fn get_bucket(&self, index: usize) -> Option<&T> {
        unsafe {
            if index < self.num_buckets() && self.is_bucket_full(index) {
                Some(self.bucket(index).as_ref())
            } else {
                None
            }
        }
    }
    #[inline]
    pub(crate) fn get_bucket_mut(&mut self, index: usize) -> Option<&mut T> {
        unsafe {
            if index < self.num_buckets() && self.is_bucket_full(index) {
                Some(self.bucket(index).as_mut())
            } else {
                None
            }
        }
    }
    #[inline]
    pub(crate) fn checked_bucket(&self, index: usize) -> Option<Bucket<T>> {
        unsafe {
            if index < self.num_buckets() && self.is_bucket_full(index) {
                Some(self.bucket(index))
            } else {
                None
            }
        }
    }
    pub(crate) fn get_disjoint_mut<const N: usize>(
        &mut self,
        hashes: [u64; N],
        eq: impl FnMut(usize, &T) -> bool,
    ) -> [Option<&'_ mut T>; N] {
        unsafe {
            let ptrs = self.get_disjoint_mut_pointers(hashes, eq);
            for (i, cur) in ptrs.iter().enumerate() {
                if cur.is_some() && ptrs[..i].contains(cur) {
                    panic!("duplicate keys found");
                }
            }
            ptrs.map(|ptr| ptr.map(|mut ptr| ptr.as_mut()))
        }
    }
    pub(crate) unsafe fn get_disjoint_unchecked_mut<const N: usize>(
        &mut self,
        hashes: [u64; N],
        eq: impl FnMut(usize, &T) -> bool,
    ) -> [Option<&'_ mut T>; N] {
        let ptrs = unsafe { self.get_disjoint_mut_pointers(hashes, eq) };
        ptrs.map(|ptr| ptr.map(|mut ptr| unsafe { ptr.as_mut() }))
    }
    unsafe fn get_disjoint_mut_pointers<const N: usize>(
        &mut self,
        hashes: [u64; N],
        mut eq: impl FnMut(usize, &T) -> bool,
    ) -> [Option<NonNull<T>>; N] {
        array::from_fn(|i| {
            self.find(hashes[i], |k| eq(i, k))
                .map(|cur| cur.as_non_null())
        })
    }
    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        self.table.items + self.table.growth_left
    }
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.table.items
    }
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub(crate) fn num_buckets(&self) -> usize {
        self.table.bucket_mask + 1
    }
    #[inline]
    pub(crate) unsafe fn is_bucket_full(&self, index: usize) -> bool {
        unsafe { self.table.is_bucket_full(index) }
    }
    #[inline]
    pub(crate) unsafe fn iter(&self) -> RawIter<T> {
        unsafe { self.table.iter() }
    }
    pub(crate) unsafe fn iter_hash(&self, hash: u64) -> RawIterHash<T> {
        unsafe { RawIterHash::new(self, hash) }
    }
    pub(crate) unsafe fn iter_hash_buckets(&self, hash: u64) -> RawIterHashIndices {
        unsafe { RawIterHashIndices::new(&self.table, hash) }
    }
    #[inline(always)]
    pub(crate) unsafe fn full_buckets_indices(&self) -> FullBucketsIndices {
        unsafe { self.table.full_buckets_indices() }
    }
    pub(crate) fn drain(&mut self) -> RawDrain<'_, T, A> {
        unsafe {
            let iter = self.iter();
            self.drain_iter_from(iter)
        }
    }
    pub(crate) unsafe fn drain_iter_from(&mut self, iter: RawIter<T>) -> RawDrain<'_, T, A> {
        debug_assert_eq!(iter.len(), self.len());
        RawDrain {
            iter,
            table: mem::replace(&mut self.table, RawTableInner::NEW),
            orig_table: NonNull::from(&mut self.table),
            marker: PhantomData,
        }
    }
    pub(crate) unsafe fn into_iter_from(self, iter: RawIter<T>) -> RawIntoIter<T, A> {
        debug_assert_eq!(iter.len(), self.len());
        let allocation = self.into_allocation();
        RawIntoIter {
            iter,
            allocation,
            marker: PhantomData,
        }
    }
    pub(crate) fn into_allocation(self) -> Option<(NonNull<u8>, Layout, A)> {
        let alloc = if self.table.is_empty_singleton() {
            None
        } else {
            let (layout, ctrl_offset) = {
                let option = Self::TABLE_LAYOUT.calculate_layout_for(self.table.num_buckets());
                unsafe { option.unwrap_unchecked() }
            };
            Some((
                unsafe { NonNull::new_unchecked(self.table.ctrl.as_ptr().sub(ctrl_offset).cast()) },
                layout,
                unsafe { ptr::read(&raw const self.alloc) },
            ))
        };
        mem::forget(self);
        alloc
    }
}
unsafe impl<T, A: Allocator> Send for RawTable<T, A>
where
    T: Send,
    A: Send,
{
}
unsafe impl<T, A: Allocator> Sync for RawTable<T, A>
where
    T: Sync,
    A: Sync,
{
}
impl RawTableInner {
    const NEW: Self = RawTableInner::new();
    #[inline]
    const fn new() -> Self {
        Self {
            ctrl: unsafe {
                NonNull::new_unchecked(Group::static_empty().as_ptr().cast_mut().cast())
            },
            bucket_mask: 0,
            items: 0,
            growth_left: 0,
        }
    }
}
pub(crate) fn prev_pow2(z: usize) -> usize {
    let shift = mem::size_of::<usize>() * 8 - 1;
    1 << (shift - (z.leading_zeros() as usize))
}
fn maximum_buckets_in(
    allocation_size: usize,
    table_layout: TableLayout,
    group_width: usize,
) -> usize {
    let x = (allocation_size - group_width) / (table_layout.size + 1);
    prev_pow2(x)
}
impl RawTableInner {
    unsafe fn new_uninitialized<A>(
        alloc: &A,
        table_layout: TableLayout,
        mut buckets: usize,
        fallibility: Fallibility,
    ) -> Result<Self, TryReserveError>
    where
        A: Allocator,
    {
        debug_assert!(buckets.is_power_of_two());
        let Some((layout, mut ctrl_offset)) = table_layout.calculate_layout_for(buckets) else {
            return Err(fallibility.capacity_overflow());
        };
        let ptr: NonNull<u8> = match do_alloc(alloc, layout) {
            Ok(block) => {
                if block.len() != layout.size() {
                    let x = maximum_buckets_in(block.len(), table_layout, Group::WIDTH);
                    debug_assert!(x >= buckets);
                    let (oversized_layout, oversized_ctrl_offset) = {
                        let option = table_layout.calculate_layout_for(x);
                        unsafe { option.unwrap_unchecked() }
                    };
                    debug_assert!(oversized_layout.size() <= block.len());
                    debug_assert!(oversized_ctrl_offset >= ctrl_offset);
                    ctrl_offset = oversized_ctrl_offset;
                    buckets = x;
                }
                block.cast()
            }
            Err(_) => return Err(fallibility.alloc_err(layout)),
        };
        let ctrl = unsafe { NonNull::new_unchecked(ptr.as_ptr().add(ctrl_offset)) };
        Ok(Self {
            ctrl,
            bucket_mask: buckets - 1,
            items: 0,
            growth_left: bucket_mask_to_capacity(buckets - 1),
        })
    }
    #[inline]
    fn fallible_with_capacity<A>(
        alloc: &A,
        table_layout: TableLayout,
        capacity: usize,
        fallibility: Fallibility,
    ) -> Result<Self, TryReserveError>
    where
        A: Allocator,
    {
        if capacity == 0 {
            Ok(Self::NEW)
        } else {
            unsafe {
                let buckets = capacity_to_buckets(capacity, table_layout)
                    .ok_or_else(|| fallibility.capacity_overflow())?;
                let mut result =
                    Self::new_uninitialized(alloc, table_layout, buckets, fallibility)?;
                result.ctrl_slice().fill_empty();
                Ok(result)
            }
        }
    }
    fn with_capacity<A>(alloc: &A, table_layout: TableLayout, capacity: usize) -> Self
    where
        A: Allocator,
    {
        let result =
            Self::fallible_with_capacity(alloc, table_layout, capacity, Fallibility::Infallible);
        unsafe { result.unwrap_unchecked() }
    }
    #[inline]
    unsafe fn fix_insert_index(&self, mut index: usize) -> usize {
        if unlikely(unsafe { self.is_bucket_full(index) }) {
            debug_assert!(self.bucket_mask < Group::WIDTH);
            index = unsafe {
                Group::load_aligned(self.ctrl(0))
                    .match_empty_or_deleted()
                    .lowest_set_bit()
                    .unwrap_unchecked()
            };
        }
        index
    }
    #[inline]
    fn find_insert_index_in_group(&self, group: &Group, probe_seq: &ProbeSeq) -> Option<usize> {
        let bit = group.match_empty_or_deleted().lowest_set_bit();
        if likely(bit.is_some()) {
            Some((probe_seq.pos + bit.unwrap()) & self.bucket_mask)
        } else {
            None
        }
    }
    #[inline]
    unsafe fn find_or_find_insert_index_inner(
        &self,
        hash: u64,
        eq: &mut dyn FnMut(usize) -> bool,
    ) -> Result<usize, usize> {
        let mut insert_index = None;
        let tag_hash = Tag::full(hash);
        let mut probe_seq = self.probe_seq(hash);
        loop {
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };
            for bit in group.match_tag(tag_hash) {
                let index = (probe_seq.pos + bit) & self.bucket_mask;
                if likely(eq(index)) {
                    return Ok(index);
                }
            }
            if likely(insert_index.is_none()) {
                insert_index = self.find_insert_index_in_group(&group, &probe_seq);
            }
            if let Some(insert_index) = insert_index {
                if likely(group.match_empty().any_bit_set()) {
                    unsafe {
                        return Err(self.fix_insert_index(insert_index));
                    }
                }
            }
            probe_seq.move_next(self.bucket_mask);
        }
    }
    #[inline]
    unsafe fn prepare_insert_index(&mut self, hash: u64) -> (usize, Tag) {
        unsafe {
            let index: usize = self.find_insert_index(hash);
            let old_ctrl = *self.ctrl(index);
            self.set_ctrl_hash(index, hash);
            (index, old_ctrl)
        }
    }
    #[inline]
    unsafe fn find_insert_index(&self, hash: u64) -> usize {
        let mut probe_seq = self.probe_seq(hash);
        loop {
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };
            let index = self.find_insert_index_in_group(&group, &probe_seq);
            if likely(index.is_some()) {
                unsafe {
                    return self.fix_insert_index(index.unwrap_unchecked());
                }
            }
            probe_seq.move_next(self.bucket_mask);
        }
    }
    #[inline(always)]
    unsafe fn find_inner(&self, hash: u64, eq: &mut dyn FnMut(usize) -> bool) -> Option<usize> {
        let tag_hash = Tag::full(hash);
        let mut probe_seq = self.probe_seq(hash);
        loop {
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };
            for bit in group.match_tag(tag_hash) {
                let index = (probe_seq.pos + bit) & self.bucket_mask;
                if likely(eq(index)) {
                    return Some(index);
                }
            }
            if likely(group.match_empty().any_bit_set()) {
                return None;
            }
            probe_seq.move_next(self.bucket_mask);
        }
    }
    #[inline]
    unsafe fn prepare_rehash_in_place(&mut self) {
        unsafe {
            for i in (0..self.num_buckets()).step_by(Group::WIDTH) {
                let group = Group::load_aligned(self.ctrl(i));
                let group = group.convert_special_to_empty_and_full_to_deleted();
                group.store_aligned(self.ctrl(i));
            }
        }
        if unlikely(self.num_buckets() < Group::WIDTH) {
            unsafe {
                self.ctrl(0)
                    .copy_to(self.ctrl(Group::WIDTH), self.num_buckets());
            }
        } else {
            unsafe {
                self.ctrl(0)
                    .copy_to(self.ctrl(self.num_buckets()), Group::WIDTH);
            }
        }
    }
    #[inline]
    unsafe fn iter<T>(&self) -> RawIter<T> {
        unsafe {
            let data = Bucket::from_base_index(self.data_end(), 0);
            RawIter {
                iter: RawIterRange::new(self.ctrl.as_ptr(), data, self.num_buckets()),
                items: self.items,
            }
        }
    }
    unsafe fn drop_elements<T>(&mut self) {
        if T::NEEDS_DROP && self.items != 0 {
            unsafe {
                for item in self.iter::<T>() {
                    item.drop();
                }
            }
        }
    }
    unsafe fn drop_inner_table<T, A: Allocator>(&mut self, alloc: &A, table_layout: TableLayout) {
        if !self.is_empty_singleton() {
            unsafe {
                self.drop_elements::<T>();
            }
            unsafe {
                self.free_buckets(alloc, table_layout);
            }
        }
    }
    #[inline]
    unsafe fn bucket<T>(&self, index: usize) -> Bucket<T> {
        debug_assert_ne!(self.bucket_mask, 0);
        debug_assert!(index < self.num_buckets());
        unsafe { Bucket::from_base_index(self.data_end(), index) }
    }
    #[inline]
    unsafe fn bucket_ptr(&self, index: usize, size_of: usize) -> *mut u8 {
        debug_assert_ne!(self.bucket_mask, 0);
        debug_assert!(index < self.num_buckets());
        unsafe {
            let base: *mut u8 = self.data_end().as_ptr();
            base.sub((index + 1) * size_of)
        }
    }
    #[inline]
    fn data_end<T>(&self) -> NonNull<T> {
        self.ctrl.cast()
    }
    #[inline]
    fn probe_seq(&self, hash: u64) -> ProbeSeq {
        ProbeSeq {
            pos: h1(hash) & self.bucket_mask,
            stride: 0,
        }
    }
    #[inline]
    unsafe fn record_item_insert_at(&mut self, index: usize, old_ctrl: Tag, new_ctrl: Tag) {
        self.growth_left -= usize::from(old_ctrl.special_is_empty());
        unsafe {
            self.set_ctrl(index, new_ctrl);
        }
        self.items += 1;
    }
    #[inline]
    fn is_in_same_group(&self, i: usize, new_i: usize, hash: u64) -> bool {
        let probe_seq_pos = self.probe_seq(hash).pos;
        let probe_index =
            |pos: usize| (pos.wrapping_sub(probe_seq_pos) & self.bucket_mask) / Group::WIDTH;
        probe_index(i) == probe_index(new_i)
    }
    #[inline]
    unsafe fn set_ctrl_hash(&mut self, index: usize, hash: u64) {
        unsafe {
            self.set_ctrl(index, Tag::full(hash));
        }
    }
    #[inline]
    unsafe fn replace_ctrl_hash(&mut self, index: usize, hash: u64) -> Tag {
        unsafe {
            let prev_ctrl = *self.ctrl(index);
            self.set_ctrl_hash(index, hash);
            prev_ctrl
        }
    }
    #[inline]
    unsafe fn set_ctrl(&mut self, index: usize, ctrl: Tag) {
        let index2 = ((index.wrapping_sub(Group::WIDTH)) & self.bucket_mask) + Group::WIDTH;
        unsafe {
            *self.ctrl(index) = ctrl;
            *self.ctrl(index2) = ctrl;
        }
    }
    #[inline]
    unsafe fn ctrl(&self, index: usize) -> *mut Tag {
        debug_assert!(index < self.num_ctrl_bytes());
        unsafe { self.ctrl.as_ptr().add(index).cast() }
    }
    fn ctrl_slice(&mut self) -> &mut [mem::MaybeUninit<Tag>] {
        unsafe { slice::from_raw_parts_mut(self.ctrl.as_ptr().cast(), self.num_ctrl_bytes()) }
    }
    #[inline]
    fn num_buckets(&self) -> usize {
        self.bucket_mask + 1
    }
    #[inline]
    unsafe fn is_bucket_full(&self, index: usize) -> bool {
        debug_assert!(index < self.num_buckets());
        unsafe { (*self.ctrl(index)).is_full() }
    }
    #[inline]
    fn num_ctrl_bytes(&self) -> usize {
        self.bucket_mask + 1 + Group::WIDTH
    }
    #[inline]
    fn is_empty_singleton(&self) -> bool {
        self.bucket_mask == 0
    }
    #[inline]
    fn prepare_resize<'a, A>(
        &self,
        alloc: &'a A,
        table_layout: TableLayout,
        capacity: usize,
        fallibility: Fallibility,
    ) -> Result<
        crate::hashbrown::scopeguard::ScopeGuard<Self, impl FnMut(&mut Self) + 'a>,
        TryReserveError,
    >
    where
        A: Allocator,
    {
        debug_assert!(self.items <= capacity);
        let new_table =
            RawTableInner::fallible_with_capacity(alloc, table_layout, capacity, fallibility)?;
        Ok(guard(new_table, move |self_| {
            if !self_.is_empty_singleton() {
                unsafe { self_.free_buckets(alloc, table_layout) };
            }
        }))
    }
    #[inline(always)]
    unsafe fn reserve_rehash_inner<A>(
        &mut self,
        alloc: &A,
        additional: usize,
        hasher: &dyn Fn(&mut Self, usize) -> u64,
        fallibility: Fallibility,
        layout: TableLayout,
        drop: Option<unsafe fn(*mut u8)>,
    ) -> Result<(), TryReserveError>
    where
        A: Allocator,
    {
        let Some(new_items) = self.items.checked_add(additional) else {
            return Err(fallibility.capacity_overflow());
        };
        let full_capacity = bucket_mask_to_capacity(self.bucket_mask);
        if new_items <= full_capacity / 2 {
            unsafe {
                self.rehash_in_place(hasher, layout.size, drop);
            }
            Ok(())
        } else {
            unsafe {
                self.resize_inner(
                    alloc,
                    usize::max(new_items, full_capacity + 1),
                    hasher,
                    fallibility,
                    layout,
                )
            }
        }
    }
    #[inline(always)]
    unsafe fn full_buckets_indices(&self) -> FullBucketsIndices {
        unsafe {
            let ctrl = NonNull::new_unchecked(self.ctrl(0).cast::<u8>());
            FullBucketsIndices {
                current_group: Group::load_aligned(ctrl.as_ptr().cast())
                    .match_full()
                    .into_iter(),
                group_first_index: 0,
                ctrl,
                items: self.items,
            }
        }
    }
    #[inline(always)]
    unsafe fn resize_inner<A>(
        &mut self,
        alloc: &A,
        capacity: usize,
        hasher: &dyn Fn(&mut Self, usize) -> u64,
        fallibility: Fallibility,
        layout: TableLayout,
    ) -> Result<(), TryReserveError>
    where
        A: Allocator,
    {
        let mut new_table = self.prepare_resize(alloc, layout, capacity, fallibility)?;
        unsafe {
            for full_byte_index in self.full_buckets_indices() {
                let hash = hasher(self, full_byte_index);
                let (new_index, _) = new_table.prepare_insert_index(hash);
                ptr::copy_nonoverlapping(
                    self.bucket_ptr(full_byte_index, layout.size),
                    new_table.bucket_ptr(new_index, layout.size),
                    layout.size,
                );
            }
        }
        new_table.growth_left -= self.items;
        new_table.items = self.items;
        mem::swap(self, &mut new_table);
        Ok(())
    }
    #[cfg_attr(any(), inline(always))]
    unsafe fn rehash_in_place(
        &mut self,
        hasher: &dyn Fn(&mut Self, usize) -> u64,
        size_of: usize,
        drop: Option<unsafe fn(*mut u8)>,
    ) {
        unsafe {
            self.prepare_rehash_in_place();
        }
        let mut guard = guard(self, move |self_| {
            for i in 0..self_.num_buckets() {
                unsafe {
                    if *self_.ctrl(i) == Tag::DELETED {
                        self_.set_ctrl(i, Tag::EMPTY);
                        if let Some(drop) = drop {
                            drop(self_.bucket_ptr(i, size_of));
                        }
                        self_.items -= 1;
                    }
                }
            }
            self_.growth_left = bucket_mask_to_capacity(self_.bucket_mask) - self_.items;
        });
        'outer: for i in 0..guard.num_buckets() {
            unsafe {
                if *guard.ctrl(i) != Tag::DELETED {
                    continue;
                }
            }
            let i_p = unsafe { guard.bucket_ptr(i, size_of) };
            loop {
                let hash = hasher(*guard, i);
                let new_i = unsafe { guard.find_insert_index(hash) };
                if likely(guard.is_in_same_group(i, new_i, hash)) {
                    unsafe { guard.set_ctrl_hash(i, hash) };
                    continue 'outer;
                }
                let new_i_p = unsafe { guard.bucket_ptr(new_i, size_of) };
                let prev_ctrl = unsafe { guard.replace_ctrl_hash(new_i, hash) };
                if prev_ctrl == Tag::EMPTY {
                    unsafe { guard.set_ctrl(i, Tag::EMPTY) };
                    unsafe {
                        ptr::copy_nonoverlapping(i_p, new_i_p, size_of);
                    }
                    continue 'outer;
                }
                debug_assert_eq!(prev_ctrl, Tag::DELETED);
                unsafe {
                    ptr::swap_nonoverlapping(i_p, new_i_p, size_of);
                }
            }
        }
        guard.growth_left = bucket_mask_to_capacity(guard.bucket_mask) - guard.items;
        mem::forget(guard);
    }
    #[inline]
    unsafe fn free_buckets<A>(&mut self, alloc: &A, table_layout: TableLayout)
    where
        A: Allocator,
    {
        unsafe {
            let (ptr, layout) = self.allocation_info(table_layout);
            alloc.deallocate(ptr, layout);
        }
    }
    #[inline]
    unsafe fn allocation_info(&self, table_layout: TableLayout) -> (NonNull<u8>, Layout) {
        debug_assert!(
            !self.is_empty_singleton(),
            "this function can only be called on non-empty tables"
        );
        let (layout, ctrl_offset) = {
            let option = table_layout.calculate_layout_for(self.num_buckets());
            unsafe { option.unwrap_unchecked() }
        };
        (
            unsafe { NonNull::new_unchecked(self.ctrl.as_ptr().sub(ctrl_offset)) },
            layout,
        )
    }
    #[inline]
    unsafe fn allocation_size_or_zero(&self, table_layout: TableLayout) -> usize {
        if self.is_empty_singleton() {
            0
        } else {
            unsafe { self.allocation_info(table_layout).1.size() }
        }
    }
    #[inline]
    fn clear_no_drop(&mut self) {
        if !self.is_empty_singleton() {
            self.ctrl_slice().fill_empty();
        }
        self.items = 0;
        self.growth_left = bucket_mask_to_capacity(self.bucket_mask);
    }
    #[inline]
    unsafe fn erase(&mut self, index: usize) {
        unsafe {
            debug_assert!(self.is_bucket_full(index));
        }
        let index_before = index.wrapping_sub(Group::WIDTH) & self.bucket_mask;
        let (empty_before, empty_after) = unsafe {
            (
                Group::load(self.ctrl(index_before)).match_empty(),
                Group::load(self.ctrl(index)).match_empty(),
            )
        };
        let ctrl = if empty_before.leading_zeros() + empty_after.trailing_zeros() >= Group::WIDTH {
            Tag::DELETED
        } else {
            self.growth_left += 1;
            Tag::EMPTY
        };
        unsafe {
            self.set_ctrl(index, ctrl);
        }
        self.items -= 1;
    }
}
impl<T: Clone, A: Allocator + Clone> Clone for RawTable<T, A> {
    fn clone(&self) -> Self {
        if self.table.is_empty_singleton() {
            Self::new_in(self.alloc.clone())
        } else {
            let result = unsafe {
                Self::new_uninitialized(
                    self.alloc.clone(),
                    self.table.num_buckets(),
                    Fallibility::Infallible,
                )
            };
            let mut new_table = unsafe { result.unwrap_unchecked() };
            unsafe { new_table.clone_from_spec(self) };
            new_table
        }
    }
    fn clone_from(&mut self, source: &Self) {
        if source.table.is_empty_singleton() {
            let mut old_inner = mem::replace(&mut self.table, RawTableInner::NEW);
            unsafe {
                old_inner.drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
            }
        } else {
            unsafe {
                let mut self_ = guard(self, |self_| {
                    self_.clear_no_drop();
                });
                self_.table.drop_elements::<T>();
                if self_.num_buckets() != source.num_buckets() {
                    let new_inner = {
                        let result = RawTableInner::new_uninitialized(
                            &self_.alloc,
                            Self::TABLE_LAYOUT,
                            source.num_buckets(),
                            Fallibility::Infallible,
                        );
                        result.unwrap_unchecked()
                    };
                    let mut old_inner = mem::replace(&mut self_.table, new_inner);
                    if !old_inner.is_empty_singleton() {
                        old_inner.free_buckets(&self_.alloc, Self::TABLE_LAYOUT);
                    }
                }
                self_.clone_from_spec(source);
                ScopeGuard::into_inner(self_);
            }
        }
    }
}
trait RawTableClone {
    unsafe fn clone_from_spec(&mut self, source: &Self);
}
impl<T: Clone, A: Allocator + Clone> RawTableClone for RawTable<T, A> {
    default_fn! { #[cfg_attr (any () , inline)] unsafe fn clone_from_spec (& mut self , source : & Self) { unsafe { self . clone_from_impl (source) ; } } }
}
impl<T: Clone, A: Allocator + Clone> RawTable<T, A> {
    unsafe fn clone_from_impl(&mut self, source: &Self) {
        unsafe {
            source
                .table
                .ctrl(0)
                .copy_to_nonoverlapping(self.table.ctrl(0), self.table.num_ctrl_bytes());
        }
        let mut guard = guard((0, &mut *self), |(index, self_)| {
            if T::NEEDS_DROP {
                for i in 0..*index {
                    unsafe {
                        if self_.is_bucket_full(i) {
                            self_.bucket(i).drop();
                        }
                    }
                }
            }
        });
        unsafe {
            for from in source.iter() {
                let index = source.bucket_index(&from);
                let to = guard.1.bucket(index);
                to.write(from.as_ref().clone());
                guard.0 = index + 1;
            }
        }
        mem::forget(guard);
        self.table.items = source.table.items;
        self.table.growth_left = source.table.growth_left;
    }
}
impl<T, A: Allocator + Default> Default for RawTable<T, A> {
    #[inline]
    fn default() -> Self {
        Self::new_in(Default::default())
    }
}
impl<T, A: Allocator> Drop for RawTable<T, A> {
    fn drop(&mut self) {
        unsafe {
            self.table
                .drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
        }
    }
}
impl<T, A: Allocator> IntoIterator for RawTable<T, A> {
    type Item = T;
    type IntoIter = RawIntoIter<T, A>;
    fn into_iter(self) -> RawIntoIter<T, A> {
        unsafe {
            let iter = self.iter();
            self.into_iter_from(iter)
        }
    }
}
pub(crate) struct RawIterRange<T> {
    current_group: BitMaskIter,
    data: Bucket<T>,
    next_ctrl: *const u8,
    end: *const u8,
}
impl<T> RawIterRange<T> {
    unsafe fn new(ctrl: *const u8, data: Bucket<T>, len: usize) -> Self {
        debug_assert_ne!(len, 0);
        debug_assert_eq!(ctrl as usize % Group::WIDTH, 0);
        let end = unsafe { ctrl.add(len) };
        let (current_group, next_ctrl) = unsafe {
            (
                Group::load_aligned(ctrl.cast()).match_full(),
                ctrl.add(Group::WIDTH),
            )
        };
        Self {
            current_group: current_group.into_iter(),
            data,
            next_ctrl,
            end,
        }
    }
    unsafe fn next_impl<const DO_CHECK_PTR_RANGE: bool>(&mut self) -> Option<Bucket<T>> {
        loop {
            if let Some(index) = self.current_group.next() {
                return Some(unsafe { self.data.next_n(index) });
            }
            if DO_CHECK_PTR_RANGE && self.next_ctrl >= self.end {
                return None;
            }
            unsafe {
                self.current_group = Group::load_aligned(self.next_ctrl.cast())
                    .match_full()
                    .into_iter();
                self.data = self.data.next_n(Group::WIDTH);
                self.next_ctrl = self.next_ctrl.add(Group::WIDTH);
            }
        }
    }
    unsafe fn fold_impl<F, B>(mut self, mut n: usize, mut acc: B, mut f: F) -> B
    where
        F: FnMut(B, Bucket<T>) -> B,
    {
        loop {
            while let Some(index) = self.current_group.next() {
                debug_assert!(n != 0);
                let bucket = unsafe { self.data.next_n(index) };
                acc = f(acc, bucket);
                n -= 1;
            }
            if n == 0 {
                return acc;
            }
            unsafe {
                self.current_group = Group::load_aligned(self.next_ctrl.cast())
                    .match_full()
                    .into_iter();
                self.data = self.data.next_n(Group::WIDTH);
                self.next_ctrl = self.next_ctrl.add(Group::WIDTH);
            }
        }
    }
}
unsafe impl<T> Send for RawIterRange<T> {}
unsafe impl<T> Sync for RawIterRange<T> {}
impl<T> Clone for RawIterRange<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            next_ctrl: self.next_ctrl,
            current_group: self.current_group.clone(),
            end: self.end,
        }
    }
}
impl<T> Iterator for RawIterRange<T> {
    type Item = Bucket<T>;
    fn next(&mut self) -> Option<Bucket<T>> {
        unsafe { self.next_impl::<true>() }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining_buckets = if self.end > self.next_ctrl {
            unsafe { offset_from(self.end, self.next_ctrl) }
        } else {
            0
        };
        (0, Some(Group::WIDTH + remaining_buckets))
    }
}
impl<T> FusedIterator for RawIterRange<T> {}
pub(crate) struct RawIter<T> {
    pub(crate) iter: RawIterRange<T>,
    items: usize,
}
impl<T> RawIter<T> {
    unsafe fn drop_elements(&mut self) {
        unsafe {
            if T::NEEDS_DROP && self.items != 0 {
                for item in self {
                    item.drop();
                }
            }
        }
    }
}
impl<T> Clone for RawIter<T> {
    fn clone(&self) -> Self {
        Self {
            iter: self.iter.clone(),
            items: self.items,
        }
    }
}
impl<T> Default for RawIter<T> {
    fn default() -> Self {
        unsafe { RawTableInner::NEW.iter() }
    }
}
impl<T> Iterator for RawIter<T> {
    type Item = Bucket<T>;
    fn next(&mut self) -> Option<Bucket<T>> {
        if self.items == 0 {
            return None;
        }
        let nxt = unsafe { self.iter.next_impl::<false>() };
        debug_assert!(nxt.is_some());
        self.items -= 1;
        nxt
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.items, Some(self.items))
    }
    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        unsafe { self.iter.fold_impl(self.items, init, f) }
    }
}
impl<T> ExactSizeIterator for RawIter<T> {}
impl<T> FusedIterator for RawIter<T> {}
#[derive(Clone)]
pub(crate) struct FullBucketsIndices {
    current_group: BitMaskIter,
    group_first_index: usize,
    ctrl: NonNull<u8>,
    items: usize,
}
impl Default for FullBucketsIndices {
    fn default() -> Self {
        unsafe { RawTableInner::NEW.full_buckets_indices() }
    }
}
impl FullBucketsIndices {
    #[inline(always)]
    unsafe fn next_impl(&mut self) -> Option<usize> {
        loop {
            if let Some(index) = self.current_group.next() {
                return Some(self.group_first_index + index);
            }
            unsafe {
                self.ctrl = NonNull::new_unchecked(self.ctrl.as_ptr().add(Group::WIDTH));
            }
            unsafe {
                self.current_group = Group::load_aligned(self.ctrl.as_ptr().cast())
                    .match_full()
                    .into_iter();
                self.group_first_index += Group::WIDTH;
            }
        }
    }
}
impl Iterator for FullBucketsIndices {
    type Item = usize;
    #[inline(always)]
    fn next(&mut self) -> Option<usize> {
        if self.items == 0 {
            return None;
        }
        let nxt = unsafe { self.next_impl() };
        debug_assert!(nxt.is_some());
        self.items -= 1;
        nxt
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.items, Some(self.items))
    }
}
impl ExactSizeIterator for FullBucketsIndices {}
impl FusedIterator for FullBucketsIndices {}
pub(crate) struct RawIntoIter<T, A: Allocator = Global> {
    iter: RawIter<T>,
    allocation: Option<(NonNull<u8>, Layout, A)>,
    marker: PhantomData<T>,
}
impl<T, A: Allocator> RawIntoIter<T, A> {
    pub(crate) fn iter(&self) -> RawIter<T> {
        self.iter.clone()
    }
}
unsafe impl<T, A: Allocator> Send for RawIntoIter<T, A>
where
    T: Send,
    A: Send,
{
}
unsafe impl<T, A: Allocator> Sync for RawIntoIter<T, A>
where
    T: Sync,
    A: Sync,
{
}
impl<T, A: Allocator> Drop for RawIntoIter<T, A> {
    fn drop(&mut self) {
        unsafe {
            self.iter.drop_elements();
            if let Some((ptr, layout, ref alloc)) = self.allocation {
                alloc.deallocate(ptr, layout);
            }
        }
    }
}
impl<T, A: Allocator> Default for RawIntoIter<T, A> {
    fn default() -> Self {
        Self {
            iter: Default::default(),
            allocation: None,
            marker: PhantomData,
        }
    }
}
impl<T, A: Allocator> Iterator for RawIntoIter<T, A> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        unsafe { Some(self.iter.next()?.read()) }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}
impl<T, A: Allocator> ExactSizeIterator for RawIntoIter<T, A> {}
impl<T, A: Allocator> FusedIterator for RawIntoIter<T, A> {}
pub(crate) struct RawDrain<'a, T, A: Allocator = Global> {
    iter: RawIter<T>,
    table: RawTableInner,
    orig_table: NonNull<RawTableInner>,
    marker: PhantomData<&'a RawTable<T, A>>,
}
impl<T, A: Allocator> RawDrain<'_, T, A> {
    pub(crate) fn iter(&self) -> RawIter<T> {
        self.iter.clone()
    }
}
unsafe impl<T, A: Allocator> Send for RawDrain<'_, T, A>
where
    T: Send,
    A: Send,
{
}
unsafe impl<T, A: Allocator> Sync for RawDrain<'_, T, A>
where
    T: Sync,
    A: Sync,
{
}
impl<T, A: Allocator> Drop for RawDrain<'_, T, A> {
    fn drop(&mut self) {
        unsafe {
            self.iter.drop_elements();
            self.table.clear_no_drop();
            self.orig_table
                .as_ptr()
                .copy_from_nonoverlapping(&raw const self.table, 1);
        }
    }
}
impl<T, A: Allocator> Iterator for RawDrain<'_, T, A> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        unsafe {
            let item = self.iter.next()?;
            Some(item.read())
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}
impl<T, A: Allocator> ExactSizeIterator for RawDrain<'_, T, A> {}
impl<T, A: Allocator> FusedIterator for RawDrain<'_, T, A> {}
pub(crate) struct RawIterHash<T> {
    inner: RawIterHashIndices,
    _marker: PhantomData<T>,
}
#[derive(Clone)]
pub(crate) struct RawIterHashIndices {
    bucket_mask: usize,
    ctrl: NonNull<u8>,
    tag_hash: Tag,
    probe_seq: ProbeSeq,
    group: Group,
    bitmask: BitMaskIter,
}
impl<T> RawIterHash<T> {
    unsafe fn new<A: Allocator>(table: &RawTable<T, A>, hash: u64) -> Self {
        RawIterHash {
            inner: unsafe { RawIterHashIndices::new(&table.table, hash) },
            _marker: PhantomData,
        }
    }
}
impl<T> Clone for RawIterHash<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}
impl<T> Default for RawIterHash<T> {
    fn default() -> Self {
        Self {
            inner: RawIterHashIndices::default(),
            _marker: PhantomData,
        }
    }
}
impl Default for RawIterHashIndices {
    fn default() -> Self {
        unsafe { RawIterHashIndices::new(&RawTableInner::NEW, 0) }
    }
}
impl RawIterHashIndices {
    unsafe fn new(table: &RawTableInner, hash: u64) -> Self {
        let tag_hash = Tag::full(hash);
        let probe_seq = table.probe_seq(hash);
        let group = unsafe { Group::load(table.ctrl(probe_seq.pos)) };
        let bitmask = group.match_tag(tag_hash).into_iter();
        RawIterHashIndices {
            bucket_mask: table.bucket_mask,
            ctrl: table.ctrl,
            tag_hash,
            probe_seq,
            group,
            bitmask,
        }
    }
}
impl<T> Iterator for RawIterHash<T> {
    type Item = Bucket<T>;
    fn next(&mut self) -> Option<Bucket<T>> {
        unsafe {
            match self.inner.next() {
                Some(index) => {
                    debug_assert!(index <= self.inner.bucket_mask);
                    let bucket = Bucket::from_base_index(self.inner.ctrl.cast(), index);
                    Some(bucket)
                }
                None => None,
            }
        }
    }
}
impl Iterator for RawIterHashIndices {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            loop {
                if let Some(bit) = self.bitmask.next() {
                    let index = (self.probe_seq.pos + bit) & self.bucket_mask;
                    return Some(index);
                }
                if likely(self.group.match_empty().any_bit_set()) {
                    return None;
                }
                self.probe_seq.move_next(self.bucket_mask);
                let index = self.probe_seq.pos;
                debug_assert!(index < self.bucket_mask + 1 + Group::WIDTH);
                let group_ctrl = self.ctrl.as_ptr().add(index).cast();
                self.group = Group::load(group_ctrl);
                self.bitmask = self.group.match_tag(self.tag_hash).into_iter();
            }
        }
    }
}
pub(crate) struct RawExtractIf<'a, T, A: Allocator> {
    pub iter: RawIter<T>,
    pub table: &'a mut RawTable<T, A>,
}
impl<T, A: Allocator> RawExtractIf<'_, T, A> {
    pub(crate) fn next<F>(&mut self, mut f: F) -> Option<T>
    where
        F: FnMut(&mut T) -> bool,
    {
        unsafe {
            for item in &mut self.iter {
                if f(item.as_mut()) {
                    return Some(self.table.remove(item).0);
                }
            }
        }
        None
    }
}
