use crate::regex_engine::automata::{
    nfa::thompson::{
        error::BuildError,
        nfa::{self, NFA, SparseTransitions, Transition},
    },
    util::{
        look::{Look, LookMatcher},
        primitives::{IteratorIndexExt, PatternID, SmallIndex, StateID},
    },
};
use core::mem;
use std::{sync::Arc, vec, vec::Vec};
#[derive(Clone, Debug, Eq, PartialEq)]
enum State {
    Empty {
        next: StateID,
    },
    ByteRange {
        trans: Transition,
    },
    Sparse {
        transitions: Vec<Transition>,
    },
    Look {
        look: Look,
        next: StateID,
    },
    CaptureStart {
        pattern_id: PatternID,
        group_index: SmallIndex,
        next: StateID,
    },
    CaptureEnd {
        pattern_id: PatternID,
        group_index: SmallIndex,
        next: StateID,
    },
    Union {
        alternates: Vec<StateID>,
    },
    UnionReverse {
        alternates: Vec<StateID>,
    },
    Fail,
    Match {
        pattern_id: PatternID,
    },
}
impl State {
    fn goto(&self) -> Option<StateID> {
        match *self {
            State::Empty { next } => Some(next),
            State::Union { ref alternates } if alternates.len() == 1 => Some(alternates[0]),
            State::UnionReverse { ref alternates } if alternates.len() == 1 => Some(alternates[0]),
            _ => None,
        }
    }
    fn memory_usage(&self) -> usize {
        match *self {
            State::Empty { .. }
            | State::ByteRange { .. }
            | State::Look { .. }
            | State::CaptureStart { .. }
            | State::CaptureEnd { .. }
            | State::Fail
            | State::Match { .. } => 0,
            State::Sparse { ref transitions } => transitions.len() * mem::size_of::<Transition>(),
            State::Union { ref alternates } => alternates.len() * mem::size_of::<StateID>(),
            State::UnionReverse { ref alternates } => alternates.len() * mem::size_of::<StateID>(),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Builder {
    pattern_id: Option<PatternID>,
    states: Vec<State>,
    start_pattern: Vec<StateID>,
    captures: Vec<Vec<Option<Arc<str>>>>,
    memory_states: usize,
    utf8: bool,
    reverse: bool,
    look_matcher: LookMatcher,
    size_limit: Option<usize>,
}
impl Builder {
    pub fn new() -> Builder {
        Builder::default()
    }
    pub fn clear(&mut self) {
        self.pattern_id = None;
        self.states.clear();
        self.start_pattern.clear();
        self.captures.clear();
        self.memory_states = 0;
    }
    pub fn build(
        &self,
        start_anchored: StateID,
        start_unanchored: StateID,
    ) -> Result<NFA, BuildError> {
        assert!(
            self.pattern_id.is_none(),
            "must call 'finish_pattern' first"
        );
        debug!(
            "intermediate NFA compilation via builder is complete, \
             intermediate NFA size: {} states, {} bytes on heap",
            self.states.len(),
            self.memory_usage(),
        );
        let mut nfa = nfa::Inner::default();
        nfa.set_utf8(self.utf8);
        nfa.set_reverse(self.reverse);
        nfa.set_look_matcher(self.look_matcher.clone());
        let mut empties = vec![];
        let mut remap = vec![];
        remap.resize(self.states.len(), StateID::ZERO);
        nfa.set_starts(start_anchored, start_unanchored, &self.start_pattern);
        nfa.set_captures(&self.captures)
            .map_err(BuildError::captures)?;
        for (sid, state) in self.states.iter().with_state_ids() {
            match *state {
                State::Empty { next } => {
                    empties.push((sid, next));
                }
                State::ByteRange { trans } => {
                    remap[sid] = nfa.add(nfa::State::ByteRange { trans });
                }
                State::Sparse { ref transitions } => {
                    remap[sid] = match transitions.len() {
                        0 => nfa.add(nfa::State::Fail),
                        1 => nfa.add(nfa::State::ByteRange {
                            trans: transitions[0],
                        }),
                        _ => {
                            let transitions = transitions.to_vec().into_boxed_slice();
                            let sparse = SparseTransitions { transitions };
                            nfa.add(nfa::State::Sparse(sparse))
                        }
                    }
                }
                State::Look { look, next } => {
                    remap[sid] = nfa.add(nfa::State::Look { look, next });
                }
                State::CaptureStart {
                    pattern_id,
                    group_index,
                    next,
                } => {
                    let slot = nfa
                        .group_info()
                        .slot(pattern_id, group_index.as_usize())
                        .expect("invalid capture index");
                    let slot = SmallIndex::new(slot).expect("a small enough slot");
                    remap[sid] = nfa.add(nfa::State::Capture {
                        next,
                        pattern_id,
                        group_index,
                        slot,
                    });
                }
                State::CaptureEnd {
                    pattern_id,
                    group_index,
                    next,
                } => {
                    let slot = nfa
                        .group_info()
                        .slot(pattern_id, group_index.as_usize())
                        .expect("invalid capture index")
                        .checked_add(1)
                        .unwrap();
                    let slot = SmallIndex::new(slot).expect("a small enough slot");
                    remap[sid] = nfa.add(nfa::State::Capture {
                        next,
                        pattern_id,
                        group_index,
                        slot,
                    });
                }
                State::Union { ref alternates } => {
                    if alternates.is_empty() {
                        remap[sid] = nfa.add(nfa::State::Fail);
                    } else if alternates.len() == 1 {
                        empties.push((sid, alternates[0]));
                        remap[sid] = alternates[0];
                    } else if alternates.len() == 2 {
                        remap[sid] = nfa.add(nfa::State::BinaryUnion {
                            alt1: alternates[0],
                            alt2: alternates[1],
                        });
                    } else {
                        let alternates = alternates.to_vec().into_boxed_slice();
                        remap[sid] = nfa.add(nfa::State::Union { alternates });
                    }
                }
                State::UnionReverse { ref alternates } => {
                    if alternates.is_empty() {
                        remap[sid] = nfa.add(nfa::State::Fail);
                    } else if alternates.len() == 1 {
                        empties.push((sid, alternates[0]));
                        remap[sid] = alternates[0];
                    } else if alternates.len() == 2 {
                        remap[sid] = nfa.add(nfa::State::BinaryUnion {
                            alt1: alternates[1],
                            alt2: alternates[0],
                        });
                    } else {
                        let mut alternates = alternates.to_vec().into_boxed_slice();
                        alternates.reverse();
                        remap[sid] = nfa.add(nfa::State::Union { alternates });
                    }
                }
                State::Fail => {
                    remap[sid] = nfa.add(nfa::State::Fail);
                }
                State::Match { pattern_id } => {
                    remap[sid] = nfa.add(nfa::State::Match { pattern_id });
                }
            }
        }
        let mut remapped = vec![false; self.states.len()];
        for &(empty_id, empty_next) in empties.iter() {
            if remapped[empty_id] {
                continue;
            }
            let mut new_next = empty_next;
            while let Some(next) = self.states[new_next].goto() {
                new_next = next;
            }
            remap[empty_id] = remap[new_next];
            remapped[empty_id] = true;
            let mut next2 = empty_next;
            while let Some(next) = self.states[next2].goto() {
                remap[next2] = remap[new_next];
                remapped[next2] = true;
                next2 = next;
            }
        }
        nfa.remap(&remap);
        let final_nfa = nfa.into_nfa();
        debug!(
            "NFA compilation via builder complete, \
             final NFA size: {} states, {} bytes on heap, \
             has empty? {:?}, utf8? {:?}",
            final_nfa.states().len(),
            final_nfa.memory_usage(),
            final_nfa.has_empty(),
            final_nfa.is_utf8(),
        );
        Ok(final_nfa)
    }
    pub fn start_pattern(&mut self) -> Result<PatternID, BuildError> {
        assert!(
            self.pattern_id.is_none(),
            "must call 'finish_pattern' first"
        );
        let proposed = self.start_pattern.len();
        let pid = PatternID::new(proposed).map_err(|_| BuildError::too_many_patterns(proposed))?;
        self.pattern_id = Some(pid);
        self.start_pattern.push(StateID::ZERO);
        Ok(pid)
    }
    pub fn finish_pattern(&mut self, start_id: StateID) -> Result<PatternID, BuildError> {
        let pid = self.current_pattern_id();
        self.start_pattern[pid] = start_id;
        self.pattern_id = None;
        Ok(pid)
    }
    pub fn current_pattern_id(&self) -> PatternID {
        self.pattern_id.expect("must call 'start_pattern' first")
    }
    pub fn pattern_len(&self) -> usize {
        self.start_pattern.len()
    }
    pub fn add_empty(&mut self) -> Result<StateID, BuildError> {
        self.add(State::Empty {
            next: StateID::ZERO,
        })
    }
    pub fn add_union(&mut self, alternates: Vec<StateID>) -> Result<StateID, BuildError> {
        self.add(State::Union { alternates })
    }
    pub fn add_union_reverse(&mut self, alternates: Vec<StateID>) -> Result<StateID, BuildError> {
        self.add(State::UnionReverse { alternates })
    }
    pub fn add_range(&mut self, trans: Transition) -> Result<StateID, BuildError> {
        self.add(State::ByteRange { trans })
    }
    pub fn add_sparse(&mut self, transitions: Vec<Transition>) -> Result<StateID, BuildError> {
        self.add(State::Sparse { transitions })
    }
    pub fn add_look(&mut self, next: StateID, look: Look) -> Result<StateID, BuildError> {
        self.add(State::Look { look, next })
    }
    pub fn add_capture_start(
        &mut self,
        next: StateID,
        group_index: u32,
        name: Option<Arc<str>>,
    ) -> Result<StateID, BuildError> {
        let pid = self.current_pattern_id();
        let group_index = match SmallIndex::try_from(group_index) {
            Err(_) => return Err(BuildError::invalid_capture_index(group_index)),
            Ok(group_index) => group_index,
        };
        if pid.as_usize() >= self.captures.len() {
            for _ in 0..=(pid.as_usize() - self.captures.len()) {
                self.captures.push(vec![]);
            }
        }
        if group_index.as_usize() >= self.captures[pid].len() {
            for _ in 0..(group_index.as_usize() - self.captures[pid].len()) {
                self.captures[pid].push(None);
            }
            self.captures[pid].push(name);
        }
        self.add(State::CaptureStart {
            pattern_id: pid,
            group_index,
            next,
        })
    }
    pub fn add_capture_end(
        &mut self,
        next: StateID,
        group_index: u32,
    ) -> Result<StateID, BuildError> {
        let pid = self.current_pattern_id();
        let group_index = match SmallIndex::try_from(group_index) {
            Err(_) => return Err(BuildError::invalid_capture_index(group_index)),
            Ok(group_index) => group_index,
        };
        self.add(State::CaptureEnd {
            pattern_id: pid,
            group_index,
            next,
        })
    }
    pub fn add_fail(&mut self) -> Result<StateID, BuildError> {
        self.add(State::Fail)
    }
    pub fn add_match(&mut self) -> Result<StateID, BuildError> {
        let pattern_id = self.current_pattern_id();
        let sid = self.add(State::Match { pattern_id })?;
        Ok(sid)
    }
    fn add(&mut self, state: State) -> Result<StateID, BuildError> {
        let id = StateID::new(self.states.len())
            .map_err(|_| BuildError::too_many_states(self.states.len()))?;
        self.memory_states += state.memory_usage();
        self.states.push(state);
        self.check_size_limit()?;
        Ok(id)
    }
    pub fn patch(&mut self, from: StateID, to: StateID) -> Result<(), BuildError> {
        let old_memory_states = self.memory_states;
        match self.states[from] {
            State::Empty { ref mut next } => {
                *next = to;
            }
            State::ByteRange { ref mut trans } => {
                trans.next = to;
            }
            State::Sparse { .. } => {
                panic!("cannot patch from a sparse NFA state")
            }
            State::Look { ref mut next, .. } => {
                *next = to;
            }
            State::Union { ref mut alternates } => {
                alternates.push(to);
                self.memory_states += mem::size_of::<StateID>();
            }
            State::UnionReverse { ref mut alternates } => {
                alternates.push(to);
                self.memory_states += mem::size_of::<StateID>();
            }
            State::CaptureStart { ref mut next, .. } => {
                *next = to;
            }
            State::CaptureEnd { ref mut next, .. } => {
                *next = to;
            }
            State::Fail => {}
            State::Match { .. } => {}
        }
        if old_memory_states != self.memory_states {
            self.check_size_limit()?;
        }
        Ok(())
    }
    pub fn set_utf8(&mut self, yes: bool) {
        self.utf8 = yes;
    }
    pub fn get_utf8(&self) -> bool {
        self.utf8
    }
    pub fn set_reverse(&mut self, yes: bool) {
        self.reverse = yes;
    }
    pub fn get_reverse(&self) -> bool {
        self.reverse
    }
    pub fn set_look_matcher(&mut self, m: LookMatcher) {
        self.look_matcher = m;
    }
    pub fn get_look_matcher(&self) -> &LookMatcher {
        &self.look_matcher
    }
    pub fn set_size_limit(&mut self, limit: Option<usize>) -> Result<(), BuildError> {
        self.size_limit = limit;
        self.check_size_limit()
    }
    pub fn get_size_limit(&self) -> Option<usize> {
        self.size_limit
    }
    pub fn memory_usage(&self) -> usize {
        self.states.len() * mem::size_of::<State>() + self.memory_states
    }
    fn check_size_limit(&self) -> Result<(), BuildError> {
        if let Some(limit) = self.size_limit {
            if self.memory_usage() > limit {
                return Err(BuildError::exceeded_size_limit(limit));
            }
        }
        Ok(())
    }
}
