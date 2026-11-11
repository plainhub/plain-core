use crate::regex_engine::automata::{
    dfa::{DEAD, automaton::Automaton, dense},
    util::{
        alphabet,
        primitives::{PatternID, StateID},
    },
};
use core::{cell::RefCell, fmt, mem};
use std::{collections::BTreeMap, rc::Rc, vec, vec::Vec};
pub(crate) struct Minimizer<'a> {
    dfa: &'a mut dense::OwnedDFA,
    in_transitions: Vec<Vec<Vec<StateID>>>,
    partitions: Vec<StateSet>,
    waiting: Vec<StateSet>,
}
impl<'a> fmt::Debug for Minimizer<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Minimizer")
            .field("dfa", &self.dfa)
            .field("in_transitions", &self.in_transitions)
            .field("partitions", &self.partitions)
            .field("waiting", &self.waiting)
            .finish()
    }
}
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct StateSet {
    ids: Rc<RefCell<Vec<StateID>>>,
}
impl<'a> Minimizer<'a> {
    pub fn new(dfa: &'a mut dense::OwnedDFA) -> Minimizer<'a> {
        let in_transitions = Minimizer::incoming_transitions(dfa);
        let partitions = Minimizer::initial_partitions(dfa);
        let waiting = partitions.clone();
        Minimizer {
            dfa,
            in_transitions,
            partitions,
            waiting,
        }
    }
    pub fn run(mut self) {
        let stride2 = self.dfa.stride2();
        let as_state_id = |index: usize| -> StateID { StateID::new(index << stride2).unwrap() };
        let as_index = |id: StateID| -> usize { id.as_usize() >> stride2 };
        let mut incoming = StateSet::empty();
        let mut scratch1 = StateSet::empty();
        let mut scratch2 = StateSet::empty();
        let mut newparts = vec![];
        while let Some(set) = self.waiting.pop() {
            for b in self.dfa.byte_classes().iter() {
                self.find_incoming_to(b, &set, &mut incoming);
                if incoming.is_empty() {
                    continue;
                }
                for p in 0..self.partitions.len() {
                    self.partitions[p].intersection(&incoming, &mut scratch1);
                    if scratch1.is_empty() {
                        newparts.push(self.partitions[p].clone());
                        continue;
                    }
                    self.partitions[p].subtract(&incoming, &mut scratch2);
                    if scratch2.is_empty() {
                        newparts.push(self.partitions[p].clone());
                        continue;
                    }
                    let (x, y) = (scratch1.deep_clone(), scratch2.deep_clone());
                    newparts.push(x.clone());
                    newparts.push(y.clone());
                    match self.find_waiting(&self.partitions[p]) {
                        Some(i) => {
                            self.waiting[i] = x;
                            self.waiting.push(y);
                        }
                        None => {
                            if x.len() <= y.len() {
                                self.waiting.push(x);
                            } else {
                                self.waiting.push(y);
                            }
                        }
                    }
                }
                newparts = mem::replace(&mut self.partitions, newparts);
                newparts.clear();
            }
        }
        let mut state_to_part = vec![DEAD; self.dfa.state_len()];
        for p in &self.partitions {
            p.iter(|id| state_to_part[as_index(id)] = p.min());
        }
        let mut minimal_ids = vec![DEAD; self.dfa.state_len()];
        let mut new_index = 0;
        for state in self.dfa.states() {
            if state_to_part[as_index(state.id())] == state.id() {
                minimal_ids[as_index(state.id())] = as_state_id(new_index);
                new_index += 1;
            }
        }
        let minimal_count = new_index;
        let remap = |old| minimal_ids[as_index(state_to_part[as_index(old)])];
        for id in (0..self.dfa.state_len()).map(as_state_id) {
            if state_to_part[as_index(id)] != id {
                continue;
            }
            self.dfa.remap_state(id, remap);
            self.dfa.swap_states(id, minimal_ids[as_index(id)]);
        }
        self.dfa.truncate_states(minimal_count);
        let starts: Vec<_> = self.dfa.starts().collect();
        for (old_start_id, anchored, start_type) in starts {
            self.dfa
                .set_start_state(anchored, start_type, remap(old_start_id));
        }
        let mut pmap = BTreeMap::new();
        for (match_id, pattern_ids) in self.dfa.pattern_map() {
            let new_id = remap(match_id);
            pmap.insert(new_id, pattern_ids);
        }
        self.dfa.set_pattern_map(&pmap).unwrap();
        let old = self.dfa.special().clone();
        let new = self.dfa.special_mut();
        if old.matches() {
            new.min_match = StateID::MAX;
            new.max_match = StateID::ZERO;
            for i in as_index(old.min_match)..=as_index(old.max_match) {
                let new_id = remap(as_state_id(i));
                if new_id < new.min_match {
                    new.min_match = new_id;
                }
                if new_id > new.max_match {
                    new.max_match = new_id;
                }
            }
        }
        if old.starts() {
            new.min_start = StateID::MAX;
            new.max_start = StateID::ZERO;
            for i in as_index(old.min_start)..=as_index(old.max_start) {
                let new_id = remap(as_state_id(i));
                if new_id == DEAD {
                    continue;
                }
                if new_id < new.min_start {
                    new.min_start = new_id;
                }
                if new_id > new.max_start {
                    new.max_start = new_id;
                }
            }
            if new.max_start == DEAD {
                new.min_start = DEAD;
            }
        }
        new.quit_id = remap(new.quit_id);
        new.set_max();
    }
    fn find_waiting(&self, set: &StateSet) -> Option<usize> {
        self.waiting.iter().position(|s| s == set)
    }
    fn find_incoming_to(&self, b: alphabet::Unit, set: &StateSet, incoming: &mut StateSet) {
        incoming.clear();
        set.iter(|id| {
            for &inid in &self.in_transitions[self.dfa.to_index(id)][b.as_usize()] {
                incoming.add(inid);
            }
        });
        incoming.canonicalize();
    }
    fn initial_partitions(dfa: &dense::OwnedDFA) -> Vec<StateSet> {
        let mut matching: BTreeMap<Vec<PatternID>, StateSet> = BTreeMap::new();
        let mut is_quit = StateSet::empty();
        let mut no_match = StateSet::empty();
        for state in dfa.states() {
            if dfa.is_match_state(state.id()) {
                let mut pids = vec![];
                for i in 0..dfa.match_len(state.id()) {
                    pids.push(dfa.match_pattern(state.id(), i));
                }
                matching
                    .entry(pids)
                    .or_insert(StateSet::empty())
                    .add(state.id());
            } else if dfa.is_quit_state(state.id()) {
                is_quit.add(state.id());
            } else {
                no_match.add(state.id());
            }
        }
        let mut sets: Vec<StateSet> = matching.into_iter().map(|(_, set)| set).collect();
        sets.push(no_match);
        sets.push(is_quit);
        sets
    }
    fn incoming_transitions(dfa: &dense::OwnedDFA) -> Vec<Vec<Vec<StateID>>> {
        let mut incoming = vec![];
        for _ in dfa.states() {
            incoming.push(vec![vec![]; dfa.alphabet_len()]);
        }
        for state in dfa.states() {
            for (b, next) in state.transitions() {
                incoming[dfa.to_index(next)][b.as_usize()].push(state.id());
            }
        }
        incoming
    }
}
impl StateSet {
    fn empty() -> StateSet {
        StateSet {
            ids: Rc::new(RefCell::new(vec![])),
        }
    }
    fn add(&mut self, id: StateID) {
        self.ids.borrow_mut().push(id);
    }
    fn min(&self) -> StateID {
        self.ids.borrow()[0]
    }
    fn canonicalize(&mut self) {
        self.ids.borrow_mut().sort();
        self.ids.borrow_mut().dedup();
    }
    fn clear(&mut self) {
        self.ids.borrow_mut().clear();
    }
    fn len(&self) -> usize {
        self.ids.borrow().len()
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn deep_clone(&self) -> StateSet {
        let ids = self.ids.borrow().iter().cloned().collect();
        StateSet {
            ids: Rc::new(RefCell::new(ids)),
        }
    }
    fn iter<F: FnMut(StateID)>(&self, mut f: F) {
        for &id in self.ids.borrow().iter() {
            f(id);
        }
    }
    fn intersection(&self, other: &StateSet, dest: &mut StateSet) {
        dest.clear();
        if self.is_empty() || other.is_empty() {
            return;
        }
        let (seta, setb) = (self.ids.borrow(), other.ids.borrow());
        let (mut ita, mut itb) = (seta.iter().cloned(), setb.iter().cloned());
        let (mut a, mut b) = (ita.next().unwrap(), itb.next().unwrap());
        loop {
            if a == b {
                dest.add(a);
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
                b = match itb.next() {
                    None => break,
                    Some(b) => b,
                };
            } else if a < b {
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
            } else {
                b = match itb.next() {
                    None => break,
                    Some(b) => b,
                };
            }
        }
    }
    fn subtract(&self, other: &StateSet, dest: &mut StateSet) {
        dest.clear();
        if self.is_empty() || other.is_empty() {
            self.iter(|s| dest.add(s));
            return;
        }
        let (seta, setb) = (self.ids.borrow(), other.ids.borrow());
        let (mut ita, mut itb) = (seta.iter().cloned(), setb.iter().cloned());
        let (mut a, mut b) = (ita.next().unwrap(), itb.next().unwrap());
        loop {
            if a == b {
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
                b = match itb.next() {
                    None => {
                        dest.add(a);
                        break;
                    }
                    Some(b) => b,
                };
            } else if a < b {
                dest.add(a);
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
            } else {
                b = match itb.next() {
                    None => {
                        dest.add(a);
                        break;
                    }
                    Some(b) => b,
                };
            }
        }
        for a in ita {
            dest.add(a);
        }
    }
}
