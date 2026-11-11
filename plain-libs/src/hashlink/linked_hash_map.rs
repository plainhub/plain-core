use crate::hashbrown::hash_table::{self, HashTable};
use crate::hashlink::DefaultHashBuilder;
use alloc::boxed::Box;
use core::{
    alloc::Layout,
    borrow::Borrow,
    cmp::Ordering,
    fmt,
    hash::{BuildHasher, Hash, Hasher},
    iter::FromIterator,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ops::{Index, IndexMut},
    ptr::{self, NonNull},
};
pub enum TryReserveError {
    CapacityOverflow,
    AllocError { layout: Layout },
}
pub struct LinkedHashMap<K, V, S = DefaultHashBuilder> {
    table: HashTable<NonNull<Node<K, V>>>,
    hash_builder: S,
    values: Option<NonNull<Node<K, V>>>,
    free: Option<NonNull<Node<K, V>>>,
}
impl<K, V> LinkedHashMap<K, V> {
    #[inline]
    pub fn new() -> Self {
        Self {
            hash_builder: DefaultHashBuilder::default(),
            table: HashTable::new(),
            values: None,
            free: None,
        }
    }
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            hash_builder: DefaultHashBuilder::default(),
            table: HashTable::with_capacity(capacity),
            values: None,
            free: None,
        }
    }
}
impl<K, V, S> LinkedHashMap<K, V, S> {
    #[inline]
    pub fn with_hasher(hash_builder: S) -> Self {
        Self {
            hash_builder,
            table: HashTable::new(),
            values: None,
            free: None,
        }
    }
    #[inline]
    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
        Self {
            hash_builder,
            table: HashTable::with_capacity(capacity),
            values: None,
            free: None,
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.table.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub fn clear(&mut self) {
        self.table.clear();
        if let Some(values) = self.values {
            unsafe {
                drop_value_nodes(values);
            }
        }
    }
    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        let (head, tail) = if let Some(values) = self.values {
            unsafe {
                let ValueLinks { next, prev } = values.as_ref().links.value;
                (next.as_ptr(), prev.as_ptr())
            }
        } else {
            (ptr::null_mut(), ptr::null_mut())
        };
        Iter {
            head,
            tail,
            remaining: self.len(),
            marker: PhantomData,
        }
    }
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        let (head, tail) = if let Some(values) = self.values {
            unsafe {
                let ValueLinks { next, prev } = values.as_ref().links.value;
                (Some(next), Some(prev))
            }
        } else {
            (None, None)
        };
        IterMut {
            head,
            tail,
            remaining: self.len(),
            marker: PhantomData,
        }
    }
    #[inline]
    pub fn drain(&mut self) -> Drain<'_, K, V> {
        unsafe {
            let (head, tail) = if let Some(mut values) = self.values {
                let ValueLinks { next, prev } = values.as_ref().links.value;
                values.as_mut().links.value = ValueLinks {
                    next: values,
                    prev: values,
                };
                (Some(next), Some(prev))
            } else {
                (None, None)
            };
            let len = self.len();
            self.table.clear();
            Drain {
                free: (&mut self.free).into(),
                head,
                tail,
                remaining: len,
                marker: PhantomData,
            }
        }
    }
    #[inline]
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys { inner: self.iter() }
    }
    #[inline]
    pub fn values(&self) -> Values<'_, K, V> {
        Values { inner: self.iter() }
    }
    #[inline]
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut {
            inner: self.iter_mut(),
        }
    }
    #[inline]
    pub fn front(&self) -> Option<(&K, &V)> {
        if self.is_empty() {
            return None;
        }
        unsafe {
            let front = (*self.values.as_ptr()).links.value.next.as_ptr();
            let (key, value) = (*front).entry_ref();
            Some((key, value))
        }
    }
    #[inline]
    pub fn back(&self) -> Option<(&K, &V)> {
        if self.is_empty() {
            return None;
        }
        unsafe {
            let back = &*(*self.values.as_ptr()).links.value.prev.as_ptr();
            let (key, value) = (*back).entry_ref();
            Some((key, value))
        }
    }
    #[inline]
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        let free = self.free;
        let mut drop_filtered_values = DropFilteredValues {
            free: &mut self.free,
            cur_free: free,
        };
        self.table.retain(|&mut node| unsafe {
            let (k, v) = (*node.as_ptr()).entry_mut();
            if f(k, v) {
                true
            } else {
                drop_filtered_values.drop_later(node);
                false
            }
        });
    }
    #[inline]
    pub fn hasher(&self) -> &S {
        &self.hash_builder
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.table.capacity()
    }
}
impl<K, V, S> LinkedHashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    #[inline]
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V, S> {
        match self.raw_entry_mut().from_key(&key) {
            RawEntryMut::Occupied(occupied) => Entry::Occupied(OccupiedEntry {
                key,
                raw_entry: occupied,
            }),
            RawEntryMut::Vacant(vacant) => Entry::Vacant(VacantEntry {
                key,
                raw_entry: vacant,
            }),
        }
    }

    #[inline]
    pub fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.raw_entry().from_key(k).map(|(_, v)| v)
    }
    #[inline]
    pub fn get_key_value<Q>(&self, k: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.raw_entry().from_key(k)
    }
    #[inline]
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(k).is_some()
    }
    #[inline]
    pub fn get_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match self.raw_entry_mut().from_key(k) {
            RawEntryMut::Occupied(occupied) => Some(occupied.into_mut()),
            RawEntryMut::Vacant(_) => None,
        }
    }
    #[inline]
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        match self.raw_entry_mut().from_key(&k) {
            RawEntryMut::Occupied(mut occupied) => {
                occupied.to_back();
                Some(occupied.replace_value(v))
            }
            RawEntryMut::Vacant(vacant) => {
                vacant.insert(k, v);
                None
            }
        }
    }

    #[inline]
    pub fn replace(&mut self, k: K, v: V) -> Option<V> {
        match self.raw_entry_mut().from_key(&k) {
            RawEntryMut::Occupied(mut occupied) => Some(occupied.replace_value(v)),
            RawEntryMut::Vacant(vacant) => {
                vacant.insert(k, v);
                None
            }
        }
    }
    #[inline]
    pub fn remove<Q>(&mut self, k: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match self.raw_entry_mut().from_key(k) {
            RawEntryMut::Occupied(occupied) => Some(occupied.remove()),
            RawEntryMut::Vacant(_) => None,
        }
    }
    #[inline]
    pub fn remove_entry<Q>(&mut self, k: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match self.raw_entry_mut().from_key(k) {
            RawEntryMut::Occupied(occupied) => Some(occupied.remove_entry()),
            RawEntryMut::Vacant(_) => None,
        }
    }
    #[inline]
    pub fn pop_front(&mut self) -> Option<(K, V)> {
        if self.is_empty() {
            return None;
        }
        unsafe {
            let front = (*self.values.as_ptr()).links.value.next;
            let hash = hash_node(&self.hash_builder, front);
            match self
                .raw_entry_mut()
                .from_hash(hash, |k| k.eq(front.as_ref().key_ref()))
            {
                RawEntryMut::Occupied(occupied) => Some(occupied.remove_entry()),
                RawEntryMut::Vacant(_) => None,
            }
        }
    }
    #[inline]
    pub fn pop_back(&mut self) -> Option<(K, V)> {
        if self.is_empty() {
            return None;
        }
        unsafe {
            let back = (*self.values.as_ptr()).links.value.prev;
            let hash = hash_node(&self.hash_builder, back);
            match self
                .raw_entry_mut()
                .from_hash(hash, |k| k.eq(back.as_ref().key_ref()))
            {
                RawEntryMut::Occupied(occupied) => Some(occupied.remove_entry()),
                RawEntryMut::Vacant(_) => None,
            }
        }
    }
    #[inline]
    pub fn to_front<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match self.raw_entry_mut().from_key(k) {
            RawEntryMut::Occupied(mut occupied) => {
                occupied.to_front();
                Some(occupied.into_mut())
            }
            RawEntryMut::Vacant(_) => None,
        }
    }
    #[inline]
    pub fn to_back<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match self.raw_entry_mut().from_key(k) {
            RawEntryMut::Occupied(mut occupied) => {
                occupied.to_back();
                Some(occupied.into_mut())
            }
            RawEntryMut::Vacant(_) => None,
        }
    }
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        let hash_builder = &self.hash_builder;
        self.table
            .reserve(additional, move |&n| unsafe { hash_node(hash_builder, n) });
    }
    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        let hash_builder = &self.hash_builder;
        self.table
            .try_reserve(additional, move |&n| unsafe { hash_node(hash_builder, n) })
            .map_err(|e| match e {
                crate::hashbrown::TryReserveError::CapacityOverflow => {
                    TryReserveError::CapacityOverflow
                }
                crate::hashbrown::TryReserveError::AllocError { layout } => {
                    TryReserveError::AllocError { layout }
                }
            })
    }
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        let hash_builder = &self.hash_builder;
        unsafe {
            self.table
                .shrink_to_fit(move |&n| hash_node(hash_builder, n));
            drop_free_nodes(self.free.take());
        }
    }

    fn cursor_mut(&mut self) -> CursorMut<'_, K, V, S> {
        unsafe { ensure_guard_node(&mut self.values) };
        CursorMut {
            cur: self.values.as_ptr(),
            hash_builder: &self.hash_builder,
            free: &mut self.free,
            values: &mut self.values,
            table: &mut self.table,
        }
    }
}
impl<K, V, S> LinkedHashMap<K, V, S>
where
    S: BuildHasher,
{
    #[inline]
    pub fn raw_entry(&self) -> RawEntryBuilder<'_, K, V, S> {
        RawEntryBuilder { map: self }
    }
    #[inline]
    pub fn raw_entry_mut(&mut self) -> RawEntryBuilderMut<'_, K, V, S> {
        RawEntryBuilderMut { map: self }
    }
}
impl<K, V, S> Default for LinkedHashMap<K, V, S>
where
    S: Default,
{
    #[inline]
    fn default() -> Self {
        Self::with_hasher(S::default())
    }
}
impl<K: Hash + Eq, V, S: BuildHasher + Default> FromIterator<(K, V)> for LinkedHashMap<K, V, S> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut map = Self::with_capacity_and_hasher(iter.size_hint().0, S::default());
        map.extend(iter);
        map
    }
}
impl<K, V, S> fmt::Debug for LinkedHashMap<K, V, S>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_map().entries(self).finish()
    }
}
impl<K: Hash + Eq, V: PartialEq, S: BuildHasher> PartialEq for LinkedHashMap<K, V, S> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other)
    }
}
impl<K: Hash + Eq, V: Eq, S: BuildHasher> Eq for LinkedHashMap<K, V, S> {}
impl<K: Hash + Eq + PartialOrd, V: PartialOrd, S: BuildHasher> PartialOrd
    for LinkedHashMap<K, V, S>
{
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other)
    }
    #[inline]
    fn lt(&self, other: &Self) -> bool {
        self.iter().lt(other)
    }
    #[inline]
    fn le(&self, other: &Self) -> bool {
        self.iter().le(other)
    }
    #[inline]
    fn ge(&self, other: &Self) -> bool {
        self.iter().ge(other)
    }
    #[inline]
    fn gt(&self, other: &Self) -> bool {
        self.iter().gt(other)
    }
}
impl<K: Hash + Eq + Ord, V: Ord, S: BuildHasher> Ord for LinkedHashMap<K, V, S> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other)
    }
}
impl<K: Hash + Eq, V: Hash, S: BuildHasher> Hash for LinkedHashMap<K, V, S> {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        for e in self.iter() {
            e.hash(h);
        }
    }
}
impl<K, V, S> Drop for LinkedHashMap<K, V, S> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            if let Some(values) = self.values {
                drop_value_nodes(values);
                let _ = Box::from_raw(values.as_ptr());
            }
            drop_free_nodes(self.free);
        }
    }
}
unsafe impl<K: Send, V: Send, S: Send> Send for LinkedHashMap<K, V, S> {}
unsafe impl<K: Sync, V: Sync, S: Sync> Sync for LinkedHashMap<K, V, S> {}
impl<'a, K, V, S, Q> Index<&'a Q> for LinkedHashMap<K, V, S>
where
    K: Hash + Eq + Borrow<Q>,
    S: BuildHasher,
    Q: Eq + Hash + ?Sized,
{
    type Output = V;
    #[inline]
    fn index(&self, index: &'a Q) -> &V {
        self.get(index).expect("no entry found for key")
    }
}
impl<'a, K, V, S, Q> IndexMut<&'a Q> for LinkedHashMap<K, V, S>
where
    K: Hash + Eq + Borrow<Q>,
    S: BuildHasher,
    Q: Eq + Hash + ?Sized,
{
    #[inline]
    fn index_mut(&mut self, index: &'a Q) -> &mut V {
        self.get_mut(index).expect("no entry found for key")
    }
}
impl<K: Hash + Eq + Clone, V: Clone, S: BuildHasher + Clone> Clone for LinkedHashMap<K, V, S> {
    #[inline]
    fn clone(&self) -> Self {
        let mut map = Self::with_hasher(self.hash_builder.clone());
        map.extend(self.iter().map(|(k, v)| (k.clone(), v.clone())));
        map
    }
}
impl<K: Hash + Eq, V, S: BuildHasher> Extend<(K, V)> for LinkedHashMap<K, V, S> {
    #[inline]
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}
impl<'a, K, V, S> Extend<(&'a K, &'a V)> for LinkedHashMap<K, V, S>
where
    K: 'a + Hash + Eq + Copy,
    V: 'a + Copy,
    S: BuildHasher,
{
    #[inline]
    fn extend<I: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: I) {
        for (&k, &v) in iter {
            self.insert(k, v);
        }
    }
}
pub enum Entry<'a, K, V, S> {
    Occupied(OccupiedEntry<'a, K, V, S>),
    Vacant(VacantEntry<'a, K, V, S>),
}
impl<K: fmt::Debug, V: fmt::Debug, S> fmt::Debug for Entry<'_, K, V, S> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Entry::Vacant(ref v) => f.debug_tuple("Entry").field(v).finish(),
            Entry::Occupied(ref o) => f.debug_tuple("Entry").field(o).finish(),
        }
    }
}
impl<'a, K, V, S> Entry<'a, K, V, S> {
    #[inline]
    pub fn or_insert(self, default: V) -> &'a mut V
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            Entry::Occupied(mut entry) => {
                entry.to_back();
                entry.into_mut()
            }
            Entry::Vacant(entry) => entry.insert(default),
        }
    }
    #[inline]
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            Entry::Occupied(mut entry) => {
                entry.to_back();
                entry.into_mut()
            }
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }
    #[inline]
    pub fn key(&self) -> &K {
        match *self {
            Entry::Occupied(ref entry) => entry.key(),
            Entry::Vacant(ref entry) => entry.key(),
        }
    }
    #[inline]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        match self {
            Entry::Occupied(mut entry) => {
                f(entry.get_mut());
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }
}
pub struct OccupiedEntry<'a, K, V, S> {
    key: K,
    raw_entry: RawOccupiedEntryMut<'a, K, V, S>,
}
impl<K: fmt::Debug, V: fmt::Debug, S> fmt::Debug for OccupiedEntry<'_, K, V, S> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("key", self.key())
            .field("value", self.get())
            .finish()
    }
}
impl<'a, K, V, S> OccupiedEntry<'a, K, V, S> {
    #[inline]
    pub fn key(&self) -> &K {
        self.raw_entry.key()
    }
    #[inline]
    pub fn remove_entry(self) -> (K, V) {
        self.raw_entry.remove_entry()
    }
    #[inline]
    pub fn get(&self) -> &V {
        self.raw_entry.get()
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut V {
        self.raw_entry.get_mut()
    }
    #[inline]
    pub fn into_mut(self) -> &'a mut V {
        self.raw_entry.into_mut()
    }
    #[inline]
    pub fn to_back(&mut self) {
        self.raw_entry.to_back()
    }
    #[inline]
    pub fn to_front(&mut self) {
        self.raw_entry.to_front()
    }
    #[inline]
    pub fn insert(&mut self, value: V) -> V {
        self.raw_entry.to_back();
        self.raw_entry.replace_value(value)
    }
    #[inline]
    pub fn remove(self) -> V {
        self.raw_entry.remove()
    }
    #[inline]
    pub fn insert_entry(mut self, value: V) -> (K, V) {
        self.raw_entry.to_back();
        self.replace_entry(value)
    }
    #[inline]
    pub fn cursor_mut(self) -> CursorMut<'a, K, V, S>
    where
        K: Eq + Hash,
        S: BuildHasher,
    {
        self.raw_entry.cursor_mut()
    }
    #[inline]
    pub fn raw_entry_mut(self) -> RawOccupiedEntryMut<'a, K, V, S>
    where
        K: Eq + Hash,
        S: BuildHasher,
    {
        self.raw_entry
    }
    pub fn replace_entry(mut self, value: V) -> (K, V) {
        let old_key = mem::replace(self.raw_entry.key_mut(), self.key);
        let old_value = mem::replace(self.raw_entry.get_mut(), value);
        (old_key, old_value)
    }
    #[inline]
    pub fn replace_key(mut self) -> K {
        mem::replace(self.raw_entry.key_mut(), self.key)
    }
}
pub struct VacantEntry<'a, K, V, S> {
    key: K,
    raw_entry: RawVacantEntryMut<'a, K, V, S>,
}
impl<K: fmt::Debug, V, S> fmt::Debug for VacantEntry<'_, K, V, S> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntry").field(self.key()).finish()
    }
}
impl<'a, K, V, S> VacantEntry<'a, K, V, S> {
    #[inline]
    pub fn key(&self) -> &K {
        &self.key
    }
    #[inline]
    pub fn into_key(self) -> K {
        self.key
    }
    #[inline]
    pub fn insert(self, value: V) -> &'a mut V
    where
        K: Hash,
        S: BuildHasher,
    {
        self.raw_entry.insert(self.key, value).1
    }
    pub fn insert_entry(self, value: V) -> RawOccupiedEntryMut<'a, K, V, S>
    where
        K: Hash,
        S: BuildHasher,
    {
        self.raw_entry.insert_entry(self.key, value)
    }
    #[inline]
    pub fn raw_entry_mut(self) -> RawVacantEntryMut<'a, K, V, S>
    where
        K: Eq + Hash,
        S: BuildHasher,
    {
        self.raw_entry
    }
}
pub struct RawEntryBuilder<'a, K, V, S> {
    map: &'a LinkedHashMap<K, V, S>,
}
impl<'a, K, V, S> RawEntryBuilder<'a, K, V, S>
where
    S: BuildHasher,
{
    #[inline]
    pub fn from_key<Q>(self, k: &Q) -> Option<(&'a K, &'a V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = hash_key(&self.map.hash_builder, k);
        self.from_key_hashed_nocheck(hash, k)
    }
    #[inline]
    pub fn from_key_hashed_nocheck<Q>(self, hash: u64, k: &Q) -> Option<(&'a K, &'a V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.from_hash(hash, move |o| k.eq(o.borrow()))
    }
    #[inline]
    pub fn from_hash(
        self,
        hash: u64,
        mut is_match: impl FnMut(&K) -> bool,
    ) -> Option<(&'a K, &'a V)> {
        unsafe {
            let node = self
                .map
                .table
                .find(hash, move |k| is_match((*k).as_ref().key_ref()))?;
            let (key, value) = (*node.as_ptr()).entry_ref();
            Some((key, value))
        }
    }
}
pub struct RawEntryBuilderMut<'a, K, V, S> {
    map: &'a mut LinkedHashMap<K, V, S>,
}
impl<'a, K, V, S> RawEntryBuilderMut<'a, K, V, S>
where
    S: BuildHasher,
{
    #[inline]
    pub fn from_key<Q>(self, k: &Q) -> RawEntryMut<'a, K, V, S>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = hash_key(&self.map.hash_builder, k);
        self.from_key_hashed_nocheck(hash, k)
    }
    #[inline]
    pub fn from_key_hashed_nocheck<Q>(self, hash: u64, k: &Q) -> RawEntryMut<'a, K, V, S>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.from_hash(hash, move |o| k.eq(o.borrow()))
    }
    #[inline]
    pub fn from_hash(
        self,
        hash: u64,
        mut is_match: impl FnMut(&K) -> bool,
    ) -> RawEntryMut<'a, K, V, S> {
        let entry = self
            .map
            .table
            .find_entry(hash, move |k| is_match(unsafe { (*k).as_ref().key_ref() }));
        match entry {
            Ok(occupied) => RawEntryMut::Occupied(RawOccupiedEntryMut {
                hash_builder: &self.map.hash_builder,
                free: &mut self.map.free,
                values: &mut self.map.values,
                entry: occupied,
            }),
            Err(absent) => RawEntryMut::Vacant(RawVacantEntryMut {
                hash_builder: &self.map.hash_builder,
                values: &mut self.map.values,
                free: &mut self.map.free,
                entry: absent,
            }),
        }
    }
}
pub enum RawEntryMut<'a, K, V, S> {
    Occupied(RawOccupiedEntryMut<'a, K, V, S>),
    Vacant(RawVacantEntryMut<'a, K, V, S>),
}
impl<'a, K, V, S> RawEntryMut<'a, K, V, S> {
    #[inline]
    pub fn or_insert(self, default_key: K, default_val: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            RawEntryMut::Occupied(mut entry) => {
                entry.to_back();
                entry.into_key_value()
            }
            RawEntryMut::Vacant(entry) => entry.insert(default_key, default_val),
        }
    }
    #[inline]
    pub fn or_insert_with<F>(self, default: F) -> (&'a mut K, &'a mut V)
    where
        F: FnOnce() -> (K, V),
        K: Hash,
        S: BuildHasher,
    {
        match self {
            RawEntryMut::Occupied(mut entry) => {
                entry.to_back();
                entry.into_key_value()
            }
            RawEntryMut::Vacant(entry) => {
                let (k, v) = default();
                entry.insert(k, v)
            }
        }
    }
    #[inline]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut K, &mut V),
    {
        match self {
            RawEntryMut::Occupied(mut entry) => {
                {
                    let (k, v) = entry.get_key_value_mut();
                    f(k, v);
                }
                RawEntryMut::Occupied(entry)
            }
            RawEntryMut::Vacant(entry) => RawEntryMut::Vacant(entry),
        }
    }
}
pub struct RawOccupiedEntryMut<'a, K, V, S> {
    hash_builder: &'a S,
    free: &'a mut Option<NonNull<Node<K, V>>>,
    values: &'a mut Option<NonNull<Node<K, V>>>,
    entry: hash_table::OccupiedEntry<'a, NonNull<Node<K, V>>>,
}
impl<'a, K, V, S> RawOccupiedEntryMut<'a, K, V, S> {
    #[inline]
    pub fn key(&self) -> &K {
        self.get_key_value().0
    }
    #[inline]
    pub fn key_mut(&mut self) -> &mut K {
        self.get_key_value_mut().0
    }
    #[inline]
    pub fn into_key(self) -> &'a mut K {
        self.into_key_value().0
    }
    #[inline]
    pub fn get(&self) -> &V {
        self.get_key_value().1
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut V {
        self.get_key_value_mut().1
    }
    #[inline]
    pub fn into_mut(self) -> &'a mut V {
        self.into_key_value().1
    }
    #[inline]
    pub fn get_key_value(&self) -> (&K, &V) {
        unsafe {
            let node = *self.entry.get();
            let (key, value) = (*node.as_ptr()).entry_ref();
            (key, value)
        }
    }
    #[inline]
    pub fn get_key_value_mut(&mut self) -> (&mut K, &mut V) {
        unsafe {
            let node = *self.entry.get_mut();
            let (key, value) = (*node.as_ptr()).entry_mut();
            (key, value)
        }
    }
    #[inline]
    pub fn into_key_value(self) -> (&'a mut K, &'a mut V) {
        unsafe {
            let node = *self.entry.into_mut();
            let (key, value) = (*node.as_ptr()).entry_mut();
            (key, value)
        }
    }
    #[inline]
    pub fn to_back(&mut self) {
        unsafe {
            let node = *self.entry.get_mut();
            detach_node(node);
            attach_before(node, NonNull::new_unchecked(self.values.as_ptr()));
        }
    }
    #[inline]
    pub fn to_front(&mut self) {
        unsafe {
            let node = *self.entry.get_mut();
            detach_node(node);
            attach_before(node, (*self.values.as_ptr()).links.value.next);
        }
    }
    #[inline]
    pub fn replace_value(&mut self, value: V) -> V {
        unsafe {
            let mut node = *self.entry.get_mut();
            mem::replace(&mut node.as_mut().entry_mut().1, value)
        }
    }
    #[inline]
    pub fn replace_key(&mut self, key: K) -> K {
        unsafe {
            let mut node = *self.entry.get_mut();
            mem::replace(&mut node.as_mut().entry_mut().0, key)
        }
    }
    #[inline]
    pub fn remove(self) -> V {
        self.remove_entry().1
    }
    #[inline]
    pub fn remove_entry(self) -> (K, V) {
        let node = self.entry.remove().0;
        unsafe { remove_node(self.free, node) }
    }
    #[inline]
    pub fn cursor_mut(self) -> CursorMut<'a, K, V, S>
    where
        K: Eq + Hash,
        S: BuildHasher,
    {
        CursorMut {
            cur: self.entry.get().as_ptr(),
            hash_builder: self.hash_builder,
            free: self.free,
            values: self.values,
            table: self.entry.into_table(),
        }
    }
}
pub struct RawVacantEntryMut<'a, K, V, S> {
    hash_builder: &'a S,
    values: &'a mut Option<NonNull<Node<K, V>>>,
    free: &'a mut Option<NonNull<Node<K, V>>>,
    entry: hash_table::AbsentEntry<'a, NonNull<Node<K, V>>>,
}
impl<'a, K, V, S> RawVacantEntryMut<'a, K, V, S> {
    #[inline]
    pub fn insert(self, key: K, value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        let hash = hash_key(self.hash_builder, &key);
        self.insert_hashed_nocheck(hash, key, value)
    }
    #[inline]
    pub fn insert_hashed_nocheck(self, hash: u64, key: K, value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        let hash_builder = self.hash_builder;
        self.insert_with_hasher(hash, key, value, |k| hash_key(hash_builder, k))
    }
    #[inline]
    pub fn insert_with_hasher(
        self,
        hash: u64,
        key: K,
        value: V,
        hasher: impl Fn(&K) -> u64,
    ) -> (&'a mut K, &'a mut V)
    where
        S: BuildHasher,
    {
        self.insert_entry_with_hasher(hash, key, value, hasher)
            .into_key_value()
    }
    #[inline]
    pub fn insert_entry_with_hasher(
        self,
        hash: u64,
        key: K,
        value: V,
        hasher: impl Fn(&K) -> u64,
    ) -> RawOccupiedEntryMut<'a, K, V, S>
    where
        S: BuildHasher,
    {
        unsafe {
            ensure_guard_node(self.values);
            let mut new_node = allocate_node(self.free);
            new_node.as_mut().put_entry((key, value));
            attach_before(new_node, NonNull::new_unchecked(self.values.as_ptr()));
            let node = self
                .entry
                .into_table()
                .insert_unique(hash, new_node, move |k| hasher((*k).as_ref().key_ref()));
            RawOccupiedEntryMut {
                hash_builder: self.hash_builder,
                free: self.free,
                values: self.values,
                entry: node,
            }
        }
    }
    #[inline]
    pub fn insert_entry_hashed_nocheck(
        self,
        hash: u64,
        key: K,
        value: V,
    ) -> RawOccupiedEntryMut<'a, K, V, S>
    where
        K: Hash,
        S: BuildHasher,
    {
        let hash_builder = self.hash_builder;
        self.insert_entry_with_hasher(hash, key, value, |k| hash_key(hash_builder, k))
    }
    #[inline]
    pub fn insert_entry(self, key: K, value: V) -> RawOccupiedEntryMut<'a, K, V, S>
    where
        K: Hash,
        S: BuildHasher,
    {
        let hash = hash_key(self.hash_builder, &key);
        self.insert_entry_hashed_nocheck(hash, key, value)
    }
}
impl<K, V, S> fmt::Debug for RawEntryBuilderMut<'_, K, V, S> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawEntryBuilder").finish()
    }
}
impl<K: fmt::Debug, V: fmt::Debug, S> fmt::Debug for RawEntryMut<'_, K, V, S> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RawEntryMut::Vacant(ref v) => f.debug_tuple("RawEntry").field(v).finish(),
            RawEntryMut::Occupied(ref o) => f.debug_tuple("RawEntry").field(o).finish(),
        }
    }
}
impl<K: fmt::Debug, V: fmt::Debug, S> fmt::Debug for RawOccupiedEntryMut<'_, K, V, S> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawOccupiedEntryMut")
            .field("key", self.key())
            .field("value", self.get())
            .finish()
    }
}
impl<K, V, S> fmt::Debug for RawVacantEntryMut<'_, K, V, S> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawVacantEntryMut").finish()
    }
}
impl<K, V, S> fmt::Debug for RawEntryBuilder<'_, K, V, S> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawEntryBuilder").finish()
    }
}
unsafe impl<K, V, S> Send for RawOccupiedEntryMut<'_, K, V, S>
where
    K: Send,
    V: Send,
    S: Send,
{
}
unsafe impl<K, V, S> Sync for RawOccupiedEntryMut<'_, K, V, S>
where
    K: Sync,
    V: Sync,
    S: Sync,
{
}
unsafe impl<K, V, S> Send for RawVacantEntryMut<'_, K, V, S>
where
    K: Send,
    V: Send,
    S: Send,
{
}
unsafe impl<K, V, S> Sync for RawVacantEntryMut<'_, K, V, S>
where
    K: Sync,
    V: Sync,
    S: Sync,
{
}
pub struct Iter<'a, K, V> {
    head: *const Node<K, V>,
    tail: *const Node<K, V>,
    remaining: usize,
    marker: PhantomData<(&'a K, &'a V)>,
}
pub struct IterMut<'a, K, V> {
    head: Option<NonNull<Node<K, V>>>,
    tail: Option<NonNull<Node<K, V>>>,
    remaining: usize,
    marker: PhantomData<(&'a K, &'a mut V)>,
}
pub struct IntoIter<K, V> {
    head: Option<NonNull<Node<K, V>>>,
    tail: Option<NonNull<Node<K, V>>>,
    remaining: usize,
    marker: PhantomData<(K, V)>,
}
pub struct Drain<'a, K, V> {
    free: NonNull<Option<NonNull<Node<K, V>>>>,
    head: Option<NonNull<Node<K, V>>>,
    tail: Option<NonNull<Node<K, V>>>,
    remaining: usize,
    marker: PhantomData<(K, V, &'a LinkedHashMap<K, V>)>,
}
impl<K, V> IterMut<'_, K, V> {
    #[inline]
    pub(crate) fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            head: self.head.as_ptr(),
            tail: self.tail.as_ptr(),
            remaining: self.remaining,
            marker: PhantomData,
        }
    }
}
impl<K, V> IntoIter<K, V> {
    #[inline]
    pub(crate) fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            head: self.head.as_ptr(),
            tail: self.tail.as_ptr(),
            remaining: self.remaining,
            marker: PhantomData,
        }
    }
}
impl<K, V> Drain<'_, K, V> {
    #[inline]
    pub(crate) fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            head: self.head.as_ptr(),
            tail: self.tail.as_ptr(),
            remaining: self.remaining,
            marker: PhantomData,
        }
    }
}
unsafe impl<K, V> Send for Iter<'_, K, V>
where
    K: Sync,
    V: Sync,
{
}
unsafe impl<K, V> Send for IterMut<'_, K, V>
where
    K: Send,
    V: Send,
{
}
unsafe impl<K, V> Send for IntoIter<K, V>
where
    K: Send,
    V: Send,
{
}
unsafe impl<K, V> Send for Drain<'_, K, V>
where
    K: Send,
    V: Send,
{
}
unsafe impl<K, V> Sync for Iter<'_, K, V>
where
    K: Sync,
    V: Sync,
{
}
unsafe impl<K, V> Sync for IterMut<'_, K, V>
where
    K: Sync,
    V: Sync,
{
}
unsafe impl<K, V> Sync for IntoIter<K, V>
where
    K: Sync,
    V: Sync,
{
}
unsafe impl<K, V> Sync for Drain<'_, K, V>
where
    K: Sync,
    V: Sync,
{
}
impl<K, V> Clone for Iter<'_, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Iter { ..*self }
    }
}
impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Iter<'_, K, V> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
impl<K, V> fmt::Debug for IterMut<'_, K, V>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl<K, V> fmt::Debug for IntoIter<K, V>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl<K, V> fmt::Debug for Drain<'_, K, V>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    #[inline]
    fn next(&mut self) -> Option<(&'a K, &'a V)> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            unsafe {
                let (key, value) = (*self.head).entry_ref();
                self.head = (*self.head).links.value.next.as_ptr();
                Some((key, value))
            }
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);
    #[inline]
    fn next(&mut self) -> Option<(&'a K, &'a mut V)> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            unsafe {
                let head = self.head.as_ptr();
                let (key, value) = (*head).entry_mut();
                self.head = Some((*head).links.value.next);
                Some((key, value))
            }
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<K, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);
    #[inline]
    fn next(&mut self) -> Option<(K, V)> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        unsafe {
            let head = self.head.as_ptr();
            self.head = Some((*head).links.value.next);
            let mut e = Box::from_raw(head);
            Some(e.take_entry())
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<K, V> Iterator for Drain<'_, K, V> {
    type Item = (K, V);
    #[inline]
    fn next(&mut self) -> Option<(K, V)> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        unsafe {
            let mut head = NonNull::new_unchecked(self.head.as_ptr());
            self.head = Some(head.as_ref().links.value.next);
            let entry = head.as_mut().take_entry();
            push_free(self.free.as_mut(), head);
            Some(entry)
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<'a, K, V> DoubleEndedIterator for Iter<'a, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<(&'a K, &'a V)> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            unsafe {
                let tail = self.tail;
                self.tail = (*tail).links.value.prev.as_ptr();
                let (key, value) = (*tail).entry_ref();
                Some((key, value))
            }
        }
    }
}
impl<'a, K, V> DoubleEndedIterator for IterMut<'a, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<(&'a K, &'a mut V)> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            unsafe {
                let tail = self.tail.as_ptr();
                self.tail = Some((*tail).links.value.prev);
                let (key, value) = (*tail).entry_mut();
                Some((key, value))
            }
        }
    }
}
impl<K, V> DoubleEndedIterator for IntoIter<K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<(K, V)> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        unsafe {
            let mut e = *Box::from_raw(self.tail.as_ptr());
            self.tail = Some(e.links.value.prev);
            Some(e.take_entry())
        }
    }
}
impl<K, V> DoubleEndedIterator for Drain<'_, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<(K, V)> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        unsafe {
            let mut tail = NonNull::new_unchecked(self.tail.as_ptr());
            self.tail = Some(tail.as_ref().links.value.prev);
            let entry = tail.as_mut().take_entry();
            push_free(&mut *self.free.as_ptr(), tail);
            Some(entry)
        }
    }
}
impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}
impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {}
impl<K, V> ExactSizeIterator for IntoIter<K, V> {}
impl<K, V> Drop for IntoIter<K, V> {
    #[inline]
    fn drop(&mut self) {
        for _ in 0..self.remaining {
            unsafe {
                let tail = self.tail.as_ptr();
                self.tail = Some((*tail).links.value.prev);
                (*tail).take_entry();
                let _ = Box::from_raw(tail);
            }
        }
    }
}
impl<K, V> Drop for Drain<'_, K, V> {
    #[inline]
    fn drop(&mut self) {
        for _ in 0..self.remaining {
            unsafe {
                let mut tail = NonNull::new_unchecked(self.tail.as_ptr());
                self.tail = Some(tail.as_ref().links.value.prev);
                tail.as_mut().take_entry();
                push_free(&mut *self.free.as_ptr(), tail);
            }
        }
    }
}
pub struct CursorMut<'a, K, V, S> {
    cur: *mut Node<K, V>,
    hash_builder: &'a S,
    free: &'a mut Option<NonNull<Node<K, V>>>,
    values: &'a mut Option<NonNull<Node<K, V>>>,
    table: &'a mut crate::hashbrown::HashTable<NonNull<Node<K, V>>>,
}
impl<'a, K, V, S> CursorMut<'a, K, V, S> {
    #[inline]
    pub fn current(&mut self) -> Option<(&K, &mut V)> {
        unsafe {
            let at = NonNull::new_unchecked(self.cur);
            self.peek(at)
        }
    }

