use super::{
    Bucket, Entry, Equivalent, IndexMap, IndexedEntry, IterMut2, OccupiedEntry, VacantEntry,
};
use core::hash::{BuildHasher, Hash};
pub trait MutableKeys: Sealed {
    type Key;
    type Value;
    fn get_full_mut2<Q>(&mut self, key: &Q) -> Option<(usize, &mut Self::Key, &mut Self::Value)>
    where
        Q: ?Sized + Hash + Equivalent<Self::Key>;
    fn get_index_mut2(&mut self, index: usize) -> Option<(&mut Self::Key, &mut Self::Value)>;
    fn iter_mut2(&mut self) -> IterMut2<'_, Self::Key, Self::Value>;
    fn retain2<F>(&mut self, keep: F)
    where
        F: FnMut(&mut Self::Key, &mut Self::Value) -> bool;
}
impl<K, V, S> MutableKeys for IndexMap<K, V, S>
where
    S: BuildHasher,
{
    type Key = K;
    type Value = V;
    fn get_full_mut2<Q>(&mut self, key: &Q) -> Option<(usize, &mut K, &mut V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &mut self.as_entries_mut()[i];
            Some((i, &mut entry.key, &mut entry.value))
        } else {
            None
        }
    }
    fn get_index_mut2(&mut self, index: usize) -> Option<(&mut K, &mut V)> {
        self.as_entries_mut().get_mut(index).map(Bucket::muts)
    }
    fn iter_mut2(&mut self) -> IterMut2<'_, Self::Key, Self::Value> {
        IterMut2::new(self.as_entries_mut())
    }
    fn retain2<F>(&mut self, keep: F)
    where
        F: FnMut(&mut K, &mut V) -> bool,
    {
        self.core.retain_in_order(keep);
    }
}
pub trait MutableEntryKey: Sealed {
    type Key;
    fn key_mut(&mut self) -> &mut Self::Key;
}
impl<K, V> MutableEntryKey for Entry<'_, K, V> {
    type Key = K;
    fn key_mut(&mut self) -> &mut Self::Key {
        match self {
            Entry::Occupied(e) => e.key_mut(),
            Entry::Vacant(e) => e.key_mut(),
        }
    }
}
impl<K, V> MutableEntryKey for OccupiedEntry<'_, K, V> {
    type Key = K;
    fn key_mut(&mut self) -> &mut Self::Key {
        &mut self.get_bucket_mut().key
    }
}
impl<K, V> MutableEntryKey for VacantEntry<'_, K, V> {
    type Key = K;
    fn key_mut(&mut self) -> &mut Self::Key {
        self.key_mut()
    }
}
impl<K, V> MutableEntryKey for IndexedEntry<'_, K, V> {
    type Key = K;
    fn key_mut(&mut self) -> &mut Self::Key {
        self.key_mut()
    }
}
trait Sealed {}
impl<K, V, S> Sealed for IndexMap<K, V, S> {}
impl<K, V> Sealed for Entry<'_, K, V> {}
impl<K, V> Sealed for OccupiedEntry<'_, K, V> {}
impl<K, V> Sealed for VacantEntry<'_, K, V> {}
impl<K, V> Sealed for IndexedEntry<'_, K, V> {}
