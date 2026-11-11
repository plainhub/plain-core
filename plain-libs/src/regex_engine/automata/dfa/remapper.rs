use crate::regex_engine::automata::util::primitives::StateID;
use std::vec::Vec;
pub(super) trait Remappable: core::fmt::Debug {
    fn state_len(&self) -> usize;
    fn stride2(&self) -> usize;
    fn swap_states(&mut self, id1: StateID, id2: StateID);
    fn remap(&mut self, map: impl Fn(StateID) -> StateID);
}
#[derive(Debug)]
pub(super) struct Remapper {
    map: Vec<StateID>,
    idxmap: IndexMapper,
}
impl Remapper {
    pub(super) fn new(r: &impl Remappable) -> Remapper {
        let idxmap = IndexMapper {
            stride2: r.stride2(),
        };
        let map = (0..r.state_len()).map(|i| idxmap.to_state_id(i)).collect();
        Remapper { map, idxmap }
    }
    pub(super) fn swap(&mut self, r: &mut impl Remappable, id1: StateID, id2: StateID) {
        if id1 == id2 {
            return;
        }
        r.swap_states(id1, id2);
        self.map
            .swap(self.idxmap.to_index(id1), self.idxmap.to_index(id2));
    }
    pub(super) fn remap(mut self, r: &mut impl Remappable) {
        let oldmap = self.map.clone();
        for i in 0..r.state_len() {
            let cur_id = self.idxmap.to_state_id(i);
            let mut new_id = oldmap[i];
            if cur_id == new_id {
                continue;
            }
            loop {
                let id = oldmap[self.idxmap.to_index(new_id)];
                if cur_id == id {
                    self.map[i] = new_id;
                    break;
                }
                new_id = id;
            }
        }
        r.remap(|next| self.map[self.idxmap.to_index(next)]);
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
mod dense {
    use super::Remappable;
    use crate::regex_engine::automata::{dfa::dense::OwnedDFA, util::primitives::StateID};
    impl Remappable for OwnedDFA {
        fn state_len(&self) -> usize {
            OwnedDFA::state_len(self)
        }
        fn stride2(&self) -> usize {
            OwnedDFA::stride2(self)
        }
        fn swap_states(&mut self, id1: StateID, id2: StateID) {
            OwnedDFA::swap_states(self, id1, id2)
        }
        fn remap(&mut self, map: impl Fn(StateID) -> StateID) {
            OwnedDFA::remap(self, map)
        }
    }
}
mod onepass {
    use super::Remappable;
    use crate::regex_engine::automata::{dfa::onepass::DFA, util::primitives::StateID};
    impl Remappable for DFA {
        fn state_len(&self) -> usize {
            DFA::state_len(self)
        }
        fn stride2(&self) -> usize {
            0
        }
        fn swap_states(&mut self, id1: StateID, id2: StateID) {
            DFA::swap_states(self, id1, id2)
        }
        fn remap(&mut self, map: impl Fn(StateID) -> StateID) {
            DFA::remap(self, map)
        }
    }
}
