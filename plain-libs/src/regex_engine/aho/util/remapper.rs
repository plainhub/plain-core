use crate::regex_engine::aho::{nfa::noncontiguous, util::primitives::StateID};
use std::vec::Vec;
pub(crate) trait Remappable: core::fmt::Debug {
    fn state_len(&self) -> usize;
    fn swap_states(&mut self, id1: StateID, id2: StateID);
    fn remap(&mut self, map: impl Fn(StateID) -> StateID);
}
#[derive(Debug)]
pub(crate) struct Remapper {
    map: Vec<StateID>,
    idx: IndexMapper,
}
impl Remapper {
    pub(crate) fn new(r: &impl Remappable, stride2: usize) -> Remapper {
        let idx = IndexMapper { stride2 };
        let map = (0..r.state_len()).map(|i| idx.to_state_id(i)).collect();
        Remapper { map, idx }
    }
    pub(crate) fn swap(&mut self, r: &mut impl Remappable, id1: StateID, id2: StateID) {
        if id1 == id2 {
            return;
        }
        r.swap_states(id1, id2);
        self.map
            .swap(self.idx.to_index(id1), self.idx.to_index(id2));
    }
    pub(crate) fn remap(mut self, r: &mut impl Remappable) {
        let oldmap = self.map.clone();
        for i in 0..r.state_len() {
            let cur_id = self.idx.to_state_id(i);
            let mut new_id = oldmap[i];
            if cur_id == new_id {
                continue;
            }
            loop {
                let id = oldmap[self.idx.to_index(new_id)];
                if cur_id == id {
                    self.map[i] = new_id;
                    break;
                }
                new_id = id;
            }
        }
        r.remap(|sid| self.map[self.idx.to_index(sid)]);
    }
}
#[derive(Debug)]
struct IndexMapper {
    stride2: usize,
}
impl IndexMapper {
    fn to_index(&self, id: StateID) -> usize {
        id.as_usize() >> self.stride2
    }
    fn to_state_id(&self, index: usize) -> StateID {
        StateID::new_unchecked(index << self.stride2)
    }
}
impl Remappable for noncontiguous::NFA {
    fn state_len(&self) -> usize {
        noncontiguous::NFA::states(self).len()
    }
    fn swap_states(&mut self, id1: StateID, id2: StateID) {
        noncontiguous::NFA::swap_states(self, id1, id2)
    }
    fn remap(&mut self, map: impl Fn(StateID) -> StateID) {
        noncontiguous::NFA::remap(self, map)
    }
}
