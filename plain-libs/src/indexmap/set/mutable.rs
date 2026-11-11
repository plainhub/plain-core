use super::{Equivalent, IndexSet};
use crate::indexmap::map::MutableKeys;
use core::hash::{BuildHasher, Hash};
pub trait MutableValues: Sealed {
    type Value;
    fn get_full_mut2<Q>(&mut self, value: &Q) -> Option<(usize, &mut Self::Value)>
    where
        Q: ?Sized + Hash + Equivalent<Self::Value>;
    fn get_index_mut2(&mut self, index: usize) -> Option<&mut Self::Value>;
    fn retain2<F>(&mut self, keep: F)
    where
        F: FnMut(&mut Self::Value) -> bool;
}
impl<T, S> MutableValues for IndexSet<T, S>
where
    S: BuildHasher,
{
    type Value = T;
    fn get_full_mut2<Q>(&mut self, value: &Q) -> Option<(usize, &mut T)>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        match self.map.get_full_mut2(value) {
            Some((index, value, ())) => Some((index, value)),
            None => None,
        }
    }
    fn get_index_mut2(&mut self, index: usize) -> Option<&mut T> {
        match self.map.get_index_mut2(index) {
            Some((value, ())) => Some(value),
            None => None,
        }
    }
    fn retain2<F>(&mut self, mut keep: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        self.map.retain2(move |value, ()| keep(value));
    }
}
trait Sealed {}
impl<T, S> Sealed for IndexSet<T, S> {}
