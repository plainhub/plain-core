use crate::hashlink::DefaultHashBuilder;
use crate::hashlink::linked_hash_map::{self, LinkedHashMap};
pub use crate::hashlink::linked_hash_map::{
    Drain, Entry, IntoIter, Iter, IterMut, OccupiedEntry, RawEntryBuilder, RawEntryBuilderMut,
    RawOccupiedEntryMut, RawVacantEntryMut, VacantEntry,
};
use core::{
    borrow::Borrow,
    fmt,
    hash::{BuildHasher, Hash},
};
pub struct LruCache<K, V, S = DefaultHashBuilder> {
    map: LinkedHashMap<K, V, S>,
    max_size: usize,
}
impl<K: Eq + Hash, V> LruCache<K, V> {
    #[inline]
    pub fn new(capacity: usize) -> Self {
        LruCache {
            map: LinkedHashMap::new(),
            max_size: capacity,
        }
    }
}
impl<K, V, S> LruCache<K, V, S> {
    #[inline]
    pub fn with_hasher(capacity: usize, hash_builder: S) -> Self {
        LruCache {
            map: LinkedHashMap::with_hasher(hash_builder),
            max_size: capacity,
        }
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.max_size
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    #[inline]
    pub fn clear(&mut self) {
        self.map.clear();
    }
    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        self.map.iter()
    }
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        self.map.iter_mut()
    }
    #[inline]
    pub fn drain(&mut self) -> Drain<'_, K, V> {
        self.map.drain()
    }
    #[inline]
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        self.map.retain(f);
    }
}
impl<K: Eq + Hash, V, S> LruCache<K, V, S>
where
    S: BuildHasher,
{
    #[inline]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.contains_key(key)
    }
    #[inline]
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        let old_val = self.map.insert(k, v);
        if self.len() > self.capacity() {
            self.remove_lru();
        }
        old_val
    }
    #[inline]
    pub fn peek<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.get(k)
    }
    #[inline]
    pub fn peek_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.get_mut(k)
    }
    #[inline]
    pub fn get<Q>(&mut self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get_mut(k).map(|v| &*v)
    }
    #[inline]
    pub fn get_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match self.map.raw_entry_mut().from_key(k) {
            linked_hash_map::RawEntryMut::Occupied(mut occupied) => {
                occupied.to_back();
                Some(occupied.into_mut())
            }
            linked_hash_map::RawEntryMut::Vacant(_) => None,
        }
    }
    #[inline]
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V, S> {
        if self.len() > self.capacity() {
            self.remove_lru();
        }
        self.map.entry(key)
    }
    #[inline]
    pub fn raw_entry(&self) -> RawEntryBuilder<'_, K, V, S> {
        self.map.raw_entry()
    }
    #[inline]
    pub fn raw_entry_mut(&mut self) -> RawEntryBuilderMut<'_, K, V, S> {
        if self.len() > self.capacity() {
            self.remove_lru();
        }
        self.map.raw_entry_mut()
    }
    #[inline]
    pub fn remove<Q>(&mut self, k: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.remove(k)
    }
    #[inline]
    pub fn remove_entry<Q>(&mut self, k: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.remove_entry(k)
    }
    #[inline]
    pub fn set_capacity(&mut self, capacity: usize) {
        for _ in capacity..self.len() {
            self.remove_lru();
        }
        self.max_size = capacity;
    }
    #[inline]
    pub fn remove_lru(&mut self) -> Option<(K, V)> {
        self.map.pop_front()
    }
}
impl<K: Hash + Eq + Clone, V: Clone, S: BuildHasher + Clone> Clone for LruCache<K, V, S> {
    #[inline]
    fn clone(&self) -> Self {
        LruCache {
            map: self.map.clone(),
            max_size: self.max_size,
        }
    }
}
impl<K: Eq + Hash, V, S: BuildHasher> Extend<(K, V)> for LruCache<K, V, S> {
    #[inline]
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}
impl<K, V, S> IntoIterator for LruCache<K, V, S> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;
    #[inline]
    fn into_iter(self) -> IntoIter<K, V> {
        self.map.into_iter()
    }
}
impl<'a, K, V, S> IntoIterator for &'a LruCache<K, V, S> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    #[inline]
    fn into_iter(self) -> Iter<'a, K, V> {
        self.iter()
    }
}
impl<'a, K, V, S> IntoIterator for &'a mut LruCache<K, V, S> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;
    #[inline]
    fn into_iter(self) -> IterMut<'a, K, V> {
        self.iter_mut()
    }
}
impl<K, V, S> fmt::Debug for LruCache<K, V, S>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_map().entries(self.iter().rev()).finish()
    }
}