    #[inline]
    fn peek(&mut self, at: NonNull<Node<K, V>>) -> Option<(&K, &mut V)> {
        if let Some(values) = self.values {
            unsafe {
                let node = at.as_ptr();
                if node == values.as_ptr() {
                    None
                } else {
                    let entry = (*node).entry_mut();
                    Some((&entry.0, &mut entry.1))
                }
            }
        } else {
            None
        }
    }
    #[inline]
    pub fn move_next(&mut self) {
        let at = unsafe { (*self.cur).links.value.next };
        self.muv(at);
    }
    #[inline]
    pub fn move_prev(&mut self) {
        let at = unsafe { (*self.cur).links.value.prev };
        self.muv(at);
    }
    #[inline]
    fn muv(&mut self, at: NonNull<Node<K, V>>) {
        self.cur = at.as_ptr();
    }
    #[inline]
    pub fn insert_before(&mut self, key: K, value: V) -> Option<V>
    where
        K: Eq + Hash,
        S: BuildHasher,
    {
        let before = unsafe { NonNull::new_unchecked(self.cur) };
        self.insert(key, value, before)
    }

    #[inline]
    fn insert(&mut self, key: K, value: V, before: NonNull<Node<K, V>>) -> Option<V>
    where
        K: Eq + Hash,
        S: BuildHasher,
    {
        unsafe {
            let hash = hash_key(self.hash_builder, &key);
            let i_entry = self
                .table
                .find_entry(hash, |o| (*o).as_ref().key_ref().eq(&key));
            match i_entry {
                Ok(occupied) => {
                    let mut node = *occupied.into_mut();
                    let pv = mem::replace(&mut node.as_mut().entry_mut().1, value);
                    if node != before {
                        detach_node(node);
                        attach_before(node, before);
                    }
                    Some(pv)
                }
                Err(_) => {
                    let mut new_node = allocate_node(self.free);
                    new_node.as_mut().put_entry((key, value));
                    attach_before(new_node, before);
                    let hash_builder = self.hash_builder;
                    self.table.insert_unique(hash, new_node, move |k| {
                        hash_key(hash_builder, (*k).as_ref().key_ref())
                    });
                    None
                }
            }
        }
    }
}
pub struct Keys<'a, K, V> {
    inner: Iter<'a, K, V>,
}
impl<K: fmt::Debug, V> fmt::Debug for Keys<'_, K, V> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
impl<'a, K, V> Clone for Keys<'a, K, V> {
    #[inline]
    fn clone(&self) -> Keys<'a, K, V> {
        Keys {
            inner: self.inner.clone(),
        }
    }
}
impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;
    #[inline]
    fn next(&mut self) -> Option<&'a K> {
        self.inner.next().map(|e| e.0)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
