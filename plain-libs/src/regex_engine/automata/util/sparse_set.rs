use crate::regex_engine::automata::util::primitives::StateID;
use std::{vec, vec::Vec};
#[derive(Clone, Debug)]
pub(crate) struct SparseSets {
    pub(crate) set1: SparseSet,
    pub(crate) set2: SparseSet,
}
impl SparseSets {
    pub(crate) fn new(capacity: usize) -> SparseSets {
        SparseSets {
            set1: SparseSet::new(capacity),
            set2: SparseSet::new(capacity),
        }
    }
    #[inline]
    pub(crate) fn resize(&mut self, new_capacity: usize) {
        self.set1.resize(new_capacity);
        self.set2.resize(new_capacity);
    }
    pub(crate) fn clear(&mut self) {
        self.set1.clear();
        self.set2.clear();
    }
    pub(crate) fn swap(&mut self) {
        core::mem::swap(&mut self.set1, &mut self.set2);
    }
    pub(crate) fn memory_usage(&self) -> usize {
        self.set1.memory_usage() + self.set2.memory_usage()
    }
}
#[derive(Clone)]
pub(crate) struct SparseSet {
    len: usize,
    dense: Vec<StateID>,
    sparse: Vec<StateID>,
}
impl SparseSet {
    #[inline]
    pub(crate) fn new(capacity: usize) -> SparseSet {
        let mut set = SparseSet {
            len: 0,
            dense: vec![],
            sparse: vec![],
        };
        set.resize(capacity);
        set
    }
    #[inline]
    pub(crate) fn resize(&mut self, new_capacity: usize) {
        assert!(
            new_capacity <= StateID::LIMIT,
            "sparse set capacity cannot exceed {:?}",
            StateID::LIMIT
        );
        self.clear();
        self.dense.resize(new_capacity, StateID::ZERO);
        self.sparse.resize(new_capacity, StateID::ZERO);
    }
    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        self.dense.len()
    }
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn insert(&mut self, id: StateID) -> bool {
        if self.contains(id) {
            return false;
        }
        let i = self.len();
        assert!(
            i < self.capacity(),
            "{:?} exceeds capacity of {:?} when inserting {:?}",
            i,
            self.capacity(),
            id,
        );
        let index = StateID::new_unchecked(i);
        self.dense[index] = id;
        self.sparse[id] = index;
        self.len += 1;
        true
    }
    #[inline]
    pub(crate) fn contains(&self, id: StateID) -> bool {
        let index = self.sparse[id];
        index.as_usize() < self.len() && self.dense[index] == id
    }
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.len = 0;
    }
    #[inline]
    pub(crate) fn iter(&self) -> SparseSetIter<'_> {
        SparseSetIter(self.dense[..self.len()].iter())
    }
    #[inline]
    pub(crate) fn memory_usage(&self) -> usize {
        self.dense.len() * StateID::SIZE + self.sparse.len() * StateID::SIZE
    }
}
impl core::fmt::Debug for SparseSet {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let elements: Vec<StateID> = self.iter().collect();
        f.debug_tuple("SparseSet").field(&elements).finish()
    }
}
#[derive(Debug)]
pub(crate) struct SparseSetIter<'a>(core::slice::Iter<'a, StateID>);
impl<'a> Iterator for SparseSetIter<'a> {
    type Item = StateID;
    #[cfg_attr(all(), inline(always))]
    fn next(&mut self) -> Option<StateID> {
        self.0.next().copied()
    }
}
