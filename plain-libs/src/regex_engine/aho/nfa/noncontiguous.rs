use crate::regex_engine::aho::{
    automaton::Automaton,
    util::{
        alphabet::{ByteClassSet, ByteClasses},
        error::{BuildError, MatchError},
        prefilter::{self, Prefilter, opposite_ascii_case},
        primitives::{IteratorIndexExt, PatternID, SmallIndex, StateID},
        remapper::Remapper,
        search::{Anchored, MatchKind},
        special::Special,
    },
};
use std::{
    collections::{BTreeSet, VecDeque},
    vec,
    vec::Vec,
};
#[derive(Clone)]
pub struct NFA {
    match_kind: MatchKind,
    states: Vec<State>,
    sparse: Vec<Transition>,
    dense: Vec<StateID>,
    matches: Vec<Match>,
    pattern_lens: Vec<SmallIndex>,
    prefilter: Option<Prefilter>,
    byte_classes: ByteClasses,
    min_pattern_len: usize,
    max_pattern_len: usize,
    special: Special,
}
impl NFA {
    pub fn new<I, P>(patterns: I) -> Result<NFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        NFA::builder().build(patterns)
    }
    pub fn builder() -> Builder {
        Builder::new()
    }
}
impl NFA {
    pub(crate) const DEAD: StateID = StateID::new_unchecked(0);
    pub(crate) const FAIL: StateID = StateID::new_unchecked(1);
    pub(crate) fn byte_classes(&self) -> &ByteClasses {
        &self.byte_classes
    }
    pub(crate) fn pattern_lens_raw(&self) -> &[SmallIndex] {
        &self.pattern_lens
    }
    pub(crate) fn states(&self) -> &[State] {
        &self.states
    }
    pub(crate) fn special(&self) -> &Special {
        &self.special
    }
    pub(crate) fn swap_states(&mut self, id1: StateID, id2: StateID) {
        self.states.swap(id1.as_usize(), id2.as_usize());
    }
    pub(crate) fn remap(&mut self, map: impl Fn(StateID) -> StateID) {
        let alphabet_len = self.byte_classes.alphabet_len();
        for state in self.states.iter_mut() {
            state.fail = map(state.fail);
            let mut link = state.sparse;
            while link != StateID::ZERO {
                let t = &mut self.sparse[link];
                t.next = map(t.next);
                link = t.link;
            }
            if state.dense != StateID::ZERO {
                let start = state.dense.as_usize();
                for next in self.dense[start..][..alphabet_len].iter_mut() {
                    *next = map(*next);
                }
            }
        }
    }
    pub(crate) fn iter_trans(&self, sid: StateID) -> impl Iterator<Item = Transition> + '_ {
        let mut link = self.states[sid].sparse;
        core::iter::from_fn(move || {
            if link == StateID::ZERO {
                return None;
            }
            let t = self.sparse[link];
            link = t.link;
            Some(t)
        })
    }
    pub(crate) fn iter_matches(&self, sid: StateID) -> impl Iterator<Item = PatternID> + '_ {
        let mut link = self.states[sid].matches;
        core::iter::from_fn(move || {
            if link == StateID::ZERO {
                return None;
            }
            let m = self.matches[link];
            link = m.link;
            Some(m.pid)
        })
    }
    fn next_link(&self, sid: StateID, prev: Option<StateID>) -> Option<StateID> {
        let link = prev.map_or(self.states[sid].sparse, |p| self.sparse[p].link);
        if link == StateID::ZERO {
            None
        } else {
            Some(link)
        }
    }
    #[inline(always)]
    fn follow_transition(&self, sid: StateID, byte: u8) -> StateID {
        let s = &self.states[sid];
        if s.dense == StateID::ZERO {
            self.follow_transition_sparse(sid, byte)
        } else {
            let class = usize::from(self.byte_classes.get(byte));
            self.dense[s.dense.as_usize() + class]
        }
    }
    #[inline(always)]
    fn follow_transition_sparse(&self, sid: StateID, byte: u8) -> StateID {
        for t in self.iter_trans(sid) {
            if byte <= t.byte {
                if byte == t.byte {
                    return t.next;
                }
                break;
            }
        }
        NFA::FAIL
    }
    fn add_transition(&mut self, prev: StateID, byte: u8, next: StateID) -> Result<(), BuildError> {
        if self.states[prev].dense != StateID::ZERO {
            let dense = self.states[prev].dense;
            let class = usize::from(self.byte_classes.get(byte));
            self.dense[dense.as_usize() + class] = next;
        }
        let head = self.states[prev].sparse;
        if head == StateID::ZERO || byte < self.sparse[head].byte {
            let new_link = self.alloc_transition()?;
            self.sparse[new_link] = Transition {
                byte,
                next,
                link: head,
            };
            self.states[prev].sparse = new_link;
            return Ok(());
        } else if byte == self.sparse[head].byte {
            self.sparse[head].next = next;
            return Ok(());
        }
        let (mut link_prev, mut link_next) = (head, self.sparse[head].link);
        while link_next != StateID::ZERO && byte > self.sparse[link_next].byte {
            link_prev = link_next;
            link_next = self.sparse[link_next].link;
        }
        if link_next == StateID::ZERO || byte < self.sparse[link_next].byte {
            let link = self.alloc_transition()?;
            self.sparse[link] = Transition {
                byte,
                next,
                link: link_next,
            };
            self.sparse[link_prev].link = link;
        } else {
            assert_eq!(byte, self.sparse[link_next].byte);
            self.sparse[link_next].next = next;
        }
        Ok(())
    }
    fn init_full_state(&mut self, prev: StateID, next: StateID) -> Result<(), BuildError> {
        assert_eq!(
            StateID::ZERO,
            self.states[prev].dense,
            "state must not be dense yet"
        );
        assert_eq!(
            StateID::ZERO,
            self.states[prev].sparse,
            "state must have zero transitions"
        );
        let mut prev_link = StateID::ZERO;
        for byte in 0..=255 {
            let new_link = self.alloc_transition()?;
            self.sparse[new_link] = Transition {
                byte,
                next,
                link: StateID::ZERO,
            };
            if prev_link == StateID::ZERO {
                self.states[prev].sparse = new_link;
            } else {
                self.sparse[prev_link].link = new_link;
            }
            prev_link = new_link;
        }
        Ok(())
    }
    fn add_match(&mut self, sid: StateID, pid: PatternID) -> Result<(), BuildError> {
        let head = self.states[sid].matches;
        let mut link = head;
        while self.matches[link].link != StateID::ZERO {
            link = self.matches[link].link;
        }
        let new_match_link = self.alloc_match()?;
        self.matches[new_match_link].pid = pid;
        if link == StateID::ZERO {
            self.states[sid].matches = new_match_link;
        } else {
            self.matches[link].link = new_match_link;
        }
        Ok(())
    }
    fn copy_matches(&mut self, src: StateID, dst: StateID) -> Result<(), BuildError> {
        let head_dst = self.states[dst].matches;
        let mut link_dst = head_dst;
        while self.matches[link_dst].link != StateID::ZERO {
            link_dst = self.matches[link_dst].link;
        }
        let mut link_src = self.states[src].matches;
        while link_src != StateID::ZERO {
            let new_match_link = StateID::new(self.matches.len())
                .map_err(|e| BuildError::state_id_overflow(StateID::MAX.as_u64(), e.attempted()))?;
            self.matches.push(Match {
                pid: self.matches[link_src].pid,
                link: StateID::ZERO,
            });
            if link_dst == StateID::ZERO {
                self.states[dst].matches = new_match_link;
            } else {
                self.matches[link_dst].link = new_match_link;
            }
            link_dst = new_match_link;
            link_src = self.matches[link_src].link;
        }
        Ok(())
    }
    fn alloc_transition(&mut self) -> Result<StateID, BuildError> {
        let id = StateID::new(self.sparse.len())
            .map_err(|e| BuildError::state_id_overflow(StateID::MAX.as_u64(), e.attempted()))?;
        self.sparse.push(Transition::default());
        Ok(id)
    }
    fn alloc_match(&mut self) -> Result<StateID, BuildError> {
        let id = StateID::new(self.matches.len())
            .map_err(|e| BuildError::state_id_overflow(StateID::MAX.as_u64(), e.attempted()))?;
        self.matches.push(Match::default());
        Ok(id)
    }
    fn alloc_dense_state(&mut self) -> Result<StateID, BuildError> {
        let id = StateID::new(self.dense.len())
            .map_err(|e| BuildError::state_id_overflow(StateID::MAX.as_u64(), e.attempted()))?;
        self.dense
            .extend(core::iter::repeat(NFA::FAIL).take(self.byte_classes.alphabet_len()));
        Ok(id)
    }
    fn alloc_state(&mut self, depth: usize) -> Result<StateID, BuildError> {
        let depth =
            SmallIndex::new(depth).expect("patterns longer than SmallIndex::MAX are not allowed");
        let id = StateID::new(self.states.len())
            .map_err(|e| BuildError::state_id_overflow(StateID::MAX.as_u64(), e.attempted()))?;
        self.states.push(State {
            sparse: StateID::ZERO,
            dense: StateID::ZERO,
            matches: StateID::ZERO,
            fail: self.special.start_unanchored_id,
            depth,
        });
        Ok(id)
    }
}
unsafe impl Automaton for NFA {
    #[inline(always)]
    fn start_state(&self, anchored: Anchored) -> Result<StateID, MatchError> {
        match anchored {
            Anchored::No => Ok(self.special.start_unanchored_id),
            Anchored::Yes => Ok(self.special.start_anchored_id),
        }
    }
    #[inline(always)]
    fn next_state(&self, anchored: Anchored, mut sid: StateID, byte: u8) -> StateID {
        loop {
            let next = self.follow_transition(sid, byte);
            if next != NFA::FAIL {
                return next;
            }
            if anchored.is_anchored() {
                return NFA::DEAD;
            }
            sid = self.states[sid].fail();
        }
    }
    #[inline(always)]
    fn is_special(&self, sid: StateID) -> bool {
        sid <= self.special.max_special_id
    }
    #[inline(always)]
    fn is_dead(&self, sid: StateID) -> bool {
        sid == NFA::DEAD
    }
    #[inline(always)]
    fn is_match(&self, sid: StateID) -> bool {
        !self.is_dead(sid) && sid <= self.special.max_match_id
    }
    #[inline(always)]
    fn is_start(&self, sid: StateID) -> bool {
        sid == self.special.start_unanchored_id || sid == self.special.start_anchored_id
    }
    #[inline(always)]
    fn match_kind(&self) -> MatchKind {
        self.match_kind
    }
    #[inline(always)]
    fn patterns_len(&self) -> usize {
        self.pattern_lens.len()
    }
    #[inline(always)]
    fn pattern_len(&self, pid: PatternID) -> usize {
        self.pattern_lens[pid].as_usize()
    }
    #[inline(always)]
    fn min_pattern_len(&self) -> usize {
        self.min_pattern_len
    }
    #[inline(always)]
    fn max_pattern_len(&self) -> usize {
        self.max_pattern_len
    }
    #[inline(always)]
    fn match_len(&self, sid: StateID) -> usize {
        self.iter_matches(sid).count()
    }
    #[inline(always)]
    fn match_pattern(&self, sid: StateID, index: usize) -> PatternID {
        self.iter_matches(sid).nth(index).unwrap()
    }
    #[inline(always)]
    fn memory_usage(&self) -> usize {
        self.states.len() * core::mem::size_of::<State>()
            + self.sparse.len() * core::mem::size_of::<Transition>()
            + self.matches.len() * core::mem::size_of::<Match>()
            + self.dense.len() * StateID::SIZE
            + self.pattern_lens.len() * SmallIndex::SIZE
            + self.prefilter.as_ref().map_or(0, |p| p.memory_usage())
    }
    #[inline(always)]
    fn prefilter(&self) -> Option<&Prefilter> {
        self.prefilter.as_ref()
    }
}
#[derive(Clone, Debug)]
pub(crate) struct State {
    sparse: StateID,
    dense: StateID,
    matches: StateID,
    fail: StateID,
    depth: SmallIndex,
}
impl State {
    pub(crate) fn is_match(&self) -> bool {
        self.matches != StateID::ZERO
    }
    pub(crate) fn fail(&self) -> StateID {
        self.fail
    }
    pub(crate) fn depth(&self) -> SmallIndex {
        self.depth
    }
}
#[derive(Clone, Copy, Default)]
#[repr(packed)]
pub(crate) struct Transition {
    byte: u8,
    next: StateID,
    link: StateID,
}
impl Transition {
    pub(crate) fn byte(&self) -> u8 {
        self.byte
    }
    pub(crate) fn next(&self) -> StateID {
        self.next
    }
    fn link(&self) -> StateID {
        self.link
    }
}
impl core::fmt::Debug for Transition {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "Transition(byte: {:X?}, next: {:?}, link: {:?})",
            self.byte,
            self.next().as_usize(),
            self.link().as_usize()
        )
    }
}
#[derive(Clone, Copy, Default)]
struct Match {
    pid: PatternID,
    link: StateID,
}
impl Match {
    pub(crate) fn pattern(&self) -> PatternID {
        self.pid
    }
    fn link(&self) -> StateID {
        self.link
    }
}
impl core::fmt::Debug for Match {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "Match(pid: {:?}, link: {:?})",
            self.pattern().as_usize(),
            self.link().as_usize()
        )
    }
}
#[derive(Clone, Debug)]
pub struct Builder {
    match_kind: MatchKind,
    prefilter: bool,
    ascii_case_insensitive: bool,
    dense_depth: usize,
}
impl Default for Builder {
    fn default() -> Builder {
        Builder {
            match_kind: MatchKind::default(),
            prefilter: true,
            ascii_case_insensitive: false,
            dense_depth: 3,
        }
    }
}
impl Builder {
    pub fn new() -> Builder {
        Builder::default()
    }
    pub fn build<I, P>(&self, patterns: I) -> Result<NFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        debug!("building non-contiguous NFA");
        let nfa = Compiler::new(self)?.compile(patterns)?;
        debug!(
            "non-contiguous NFA built, <states: {:?}, size: {:?}>",
            nfa.states.len(),
            nfa.memory_usage()
        );
        Ok(nfa)
    }
    pub fn match_kind(&mut self, kind: MatchKind) -> &mut Builder {
        self.match_kind = kind;
        self
    }
    pub fn ascii_case_insensitive(&mut self, yes: bool) -> &mut Builder {
        self.ascii_case_insensitive = yes;
        self
    }
    pub fn dense_depth(&mut self, depth: usize) -> &mut Builder {
        self.dense_depth = depth;
        self
    }
    pub fn prefilter(&mut self, yes: bool) -> &mut Builder {
        self.prefilter = yes;
        self
    }
}
#[derive(Debug)]
struct Compiler<'a> {
    builder: &'a Builder,
    prefilter: prefilter::Builder,
    nfa: NFA,
    byteset: ByteClassSet,
}
impl<'a> Compiler<'a> {
    fn new(builder: &'a Builder) -> Result<Compiler<'a>, BuildError> {
        let prefilter = prefilter::Builder::new(builder.match_kind)
            .ascii_case_insensitive(builder.ascii_case_insensitive);
        Ok(Compiler {
            builder,
            prefilter,
            nfa: NFA {
                match_kind: builder.match_kind,
                states: vec![],
                sparse: vec![],
                dense: vec![],
                matches: vec![],
                pattern_lens: vec![],
                prefilter: None,
                byte_classes: ByteClasses::singletons(),
                min_pattern_len: usize::MAX,
                max_pattern_len: 0,
                special: Special::zero(),
            },
            byteset: ByteClassSet::empty(),
        })
    }
    fn compile<I, P>(mut self, patterns: I) -> Result<NFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        self.nfa.sparse.push(Transition::default());
        self.nfa.matches.push(Match::default());
        self.nfa.dense.push(NFA::DEAD);
        self.nfa.alloc_state(0)?;
        self.nfa.alloc_state(0)?;
        self.nfa.special.start_unanchored_id = self.nfa.alloc_state(0)?;
        self.nfa.special.start_anchored_id = self.nfa.alloc_state(0)?;
        self.init_unanchored_start_state()?;
        self.add_dead_state_loop()?;
        self.build_trie(patterns)?;
        self.nfa.states.shrink_to_fit();
        self.nfa.byte_classes = self.byteset.byte_classes();
        self.set_anchored_start_state()?;
        self.add_unanchored_start_state_loop();
        self.densify()?;
        self.fill_failure_transitions()?;
        self.close_start_state_loop_for_leftmost();
        self.shuffle();
        self.nfa.prefilter = self.prefilter.build();
        self.nfa.special.max_special_id = if self.nfa.prefilter.is_some() {
            self.nfa.special.start_anchored_id
        } else {
            self.nfa.special.max_match_id
        };
        self.nfa.sparse.shrink_to_fit();
        self.nfa.dense.shrink_to_fit();
        self.nfa.matches.shrink_to_fit();
        self.nfa.pattern_lens.shrink_to_fit();
        Ok(self.nfa)
    }
    fn build_trie<I, P>(&mut self, patterns: I) -> Result<(), BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        'PATTERNS: for (i, pat) in patterns.into_iter().enumerate() {
            let pid = PatternID::new(i).map_err(|e| {
                BuildError::pattern_id_overflow(PatternID::MAX.as_u64(), e.attempted())
            })?;
            let pat = pat.as_ref();
            let patlen = SmallIndex::new(pat.len())
                .map_err(|_| BuildError::pattern_too_long(pid, pat.len()))?;
            self.nfa.min_pattern_len = core::cmp::min(self.nfa.min_pattern_len, pat.len());
            self.nfa.max_pattern_len = core::cmp::max(self.nfa.max_pattern_len, pat.len());
            assert_eq!(
                i,
                self.nfa.pattern_lens.len(),
                "expected number of patterns to match pattern ID"
            );
            self.nfa.pattern_lens.push(patlen);
            if self.builder.prefilter {
                self.prefilter.add(pat);
            }
            let mut prev = self.nfa.special.start_unanchored_id;
            let mut saw_match = false;
            for (depth, &b) in pat.iter().enumerate() {
                saw_match = saw_match || self.nfa.states[prev].is_match();
                if self.builder.match_kind.is_leftmost_first() && saw_match {
                    continue 'PATTERNS;
                }
                self.byteset.set_range(b, b);
                if self.builder.ascii_case_insensitive {
                    let b = opposite_ascii_case(b);
                    self.byteset.set_range(b, b);
                }
                let next = self.nfa.follow_transition(prev, b);
                if next != NFA::FAIL {
                    prev = next;
                } else {
                    let next = self.nfa.alloc_state(depth)?;
                    self.nfa.add_transition(prev, b, next)?;
                    if self.builder.ascii_case_insensitive {
                        let b = opposite_ascii_case(b);
                        self.nfa.add_transition(prev, b, next)?;
                    }
                    prev = next;
                }
            }
            self.nfa.add_match(prev, pid)?;
        }
        Ok(())
    }
    fn fill_failure_transitions(&mut self) -> Result<(), BuildError> {
        let is_leftmost = self.builder.match_kind.is_leftmost();
        let start_uid = self.nfa.special.start_unanchored_id;
        let mut queue = VecDeque::new();
        let mut seen = self.queued_set();
        let mut prev_link = None;
        while let Some(link) = self.nfa.next_link(start_uid, prev_link) {
            prev_link = Some(link);
            let t = self.nfa.sparse[link];
            if start_uid == t.next() || seen.contains(t.next) {
                continue;
            }
            queue.push_back(t.next);
            seen.insert(t.next);
            if is_leftmost && self.nfa.states[t.next].is_match() {
                self.nfa.states[t.next].fail = NFA::DEAD;
            }
        }
        while let Some(id) = queue.pop_front() {
            let mut prev_link = None;
            while let Some(link) = self.nfa.next_link(id, prev_link) {
                prev_link = Some(link);
                let t = self.nfa.sparse[link];
                if seen.contains(t.next) {
                    continue;
                }
                queue.push_back(t.next);
                seen.insert(t.next);
                if is_leftmost && self.nfa.states[t.next].is_match() {
                    self.nfa.states[t.next].fail = NFA::DEAD;
                    continue;
                }
                let mut fail = self.nfa.states[id].fail;
                while self.nfa.follow_transition(fail, t.byte) == NFA::FAIL {
                    fail = self.nfa.states[fail].fail;
                }
                fail = self.nfa.follow_transition(fail, t.byte);
                self.nfa.states[t.next].fail = fail;
                self.nfa.copy_matches(fail, t.next)?;
            }
            if !is_leftmost {
                self.nfa
                    .copy_matches(self.nfa.special.start_unanchored_id, id)?;
            }
        }
        Ok(())
    }
    fn shuffle(&mut self) {
        let old_start_uid = self.nfa.special.start_unanchored_id;
        let old_start_aid = self.nfa.special.start_anchored_id;
        assert!(old_start_uid < old_start_aid);
        assert_eq!(
            3,
            old_start_aid.as_usize(),
            "anchored start state should be at index 3"
        );
        let mut remapper = Remapper::new(&self.nfa, 0);
        let mut next_avail = StateID::from(4u8);
        for i in next_avail.as_usize()..self.nfa.states.len() {
            let sid = StateID::new(i).unwrap();
            if !self.nfa.states[sid].is_match() {
                continue;
            }
            remapper.swap(&mut self.nfa, sid, next_avail);
            next_avail = StateID::new(next_avail.one_more()).unwrap();
        }
        let new_start_aid = StateID::new(next_avail.as_usize().checked_sub(1).unwrap()).unwrap();
        remapper.swap(&mut self.nfa, old_start_aid, new_start_aid);
        let new_start_uid = StateID::new(next_avail.as_usize().checked_sub(2).unwrap()).unwrap();
        remapper.swap(&mut self.nfa, old_start_uid, new_start_uid);
        let new_max_match_id = StateID::new(next_avail.as_usize().checked_sub(3).unwrap()).unwrap();
        self.nfa.special.max_match_id = new_max_match_id;
        self.nfa.special.start_unanchored_id = new_start_uid;
        self.nfa.special.start_anchored_id = new_start_aid;
        if self.nfa.states[self.nfa.special.start_anchored_id].is_match() {
            self.nfa.special.max_match_id = self.nfa.special.start_anchored_id;
        }
        remapper.remap(&mut self.nfa);
    }
    fn densify(&mut self) -> Result<(), BuildError> {
        for i in 0..self.nfa.states.len() {
            let sid = StateID::new(i).unwrap();
            if sid == NFA::DEAD || sid == NFA::FAIL {
                continue;
            }
            if self.nfa.states[sid].depth.as_usize() >= self.builder.dense_depth {
                continue;
            }
            let dense = self.nfa.alloc_dense_state()?;
            let mut prev_link = None;
            while let Some(link) = self.nfa.next_link(sid, prev_link) {
                prev_link = Some(link);
                let t = self.nfa.sparse[link];
                let class = usize::from(self.nfa.byte_classes.get(t.byte));
                let index = dense.as_usize() + class;
                self.nfa.dense[index] = t.next;
            }
            self.nfa.states[sid].dense = dense;
        }
        Ok(())
    }
    fn queued_set(&self) -> QueuedSet {
        if self.builder.ascii_case_insensitive {
            QueuedSet::active()
        } else {
            QueuedSet::inert()
        }
    }
    fn init_unanchored_start_state(&mut self) -> Result<(), BuildError> {
        let start_uid = self.nfa.special.start_unanchored_id;
        let start_aid = self.nfa.special.start_anchored_id;
        self.nfa.init_full_state(start_uid, NFA::FAIL)?;
        self.nfa.init_full_state(start_aid, NFA::FAIL)?;
        Ok(())
    }
    fn set_anchored_start_state(&mut self) -> Result<(), BuildError> {
        let start_uid = self.nfa.special.start_unanchored_id;
        let start_aid = self.nfa.special.start_anchored_id;
        let (mut uprev_link, mut aprev_link) = (None, None);
        loop {
            let unext = self.nfa.next_link(start_uid, uprev_link);
            let anext = self.nfa.next_link(start_aid, aprev_link);
            let (ulink, alink) = match (unext, anext) {
                (Some(ulink), Some(alink)) => (ulink, alink),
                (None, None) => break,
                _ => unreachable!(),
            };
            uprev_link = Some(ulink);
            aprev_link = Some(alink);
            self.nfa.sparse[alink].next = self.nfa.sparse[ulink].next;
        }
        self.nfa.copy_matches(start_uid, start_aid)?;
        self.nfa.states[start_aid].fail = NFA::DEAD;
        Ok(())
    }
    fn add_unanchored_start_state_loop(&mut self) {
        let start_uid = self.nfa.special.start_unanchored_id;
        let mut prev_link = None;
        while let Some(link) = self.nfa.next_link(start_uid, prev_link) {
            prev_link = Some(link);
            if self.nfa.sparse[link].next() == NFA::FAIL {
                self.nfa.sparse[link].next = start_uid;
            }
        }
    }
    fn close_start_state_loop_for_leftmost(&mut self) {
        let start_uid = self.nfa.special.start_unanchored_id;
        let start = &mut self.nfa.states[start_uid];
        let dense = start.dense;
        if self.builder.match_kind.is_leftmost() && start.is_match() {
            let mut prev_link = None;
            while let Some(link) = self.nfa.next_link(start_uid, prev_link) {
                prev_link = Some(link);
                if self.nfa.sparse[link].next() == start_uid {
                    self.nfa.sparse[link].next = NFA::DEAD;
                    if dense != StateID::ZERO {
                        let b = self.nfa.sparse[link].byte;
                        let class = usize::from(self.nfa.byte_classes.get(b));
                        self.nfa.dense[dense.as_usize() + class] = NFA::DEAD;
                    }
                }
            }
        }
    }
    fn add_dead_state_loop(&mut self) -> Result<(), BuildError> {
        self.nfa.init_full_state(NFA::DEAD, NFA::DEAD)?;
        Ok(())
    }
}
#[derive(Debug)]
struct QueuedSet {
    set: Option<BTreeSet<StateID>>,
}
impl QueuedSet {
    fn inert() -> QueuedSet {
        QueuedSet { set: None }
    }
    fn active() -> QueuedSet {
        QueuedSet {
            set: Some(BTreeSet::new()),
        }
    }
    fn insert(&mut self, state_id: StateID) {
        if let Some(ref mut set) = self.set {
            set.insert(state_id);
        }
    }
    fn contains(&self, state_id: StateID) -> bool {
        match self.set {
            None => false,
            Some(ref set) => set.contains(&state_id),
        }
    }
}
impl core::fmt::Debug for NFA {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use crate::regex_engine::aho::{
            automaton::{fmt_state_indicator, sparse_transitions},
            util::debug::DebugByte,
        };
        writeln!(f, "noncontiguous::NFA(")?;
        for (sid, state) in self.states.iter().with_state_ids() {
            if sid == NFA::FAIL {
                writeln!(f, "F {:06}:", sid.as_usize())?;
                continue;
            }
            fmt_state_indicator(f, self, sid)?;
            write!(f, "{:06}({:06}): ", sid.as_usize(), state.fail.as_usize())?;
            let it = sparse_transitions(self.iter_trans(sid).map(|t| (t.byte, t.next))).enumerate();
            for (i, (start, end, sid)) in it {
                if i > 0 {
                    write!(f, ", ")?;
                }
                if start == end {
                    write!(f, "{:?} => {:?}", DebugByte(start), sid.as_usize())?;
                } else {
                    write!(
                        f,
                        "{:?}-{:?} => {:?}",
                        DebugByte(start),
                        DebugByte(end),
                        sid.as_usize()
                    )?;
                }
            }
            write!(f, "\n")?;
            if self.is_match(sid) {
                write!(f, "         matches: ")?;
                for (i, pid) in self.iter_matches(sid).enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", pid.as_usize())?;
                }
                write!(f, "\n")?;
            }
        }
        writeln!(f, "match kind: {:?}", self.match_kind)?;
        writeln!(f, "prefilter: {:?}", self.prefilter.is_some())?;
        writeln!(f, "state length: {:?}", self.states.len())?;
        writeln!(f, "pattern length: {:?}", self.patterns_len())?;
        writeln!(f, "shortest pattern length: {:?}", self.min_pattern_len)?;
        writeln!(f, "longest pattern length: {:?}", self.max_pattern_len)?;
        writeln!(f, "memory usage: {:?}", self.memory_usage())?;
        writeln!(f, ")")?;
        Ok(())
    }
}
