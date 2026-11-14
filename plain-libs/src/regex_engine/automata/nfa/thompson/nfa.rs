use crate::regex_engine::automata::nfa::thompson::{
    compiler::{Compiler, Config},
    error::BuildError,
};
use crate::regex_engine::automata::{
    nfa::thompson::builder::Builder,
    util::{
        alphabet::{self, ByteClassSet, ByteClasses},
        captures::{GroupInfo, GroupInfoError},
        look::{Look, LookMatcher, LookSet},
        primitives::{IteratorIndexExt, PatternID, PatternIDIter, SmallIndex, StateID},
        sparse_set::SparseSet,
    },
};
use core::{fmt, mem};
use std::{boxed::Box, format, string::String, sync::Arc, vec, vec::Vec};
#[derive(Clone)]
pub struct NFA(Arc<Inner>);
impl NFA {
    pub fn new(pattern: &str) -> Result<NFA, BuildError> {
        NFA::compiler().build(pattern)
    }
    pub fn new_many<P: AsRef<str>>(patterns: &[P]) -> Result<NFA, BuildError> {
        NFA::compiler().build_many(patterns)
    }
    pub fn always_match() -> NFA {
        let mut builder = Builder::new();
        let pid = builder.start_pattern().unwrap();
        assert_eq!(pid.as_usize(), 0);
        let start_id = builder.add_capture_start(StateID::ZERO, 0, None).unwrap();
        let end_id = builder.add_capture_end(StateID::ZERO, 0).unwrap();
        let match_id = builder.add_match().unwrap();
        builder.patch(start_id, end_id).unwrap();
        builder.patch(end_id, match_id).unwrap();
        let pid = builder.finish_pattern(start_id).unwrap();
        assert_eq!(pid.as_usize(), 0);
        builder.build(start_id, start_id).unwrap()
    }
    pub fn never_match() -> NFA {
        let mut builder = Builder::new();
        let sid = builder.add_fail().unwrap();
        builder.build(sid, sid).unwrap()
    }
    pub fn config() -> Config {
        Config::new()
    }
    pub fn compiler() -> Compiler {
        Compiler::new()
    }
    pub fn patterns(&self) -> PatternIter<'_> {
        PatternIter {
            it: PatternID::iter(self.pattern_len()),
            _marker: core::marker::PhantomData,
        }
    }
    #[inline]
    pub fn pattern_len(&self) -> usize {
        self.0.start_pattern.len()
    }
    #[inline]
    pub fn start_anchored(&self) -> StateID {
        self.0.start_anchored
    }
    #[inline]
    pub fn start_unanchored(&self) -> StateID {
        self.0.start_unanchored
    }
    #[inline]
    pub fn start_pattern(&self, pid: PatternID) -> Option<StateID> {
        self.0.start_pattern.get(pid.as_usize()).copied()
    }
    #[inline]
    pub(crate) fn byte_class_set(&self) -> &ByteClassSet {
        &self.0.byte_class_set
    }
    #[inline]
    pub fn byte_classes(&self) -> &ByteClasses {
        &self.0.byte_classes
    }
    #[inline]
    pub fn state(&self, id: StateID) -> &State {
        &self.states()[id]
    }
    #[inline]
    pub fn states(&self) -> &[State] {
        &self.0.states
    }
    #[inline]
    pub fn group_info(&self) -> &GroupInfo {
        &self.0.group_info()
    }
    #[inline]
    pub fn has_capture(&self) -> bool {
        self.0.has_capture
    }
    #[inline]
    pub fn has_empty(&self) -> bool {
        self.0.has_empty
    }
    #[inline]
    pub fn is_utf8(&self) -> bool {
        self.0.utf8
    }
    #[inline]
    pub fn is_reverse(&self) -> bool {
        self.0.reverse
    }
    #[inline]
    pub fn is_always_start_anchored(&self) -> bool {
        self.start_anchored() == self.start_unanchored()
    }
    #[inline]
    pub fn look_matcher(&self) -> &LookMatcher {
        &self.0.look_matcher
    }
    #[inline]
    pub fn look_set_any(&self) -> LookSet {
        self.0.look_set_any
    }
    #[inline]
    pub fn look_set_prefix_any(&self) -> LookSet {
        self.0.look_set_prefix_any
    }
    #[inline]
    pub fn memory_usage(&self) -> usize {
        use core::mem::size_of;
        size_of::<Inner>()
            + self.0.states.len() * size_of::<State>()
            + self.0.start_pattern.len() * size_of::<StateID>()
            + self.0.group_info.memory_usage()
            + self.0.memory_extra
    }
}
impl fmt::Debug for NFA {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
#[derive(Default)]
pub(super) struct Inner {
    states: Vec<State>,
    start_anchored: StateID,
    start_unanchored: StateID,
    start_pattern: Vec<StateID>,
    group_info: GroupInfo,
    byte_class_set: ByteClassSet,
    byte_classes: ByteClasses,
    has_capture: bool,
    has_empty: bool,
    utf8: bool,
    reverse: bool,
    look_matcher: LookMatcher,
    look_set_any: LookSet,
    look_set_prefix_any: LookSet,
    memory_extra: usize,
}
impl Inner {
    pub(super) fn into_nfa(mut self) -> NFA {
        self.byte_classes = self.byte_class_set.byte_classes();
        let mut stack = vec![];
        let mut seen = SparseSet::new(self.states.len());
        for &start_id in self.start_pattern.iter() {
            stack.push(start_id);
            seen.clear();
            let mut prefix_any = LookSet::empty();
            while let Some(sid) = stack.pop() {
                if !seen.insert(sid) {
                    continue;
                }
                match self.states[sid] {
                    State::ByteRange { .. } | State::Dense { .. } | State::Fail => continue,
                    State::Sparse(_) => {
                        continue;
                    }
                    State::Match { .. } => self.has_empty = true,
                    State::Look { look, next } => {
                        prefix_any = prefix_any.insert(look);
                        stack.push(next);
                    }
                    State::Union { ref alternates } => {
                        stack.extend(alternates.iter());
                    }
                    State::BinaryUnion { alt1, alt2 } => {
                        stack.push(alt2);
                        stack.push(alt1);
                    }
                    State::Capture { next, .. } => {
                        stack.push(next);
                    }
                }
            }
            self.look_set_prefix_any = self.look_set_prefix_any.union(prefix_any);
        }
        self.states.shrink_to_fit();
        self.start_pattern.shrink_to_fit();
        NFA(Arc::new(self))
    }
    pub(super) fn group_info(&self) -> &GroupInfo {
        &self.group_info
    }
    pub(super) fn add(&mut self, state: State) -> StateID {
        match state {
            State::ByteRange { ref trans } => {
                self.byte_class_set.set_range(trans.start, trans.end);
            }
            State::Sparse(ref sparse) => {
                for trans in sparse.transitions.iter() {
                    self.byte_class_set.set_range(trans.start, trans.end);
                }
            }
            State::Dense { .. } => unreachable!(),
            State::Look { look, .. } => {
                self.look_matcher
                    .add_to_byteset(look, &mut self.byte_class_set);
                self.look_set_any = self.look_set_any.insert(look);
            }
            State::Capture { .. } => {
                self.has_capture = true;
            }
            State::Union { .. } | State::BinaryUnion { .. } | State::Fail | State::Match { .. } => {
            }
        }
        let id = StateID::new(self.states.len()).unwrap();
        self.memory_extra += state.memory_usage();
        self.states.push(state);
        id
    }
    pub(super) fn set_starts(
        &mut self,
        start_anchored: StateID,
        start_unanchored: StateID,
        start_pattern: &[StateID],
    ) {
        self.start_anchored = start_anchored;
        self.start_unanchored = start_unanchored;
        self.start_pattern = start_pattern.to_vec();
    }
    pub(super) fn set_utf8(&mut self, yes: bool) {
        self.utf8 = yes;
    }
    pub(super) fn set_reverse(&mut self, yes: bool) {
        self.reverse = yes;
    }
    pub(super) fn set_look_matcher(&mut self, m: LookMatcher) {
        self.look_matcher = m;
    }
    pub(super) fn set_captures(
        &mut self,
        captures: &[Vec<Option<Arc<str>>>],
    ) -> Result<(), GroupInfoError> {
        self.group_info = GroupInfo::new(captures.iter().map(|x| x.iter().map(|y| y.as_ref())))?;
        Ok(())
    }
    pub(super) fn remap(&mut self, old_to_new: &[StateID]) {
        for state in &mut self.states {
            state.remap(old_to_new);
        }
        self.start_anchored = old_to_new[self.start_anchored];
        self.start_unanchored = old_to_new[self.start_unanchored];
        for id in self.start_pattern.iter_mut() {
            *id = old_to_new[*id];
        }
    }
}
impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "thompson::NFA(")?;
        for (sid, state) in self.states.iter().with_state_ids() {
            let status = if sid == self.start_anchored {
                '^'
            } else if sid == self.start_unanchored {
                '>'
            } else {
                ' '
            };
            writeln!(f, "{}{:06?}: {:?}", status, sid.as_usize(), state)?;
        }
        let pattern_len = self.start_pattern.len();
        if pattern_len > 1 {
            writeln!(f)?;
            for pid in 0..pattern_len {
                let sid = self.start_pattern[pid];
                writeln!(f, "START({:06?}): {:?}", pid, sid.as_usize())?;
            }
        }
        writeln!(f)?;
        writeln!(f, "transition equivalence classes: {:?}", self.byte_classes,)?;
        writeln!(f, ")")?;
        Ok(())
    }
}
#[derive(Clone, Eq, PartialEq)]
pub enum State {
    ByteRange {
        trans: Transition,
    },
    Sparse(SparseTransitions),
    Dense(DenseTransitions),
    Look {
        look: Look,
        next: StateID,
    },
    Union {
        alternates: Box<[StateID]>,
    },
    BinaryUnion {
        alt1: StateID,
        alt2: StateID,
    },
    Capture {
        next: StateID,
        pattern_id: PatternID,
        group_index: SmallIndex,
        slot: SmallIndex,
    },
    Fail,
    Match {
        pattern_id: PatternID,
    },
}
impl State {
    #[inline]
    pub fn is_epsilon(&self) -> bool {
        match *self {
            State::ByteRange { .. }
            | State::Sparse { .. }
            | State::Dense { .. }
            | State::Fail
            | State::Match { .. } => false,
            State::Look { .. }
            | State::Union { .. }
            | State::BinaryUnion { .. }
            | State::Capture { .. } => true,
        }
    }
    fn memory_usage(&self) -> usize {
        match *self {
            State::ByteRange { .. }
            | State::Look { .. }
            | State::BinaryUnion { .. }
            | State::Capture { .. }
            | State::Match { .. }
            | State::Fail => 0,
            State::Sparse(SparseTransitions { ref transitions }) => {
                transitions.len() * mem::size_of::<Transition>()
            }
            State::Dense { .. } => 256 * mem::size_of::<StateID>(),
            State::Union { ref alternates } => alternates.len() * mem::size_of::<StateID>(),
        }
    }
    fn remap(&mut self, remap: &[StateID]) {
        match *self {
            State::ByteRange { ref mut trans } => trans.next = remap[trans.next],
            State::Sparse(SparseTransitions {
                ref mut transitions,
            }) => {
                for t in transitions.iter_mut() {
                    t.next = remap[t.next];
                }
            }
            State::Dense(DenseTransitions {
                ref mut transitions,
            }) => {
                for sid in transitions.iter_mut() {
                    *sid = remap[*sid];
                }
            }
            State::Look { ref mut next, .. } => *next = remap[*next],
            State::Union { ref mut alternates } => {
                for alt in alternates.iter_mut() {
                    *alt = remap[*alt];
                }
            }
            State::BinaryUnion {
                ref mut alt1,
                ref mut alt2,
            } => {
                *alt1 = remap[*alt1];
                *alt2 = remap[*alt2];
            }
            State::Capture { ref mut next, .. } => *next = remap[*next],
            State::Fail => {}
            State::Match { .. } => {}
        }
    }
}
impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            State::ByteRange { ref trans } => trans.fmt(f),
            State::Sparse(SparseTransitions { ref transitions }) => {
                let rs = transitions
                    .iter()
                    .map(|t| format!("{t:?}"))
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "sparse({rs})")
            }
            State::Dense(ref dense) => {
                write!(f, "dense(")?;
                for (i, t) in dense.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{t:?}")?;
                }
                write!(f, ")")
            }
            State::Look { ref look, next } => {
                write!(f, "{:?} => {:?}", look, next.as_usize())
            }
            State::Union { ref alternates } => {
                let alts = alternates
                    .iter()
                    .map(|id| format!("{:?}", id.as_usize()))
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "union({alts})")
            }
            State::BinaryUnion { alt1, alt2 } => {
                write!(f, "binary-union({}, {})", alt1.as_usize(), alt2.as_usize())
            }
            State::Capture {
                next,
                pattern_id,
                group_index,
                slot,
            } => {
                write!(
                    f,
                    "capture(pid={:?}, group={:?}, slot={:?}) => {:?}",
                    pattern_id.as_usize(),
                    group_index.as_usize(),
                    slot.as_usize(),
                    next.as_usize(),
                )
            }
            State::Fail => write!(f, "FAIL"),
            State::Match { pattern_id } => {
                write!(f, "MATCH({:?})", pattern_id.as_usize())
            }
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseTransitions {
    pub transitions: Box<[Transition]>,
}
impl SparseTransitions {
    #[inline]
    pub fn matches(&self, haystack: &[u8], at: usize) -> Option<StateID> {
        haystack.get(at).and_then(|&b| self.matches_byte(b))
    }
    #[inline]
    pub(crate) fn matches_unit(&self, unit: alphabet::Unit) -> Option<StateID> {
        unit.as_u8().and_then(|byte| self.matches_byte(byte))
    }
    #[inline]
    pub fn matches_byte(&self, byte: u8) -> Option<StateID> {
        for t in self.transitions.iter() {
            if t.start > byte {
                break;
            } else if t.matches_byte(byte) {
                return Some(t.next);
            }
        }
        None
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DenseTransitions {
    pub transitions: Box<[StateID]>,
}
impl DenseTransitions {
    #[inline]
    pub fn matches(&self, haystack: &[u8], at: usize) -> Option<StateID> {
        haystack.get(at).and_then(|&b| self.matches_byte(b))
    }
    #[inline]
    pub(crate) fn matches_unit(&self, unit: alphabet::Unit) -> Option<StateID> {
        unit.as_u8().and_then(|byte| self.matches_byte(byte))
    }
    #[inline]
    pub fn matches_byte(&self, byte: u8) -> Option<StateID> {
        let next = self.transitions[usize::from(byte)];
        if next == StateID::ZERO {
            None
        } else {
            Some(next)
        }
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = Transition> + '_ {
        use crate::regex_engine::int::Usize;
        self.transitions
            .iter()
            .enumerate()
            .filter(|&(_, &sid)| sid != StateID::ZERO)
            .map(|(byte, &next)| Transition {
                start: byte.as_u8(),
                end: byte.as_u8(),
                next,
            })
    }
}
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Transition {
    pub start: u8,
    pub end: u8,
    pub next: StateID,
}
impl Transition {
    pub fn matches(&self, haystack: &[u8], at: usize) -> bool {
        haystack.get(at).map_or(false, |&b| self.matches_byte(b))
    }
    pub fn matches_unit(&self, unit: alphabet::Unit) -> bool {
        unit.as_u8().map_or(false, |byte| self.matches_byte(byte))
    }
    pub fn matches_byte(&self, byte: u8) -> bool {
        self.start <= byte && byte <= self.end
    }
}
impl fmt::Debug for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use crate::regex_engine::automata::util::escape::DebugByte;
        let Transition { start, end, next } = *self;
        if self.start == self.end {
            write!(f, "{:?} => {:?}", DebugByte(start), next.as_usize())
        } else {
            write!(
                f,
                "{:?}-{:?} => {:?}",
                DebugByte(start),
                DebugByte(end),
                next.as_usize(),
            )
        }
    }
}
#[derive(Debug)]
pub struct PatternIter<'a> {
    it: PatternIDIter,
    _marker: core::marker::PhantomData<&'a ()>,
}
impl<'a> Iterator for PatternIter<'a> {
    type Item = PatternID;
    fn next(&mut self) -> Option<PatternID> {
        self.it.next()
    }
}