impl<'a, K, V> DoubleEndedIterator for Keys<'a, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a K> {
        self.inner.next_back().map(|e| e.0)
    }
}
impl<K, V> ExactSizeIterator for Keys<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
pub struct Values<'a, K, V> {
    inner: Iter<'a, K, V>,
}
impl<K, V> Clone for Values<'_, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Values {
            inner: self.inner.clone(),
        }
    }
}
impl<K, V: fmt::Debug> fmt::Debug for Values<'_, K, V> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}
impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;
    #[inline]
    fn next(&mut self) -> Option<&'a V> {
        self.inner.next().map(|e| e.1)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
impl<'a, K, V> DoubleEndedIterator for Values<'a, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a V> {
        self.inner.next_back().map(|e| e.1)
    }
}
impl<K, V> ExactSizeIterator for Values<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
pub struct ValuesMut<'a, K, V> {
    inner: IterMut<'a, K, V>,
}
impl<K, V> fmt::Debug for ValuesMut<'_, K, V>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.inner.iter()).finish()
    }
}
impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;
    #[inline]
    fn next(&mut self) -> Option<&'a mut V> {
        self.inner.next().map(|e| e.1)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
impl<'a, K, V> DoubleEndedIterator for ValuesMut<'a, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut V> {
        self.inner.next_back().map(|e| e.1)
    }
}
impl<K, V> ExactSizeIterator for ValuesMut<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<'a, K, V, S> IntoIterator for &'a LinkedHashMap<K, V, S> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    #[inline]
    fn into_iter(self) -> Iter<'a, K, V> {
        self.iter()
    }
}
impl<'a, K, V, S> IntoIterator for &'a mut LinkedHashMap<K, V, S> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;
    #[inline]
    fn into_iter(self) -> IterMut<'a, K, V> {
        self.iter_mut()
    }
}
impl<K, V, S> IntoIterator for LinkedHashMap<K, V, S> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;
    #[inline]
    fn into_iter(mut self) -> IntoIter<K, V> {
        unsafe {
            let (head, tail) = if let Some(values) = self.values {
                let ValueLinks {
                    next: head,
                    prev: tail,
                } = values.as_ref().links.value;
                let _ = Box::from_raw(self.values.as_ptr());
                self.values = None;
                (Some(head), Some(tail))
            } else {
                (None, None)
            };
            let len = self.len();
            drop_free_nodes(self.free.take());
            self.table.clear();
            IntoIter {
                head,
                tail,
                remaining: len,
                marker: PhantomData,
            }
        }
    }
}
struct ValueLinks<K, V> {
    next: NonNull<Node<K, V>>,
    prev: NonNull<Node<K, V>>,
}
impl<K, V> Clone for ValueLinks<K, V> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<K, V> Copy for ValueLinks<K, V> {}
struct FreeLink<K, V> {
    next: Option<NonNull<Node<K, V>>>,
}
impl<K, V> Clone for FreeLink<K, V> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<K, V> Copy for FreeLink<K, V> {}
union Links<K, V> {
    value: ValueLinks<K, V>,
    free: FreeLink<K, V>,
}
struct Node<K, V> {
    entry: MaybeUninit<(K, V)>,
    links: Links<K, V>,
}
impl<K, V> Node<K, V> {
    #[inline]
    unsafe fn put_entry(&mut self, entry: (K, V)) {
        unsafe { self.entry.as_mut_ptr().write(entry) }
    }
    #[inline]
    unsafe fn entry_ref(&self) -> &(K, V) {
        unsafe { &*self.entry.as_ptr() }
    }
    #[inline]
    unsafe fn key_ref(&self) -> &K {
        unsafe { &(*self.entry.as_ptr()).0 }
    }
    #[inline]
    unsafe fn entry_mut(&mut self) -> &mut (K, V) {
        unsafe { &mut *self.entry.as_mut_ptr() }
    }
    #[inline]
    unsafe fn take_entry(&mut self) -> (K, V) {
        unsafe { self.entry.as_ptr().read() }
    }
}
trait OptNonNullExt<T> {
    fn as_ptr(self) -> *mut T;
}
impl<T> OptNonNullExt<T> for Option<NonNull<T>> {
    #[inline]
    fn as_ptr(self) -> *mut T {
        match self {
            Some(ptr) => ptr.as_ptr(),
            None => ptr::null_mut(),
        }
    }
}
#[inline]
unsafe fn ensure_guard_node<K, V>(head: &mut Option<NonNull<Node<K, V>>>) {
    unsafe {
        if head.is_none() {
            let mut p = unsafe {
                NonNull::new_unchecked(Box::into_raw(Box::new(Node {
                    entry: MaybeUninit::uninit(),
                    links: Links {
                        value: ValueLinks {
                            next: NonNull::dangling(),
                            prev: NonNull::dangling(),
                        },
                    },
                })))
            };
            unsafe { p.as_mut().links.value = ValueLinks { next: p, prev: p } };
            *head = Some(p);
        }
    }
}
#[inline]
unsafe fn attach_before<K, V>(mut to_attach: NonNull<Node<K, V>>, mut node: NonNull<Node<K, V>>) {
    unsafe {
        unsafe {
            to_attach.as_mut().links.value = ValueLinks {
                prev: node.as_ref().links.value.prev,
                next: node,
            };
            node.as_mut().links.value.prev = to_attach;
            (*to_attach.as_mut().links.value.prev.as_ptr())
                .links
                .value
                .next = to_attach;
        }
    }
}
#[inline]
unsafe fn detach_node<K, V>(mut node: NonNull<Node<K, V>>) {
    unsafe {
        unsafe {
            node.as_mut().links.value.prev.as_mut().links.value.next =
                node.as_ref().links.value.next;
            node.as_mut().links.value.next.as_mut().links.value.prev =
                node.as_ref().links.value.prev;
        }
    }
}
#[inline]
unsafe fn push_free<K, V>(
    free_list: &mut Option<NonNull<Node<K, V>>>,
    mut node: NonNull<Node<K, V>>,
) {
    unsafe {
        unsafe { node.as_mut().links.free.next = *free_list };
        *free_list = Some(node);
    }
}
#[inline]
unsafe fn pop_free<K, V>(
    free_list: &mut Option<NonNull<Node<K, V>>>,
) -> Option<NonNull<Node<K, V>>> {
    unsafe {
        if let Some(free) = *free_list {
            *free_list = unsafe { free.as_ref().links.free.next };
            Some(free)
        } else {
            None
        }
    }
}
#[inline]
unsafe fn allocate_node<K, V>(free_list: &mut Option<NonNull<Node<K, V>>>) -> NonNull<Node<K, V>> {
    unsafe {
        if let Some(mut free) = unsafe { pop_free(free_list) } {
            unsafe {
                free.as_mut().links.value = ValueLinks {
                    next: NonNull::dangling(),
                    prev: NonNull::dangling(),
                };
            }
            free
        } else {
            unsafe {
                NonNull::new_unchecked(Box::into_raw(Box::new(Node {
                    entry: MaybeUninit::uninit(),
                    links: Links {
                        value: ValueLinks {
                            next: NonNull::dangling(),
                            prev: NonNull::dangling(),
                        },
                    },
                })))
            }
        }
    }
}
#[inline]
unsafe fn drop_value_nodes<K, V>(mut guard: NonNull<Node<K, V>>) {
    unsafe {
        let cur = unsafe { guard.as_ref().links.value.prev };
        unsafe {
            guard.as_mut().links.value = ValueLinks {
                prev: guard,
                next: guard,
            };
        }
        struct Remainder<K, V> {
            cur: NonNull<Node<K, V>>,
            guard: NonNull<Node<K, V>>,
        }
        impl<K, V> Drop for Remainder<K, V> {
            fn drop(&mut self) {
                while self.cur != self.guard {
                    unsafe {
                        let prev = self.cur.as_ref().links.value.prev;
                        let _ = self.cur.as_mut().take_entry();
                        let _ = Box::from_raw(self.cur.as_ptr());
                        self.cur = prev;
                    }
                }
            }
        }
        let mut rem = Remainder { cur, guard };
        while rem.cur != guard {
            let prev = unsafe { rem.cur.as_ref().links.value.prev };
            let entry = unsafe { rem.cur.as_mut().take_entry() };
            let _ = unsafe { Box::from_raw(rem.cur.as_ptr()) };
            rem.cur = prev;
            drop(entry);
        }
    }
}
#[inline]
unsafe fn drop_free_nodes<K, V>(mut free: Option<NonNull<Node<K, V>>>) {
    unsafe {
        while let Some(some_free) = free {
            let next_free = unsafe { some_free.as_ref().links.free.next };
            let _ = unsafe { Box::from_raw(some_free.as_ptr()) };
            free = next_free;
        }
    }
}
#[inline]
unsafe fn remove_node<K, V>(
    free_list: &mut Option<NonNull<Node<K, V>>>,
    mut node: NonNull<Node<K, V>>,
) -> (K, V) {
    unsafe {
        unsafe {
            detach_node(node);
            push_free(free_list, node);
            node.as_mut().take_entry()
        }
    }
}
#[inline]
unsafe fn hash_node<S, K, V>(s: &S, node: NonNull<Node<K, V>>) -> u64
where
    S: BuildHasher,
    K: Hash,
{
    unsafe { hash_key(s, unsafe { node.as_ref().key_ref() }) }
}
#[inline]
fn hash_key<S, Q>(s: &S, k: &Q) -> u64
where
    S: BuildHasher,
    Q: Hash + ?Sized,
{
    s.hash_one(k)
}
struct DropFilteredValues<'a, K, V> {
    free: &'a mut Option<NonNull<Node<K, V>>>,
    cur_free: Option<NonNull<Node<K, V>>>,
}
impl<K, V> DropFilteredValues<'_, K, V> {
    #[inline]
    fn drop_later(&mut self, node: NonNull<Node<K, V>>) {
        unsafe {
            detach_node(node);
            push_free(&mut self.cur_free, node);
        }
    }
}
impl<K, V> Drop for DropFilteredValues<'_, K, V> {
    fn drop(&mut self) {
        unsafe {
            let end_free = self.cur_free;
            while self.cur_free != *self.free {
                let cur_free = self.cur_free.as_ptr();
                (*cur_free).take_entry();
                self.cur_free = (*cur_free).links.free.next;
            }
            *self.free = end_free;
        }
    }
}
