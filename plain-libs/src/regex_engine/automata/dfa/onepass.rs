use crate::regex_engine::automata::{
    dfa::{DEAD, remapper::Remapper},
    nfa::thompson::{self, NFA},
    util::{
        alphabet::ByteClasses,
        captures::Captures,
        escape::DebugByte,
        look::{Look, LookSet, UnicodeWordBoundaryError},
        primitives::{NonMaxUsize, PatternID, StateID},
        search::{Anchored, Input, Match, MatchError, MatchKind, Span},
        sparse_set::SparseSet,
    },
};
use crate::regex_engine::int::{U8, U32, U64, Usize};
use std::{vec, vec::Vec};
#[derive(Clone, Debug, Default)]
pub struct Config {
    match_kind: Option<MatchKind>,
    starts_for_each_pattern: Option<bool>,
    byte_classes: Option<bool>,
    size_limit: Option<Option<usize>>,
}
impl Config {
    pub fn new() -> Config {
        Config::default()
    }
    pub fn match_kind(mut self, kind: MatchKind) -> Config {
        self.match_kind = Some(kind);
        self
    }
    pub fn starts_for_each_pattern(mut self, yes: bool) -> Config {
        self.starts_for_each_pattern = Some(yes);
        self
    }
    pub fn byte_classes(mut self, yes: bool) -> Config {
        self.byte_classes = Some(yes);
        self
    }
    pub fn size_limit(mut self, limit: Option<usize>) -> Config {
        self.size_limit = Some(limit);
        self
    }
    pub fn get_match_kind(&self) -> MatchKind {
        self.match_kind.unwrap_or(MatchKind::LeftmostFirst)
    }
    pub fn get_starts_for_each_pattern(&self) -> bool {
        self.starts_for_each_pattern.unwrap_or(false)
    }
    pub fn get_byte_classes(&self) -> bool {
        self.byte_classes.unwrap_or(true)
    }
    pub fn get_size_limit(&self) -> Option<usize> {
        self.size_limit.unwrap_or(None)
    }
    pub(crate) fn overwrite(&self, o: Config) -> Config {
        Config {
            match_kind: o.match_kind.or(self.match_kind),
            starts_for_each_pattern: o.starts_for_each_pattern.or(self.starts_for_each_pattern),
            byte_classes: o.byte_classes.or(self.byte_classes),
            size_limit: o.size_limit.or(self.size_limit),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Builder {
    config: Config,
    thompson: thompson::Compiler,
}
impl Builder {
    pub fn new() -> Builder {
        Builder {
            config: Config::default(),
            thompson: thompson::Compiler::new(),
        }
    }
    pub fn build(&self, pattern: &str) -> Result<DFA, BuildError> {
        self.build_many(&[pattern])
    }
    pub fn build_many<P: AsRef<str>>(&self, patterns: &[P]) -> Result<DFA, BuildError> {
        let nfa = self
            .thompson
            .build_many(patterns)
            .map_err(BuildError::nfa)?;
        self.build_from_nfa(nfa)
    }
    pub fn build_from_nfa(&self, nfa: NFA) -> Result<DFA, BuildError> {
        InternalBuilder::new(self.config.clone(), &nfa).build()
    }
    pub fn configure(&mut self, config: Config) -> &mut Builder {
        self.config = self.config.overwrite(config);
        self
    }
    pub fn syntax(
        &mut self,
        config: crate::regex_engine::automata::util::syntax::Config,
    ) -> &mut Builder {
        self.thompson.syntax(config);
        self
    }
    pub fn thompson(&mut self, config: thompson::Config) -> &mut Builder {
        self.thompson.configure(config);
        self
    }
}
#[derive(Debug)]
struct InternalBuilder<'a> {
    dfa: DFA,
    uncompiled_nfa_ids: Vec<StateID>,
    nfa_to_dfa_id: Vec<StateID>,
    stack: Vec<(StateID, Epsilons)>,
    seen: SparseSet,
    matched: bool,
    config: Config,
    nfa: &'a NFA,
    classes: ByteClasses,
}
impl<'a> InternalBuilder<'a> {
    fn new(config: Config, nfa: &'a NFA) -> InternalBuilder<'a> {
        let classes = if !config.get_byte_classes() {
            ByteClasses::singletons()
        } else {
            nfa.byte_classes().clone()
        };
        let alphabet_len = classes.alphabet_len().checked_sub(1).unwrap();
        let stride2 = classes.stride2();
        let dfa = DFA {
            config: config.clone(),
            nfa: nfa.clone(),
            table: vec![],
            starts: vec![],
            min_match_id: StateID::MAX,
            classes: classes.clone(),
            alphabet_len,
            stride2,
            pateps_offset: alphabet_len,
            explicit_slot_start: nfa.pattern_len().checked_mul(2).unwrap(),
        };
        InternalBuilder {
            dfa,
            uncompiled_nfa_ids: vec![],
            nfa_to_dfa_id: vec![DEAD; nfa.states().len()],
            stack: vec![],
            seen: SparseSet::new(nfa.states().len()),
            matched: false,
            config,
            nfa,
            classes,
        }
    }
    fn build(mut self) -> Result<DFA, BuildError> {
        self.nfa
            .look_set_any()
            .available()
            .map_err(BuildError::word)?;
        for look in self.nfa.look_set_any().iter() {
            if look.as_repr() > Look::WordUnicodeNegate.as_repr() {
                return Err(BuildError::unsupported_look(look));
            }
        }
        if self.nfa.pattern_len().as_u64() > PatternEpsilons::PATTERN_ID_LIMIT {
            return Err(BuildError::too_many_patterns(
                PatternEpsilons::PATTERN_ID_LIMIT,
            ));
        }
        if self.nfa.group_info().explicit_slot_len() > Slots::LIMIT {
            return Err(BuildError::not_one_pass(
                "too many explicit capturing groups (max is 16)",
            ));
        }
        assert_eq!(DEAD, self.add_empty_state()?);
        let explicit_slot_start = self.nfa.pattern_len() * 2;
        self.add_start_state(None, self.nfa.start_anchored())?;
        if self.config.get_starts_for_each_pattern() {
            for pid in self.nfa.patterns() {
                self.add_start_state(Some(pid), self.nfa.start_pattern(pid).unwrap())?;
            }
        }
        while let Some(nfa_id) = self.uncompiled_nfa_ids.pop() {
            let dfa_id = self.nfa_to_dfa_id[nfa_id];
            self.matched = false;
            self.seen.clear();
            self.stack_push(nfa_id, Epsilons::empty())?;
            while let Some((id, epsilons)) = self.stack.pop() {
                match *self.nfa.state(id) {
                    thompson::State::ByteRange { ref trans } => {
                        self.compile_transition(dfa_id, trans, epsilons)?;
                    }
                    thompson::State::Sparse(ref sparse) => {
                        for trans in sparse.transitions.iter() {
                            self.compile_transition(dfa_id, trans, epsilons)?;
                        }
                    }
                    thompson::State::Dense(ref dense) => {
                        for trans in dense.iter() {
                            self.compile_transition(dfa_id, &trans, epsilons)?;
                        }
                    }
                    thompson::State::Look { look, next } => {
                        let looks = epsilons.looks().insert(look);
                        self.stack_push(next, epsilons.set_looks(looks))?;
                    }
                    thompson::State::Union { ref alternates } => {
                        for &sid in alternates.iter().rev() {
                            self.stack_push(sid, epsilons)?;
                        }
                    }
                    thompson::State::BinaryUnion { alt1, alt2 } => {
                        self.stack_push(alt2, epsilons)?;
                        self.stack_push(alt1, epsilons)?;
                    }
                    thompson::State::Capture { next, slot, .. } => {
                        let slot = slot.as_usize();
                        let epsilons = if slot < explicit_slot_start {
                            epsilons
                        } else {
                            let offset = slot - explicit_slot_start;
                            epsilons.set_slots(epsilons.slots().insert(offset))
                        };
                        self.stack_push(next, epsilons)?;
                    }
                    thompson::State::Fail => {
                        continue;
                    }
                    thompson::State::Match { pattern_id } => {
                        if self.matched {
                            return Err(BuildError::not_one_pass(
                                "multiple epsilon transitions to match state",
                            ));
                        }
                        self.matched = true;
                        self.dfa.set_pattern_epsilons(
                            dfa_id,
                            PatternEpsilons::empty()
                                .set_pattern_id(pattern_id)
                                .set_epsilons(epsilons),
                        );
                    }
                }
            }
        }
        self.shuffle_states();
        self.dfa.starts.shrink_to_fit();
        self.dfa.table.shrink_to_fit();
        Ok(self.dfa)
    }
    fn shuffle_states(&mut self) {
        let mut remapper = Remapper::new(&self.dfa);
        let mut next_dest = self.dfa.last_state_id();
        for i in (0..self.dfa.state_len()).rev() {
            let id = StateID::must(i);
            let is_match = self.dfa.pattern_epsilons(id).pattern_id().is_some();
            if !is_match {
                continue;
            }
            remapper.swap(&mut self.dfa, next_dest, id);
            self.dfa.min_match_id = next_dest;
            next_dest = self
                .dfa
                .prev_state_id(next_dest)
                .expect("match states should be a proper subset of all states");
        }
        remapper.remap(&mut self.dfa);
    }
    fn compile_transition(
        &mut self,
        dfa_id: StateID,
        trans: &thompson::Transition,
        epsilons: Epsilons,
    ) -> Result<(), BuildError> {
        let next_dfa_id = self.add_dfa_state_for_nfa_state(trans.next)?;
        for byte in self
            .classes
            .representatives(trans.start..=trans.end)
            .filter_map(|r| r.as_u8())
        {
            let oldtrans = self.dfa.transition(dfa_id, byte);
            let newtrans = Transition::new(self.matched, next_dfa_id, epsilons);
            if oldtrans.state_id() == DEAD {
                self.dfa.set_transition(dfa_id, byte, newtrans);
            } else if oldtrans != newtrans {
                return Err(BuildError::not_one_pass("conflicting transition"));
            }
        }
        Ok(())
    }
    fn add_start_state(
        &mut self,
        pid: Option<PatternID>,
        nfa_id: StateID,
    ) -> Result<StateID, BuildError> {
        match pid {
            None => assert!(self.dfa.starts.is_empty()),
            Some(pid) => assert!(self.dfa.starts.len() == pid.one_more()),
        }
        let dfa_id = self.add_dfa_state_for_nfa_state(nfa_id)?;
        self.dfa.starts.push(dfa_id);
        Ok(dfa_id)
    }
    fn add_dfa_state_for_nfa_state(&mut self, nfa_id: StateID) -> Result<StateID, BuildError> {
        let existing_dfa_id = self.nfa_to_dfa_id[nfa_id];
        if existing_dfa_id != DEAD {
            return Ok(existing_dfa_id);
        }
        let dfa_id = self.add_empty_state()?;
        self.nfa_to_dfa_id[nfa_id] = dfa_id;
        self.uncompiled_nfa_ids.push(nfa_id);
        Ok(dfa_id)
    }
    fn add_empty_state(&mut self) -> Result<StateID, BuildError> {
        let state_limit = Transition::STATE_ID_LIMIT;
        let next_id = self.dfa.table.len() >> self.dfa.stride2();
        let id = StateID::new(next_id).map_err(|_| BuildError::too_many_states(state_limit))?;
        if id.as_u64() > Transition::STATE_ID_LIMIT {
            return Err(BuildError::too_many_states(state_limit));
        }
        self.dfa
            .table
            .extend(core::iter::repeat(Transition(0)).take(self.dfa.stride()));
        self.dfa.set_pattern_epsilons(id, PatternEpsilons::empty());
        if let Some(size_limit) = self.config.get_size_limit() {
            if self.dfa.memory_usage() > size_limit {
                return Err(BuildError::exceeded_size_limit(size_limit));
            }
        }
        Ok(id)
    }
    fn stack_push(&mut self, nfa_id: StateID, epsilons: Epsilons) -> Result<(), BuildError> {
        if !self.seen.insert(nfa_id) {
            return Err(BuildError::not_one_pass(
                "multiple epsilon transitions to same state",
            ));
        }
        self.stack.push((nfa_id, epsilons));
        Ok(())
    }
}
#[derive(Clone)]
pub struct DFA {
    config: Config,
    nfa: NFA,
    table: Vec<Transition>,
    starts: Vec<StateID>,
    min_match_id: StateID,
    classes: ByteClasses,
    alphabet_len: usize,
    stride2: usize,
    pateps_offset: usize,
    explicit_slot_start: usize,
}
impl DFA {
    #[inline]
    pub fn new(pattern: &str) -> Result<DFA, BuildError> {
        DFA::builder().build(pattern)
    }
    #[inline]
    pub fn new_many<P: AsRef<str>>(patterns: &[P]) -> Result<DFA, BuildError> {
        DFA::builder().build_many(patterns)
    }
    pub fn new_from_nfa(nfa: NFA) -> Result<DFA, BuildError> {
        DFA::builder().build_from_nfa(nfa)
    }
    pub fn always_match() -> Result<DFA, BuildError> {
        let nfa = thompson::NFA::always_match();
        Builder::new().build_from_nfa(nfa)
    }
    pub fn never_match() -> Result<DFA, BuildError> {
        let nfa = thompson::NFA::never_match();
        Builder::new().build_from_nfa(nfa)
    }
    #[inline]
    pub fn config() -> Config {
        Config::new()
    }
    #[inline]
    pub fn builder() -> Builder {
        Builder::new()
    }
    #[inline]
    pub fn create_captures(&self) -> Captures {
        Captures::all(self.nfa.group_info().clone())
    }
    #[inline]
    pub fn create_cache(&self) -> Cache {
        Cache::new(self)
    }
    #[inline]
    pub fn reset_cache(&self, cache: &mut Cache) {
        cache.reset(self);
    }
    #[inline]
    pub fn get_config(&self) -> &Config {
        &self.config
    }
    #[inline]
    pub fn get_nfa(&self) -> &NFA {
        &self.nfa
    }
    #[inline]
    pub fn pattern_len(&self) -> usize {
        self.get_nfa().pattern_len()
    }
    #[inline]
    pub fn state_len(&self) -> usize {
        self.table.len() >> self.stride2()
    }
    #[inline]
    pub fn alphabet_len(&self) -> usize {
        self.alphabet_len
    }
    #[inline]
    pub fn stride2(&self) -> usize {
        self.stride2
    }
    #[inline]
    pub fn stride(&self) -> usize {
        1 << self.stride2()
    }
    #[inline]
    pub fn memory_usage(&self) -> usize {
        use core::mem::size_of;
        self.table.len() * size_of::<Transition>() + self.starts.len() * size_of::<StateID>()
    }
}
impl DFA {
    #[inline]
    pub fn is_match<'h, I: Into<Input<'h>>>(&self, cache: &mut Cache, input: I) -> bool {
        let mut input = input.into().earliest(true);
        if matches!(input.get_anchored(), Anchored::No) {
            input.set_anchored(Anchored::Yes);
        }
        self.try_search_slots(cache, &input, &mut [])
            .unwrap()
            .is_some()
    }
    #[inline]
    pub fn find<'h, I: Into<Input<'h>>>(&self, cache: &mut Cache, input: I) -> Option<Match> {
        let mut input = input.into();
        if matches!(input.get_anchored(), Anchored::No) {
            input.set_anchored(Anchored::Yes);
        }
        if self.get_nfa().pattern_len() == 1 {
            let mut slots = [None, None];
            let pid = self.try_search_slots(cache, &input, &mut slots).unwrap()?;
            let start = slots[0].unwrap().get();
            let end = slots[1].unwrap().get();
            return Some(Match::new(pid, Span { start, end }));
        }
        let ginfo = self.get_nfa().group_info();
        let slots_len = ginfo.implicit_slot_len();
        let mut slots = vec![None; slots_len];
        let pid = self.try_search_slots(cache, &input, &mut slots).unwrap()?;
        let start = slots[pid.as_usize() * 2].unwrap().get();
        let end = slots[pid.as_usize() * 2 + 1].unwrap().get();
        Some(Match::new(pid, Span { start, end }))
    }
    #[inline]
    pub fn captures<'h, I: Into<Input<'h>>>(
        &self,
        cache: &mut Cache,
        input: I,
        caps: &mut Captures,
    ) {
        let mut input = input.into();
        if matches!(input.get_anchored(), Anchored::No) {
            input.set_anchored(Anchored::Yes);
        }
        self.try_search(cache, &input, caps).unwrap();
    }
    #[inline]
    pub fn try_search(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        caps: &mut Captures,
    ) -> Result<(), MatchError> {
        let pid = self.try_search_slots(cache, input, caps.slots_mut())?;
        caps.set_pattern(pid);
        Ok(())
    }
    #[inline]
    pub fn try_search_slots(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Result<Option<PatternID>, MatchError> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        if !utf8empty {
            return self.try_search_slots_imp(cache, input, slots);
        }
        let min = self.get_nfa().group_info().implicit_slot_len();
        if slots.len() >= min {
            return self.try_search_slots_imp(cache, input, slots);
        }
        if self.get_nfa().pattern_len() == 1 {
            let mut enough = [None, None];
            let got = self.try_search_slots_imp(cache, input, &mut enough)?;
            slots.copy_from_slice(&enough[..slots.len()]);
            return Ok(got);
        }
        let mut enough = vec![None; min];
        let got = self.try_search_slots_imp(cache, input, &mut enough)?;
        slots.copy_from_slice(&enough[..slots.len()]);
        Ok(got)
    }
    #[inline(never)]
    fn try_search_slots_imp(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Result<Option<PatternID>, MatchError> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        match self.search_imp(cache, input, slots)? {
            None => return Ok(None),
            Some(pid) if !utf8empty => return Ok(Some(pid)),
            Some(pid) => {
                let slot_start = pid.as_usize().wrapping_mul(2);
                let slot_end = slot_start.wrapping_add(1);
                let start = slots[slot_start].unwrap().get();
                let end = slots[slot_end].unwrap().get();
                if start == end && !input.is_char_boundary(start) {
                    return Ok(None);
                }
                Ok(Some(pid))
            }
        }
    }
}
impl DFA {
    fn search_imp(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Result<Option<PatternID>, MatchError> {
        if input.is_done() {
            return Ok(None);
        }
        let explicit_slots_len = core::cmp::min(
            Slots::LIMIT,
            core::cmp::min(
                slots.len().saturating_sub(self.explicit_slot_start),
                cache.explicit_slots.len(),
            ),
        );
        cache.setup_search(explicit_slots_len);
        for slot in cache.explicit_slots() {
            *slot = None;
        }
        for slot in slots.iter_mut() {
            *slot = None;
        }
        for pid in self.nfa.patterns() {
            let i = pid.as_usize() * 2;
            if i >= slots.len() {
                break;
            }
            slots[i] = NonMaxUsize::new(input.start());
        }
        let mut pid = None;
        let mut next_sid = match input.get_anchored() {
            Anchored::Yes => self.start(),
            Anchored::Pattern(pid) => self.start_pattern(pid)?,
            Anchored::No => {
                if !self.nfa.is_always_start_anchored() {
                    return Err(MatchError::unsupported_anchored(Anchored::No));
                }
                self.start()
            }
        };
        let leftmost_first = matches!(self.config.get_match_kind(), MatchKind::LeftmostFirst);
        for at in input.start()..input.end() {
            let sid = next_sid;
            let trans = self.transition(sid, input.haystack()[at]);
            next_sid = trans.state_id();
            let epsilons = trans.epsilons();
            if sid >= self.min_match_id {
                if self.find_match(cache, input, at, sid, slots, &mut pid) {
                    if input.get_earliest() || (leftmost_first && trans.match_wins()) {
                        return Ok(pid);
                    }
                }
            }
            if sid == DEAD
                || (!epsilons.looks().is_empty()
                    && !self.nfa.look_matcher().matches_set_inline(
                        epsilons.looks(),
                        input.haystack(),
                        at,
                    ))
            {
                return Ok(pid);
            }
            epsilons.slots().apply(at, cache.explicit_slots());
        }
        if next_sid >= self.min_match_id {
            self.find_match(cache, input, input.end(), next_sid, slots, &mut pid);
        }
        Ok(pid)
    }
    #[cfg_attr(all(), inline(always))]
    fn find_match(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        at: usize,
        sid: StateID,
        slots: &mut [Option<NonMaxUsize>],
        matched_pid: &mut Option<PatternID>,
    ) -> bool {
        debug_assert!(sid >= self.min_match_id);
        let pateps = self.pattern_epsilons(sid);
        let epsilons = pateps.epsilons();
        if !epsilons.looks().is_empty()
            && !self
                .nfa
                .look_matcher()
                .matches_set_inline(epsilons.looks(), input.haystack(), at)
        {
            return false;
        }
        let pid = pateps.pattern_id_unchecked();
        let slot_end = pid.as_usize().wrapping_mul(2).wrapping_add(1);
        if slot_end < slots.len() {
            slots[slot_end] = NonMaxUsize::new(at);
        }
        if self.explicit_slot_start < slots.len() {
            let cache_slots = cache.explicit_slots();
            slots[self.explicit_slot_start..][..cache_slots.len()].copy_from_slice(cache_slots);
            epsilons
                .slots()
                .apply(at, &mut slots[self.explicit_slot_start..]);
        }
        *matched_pid = Some(pid);
        true
    }
}
impl DFA {
    fn start(&self) -> StateID {
        self.starts[0]
    }
    fn start_pattern(&self, pid: PatternID) -> Result<StateID, MatchError> {
        if !self.config.get_starts_for_each_pattern() {
            return Err(MatchError::unsupported_anchored(Anchored::Pattern(pid)));
        }
        Ok(self.starts.get(pid.one_more()).copied().unwrap_or(DEAD))
    }
    fn transition(&self, sid: StateID, byte: u8) -> Transition {
        let offset = sid.as_usize() << self.stride2();
        let class = self.classes.get(byte).as_usize();
        self.table[offset + class]
    }
    fn set_transition(&mut self, sid: StateID, byte: u8, to: Transition) {
        let offset = sid.as_usize() << self.stride2();
        let class = self.classes.get(byte).as_usize();
        self.table[offset + class] = to;
    }
    fn sparse_transitions(&self, sid: StateID) -> SparseTransitionIter<'_> {
        let start = sid.as_usize() << self.stride2();
        let end = start + self.alphabet_len();
        SparseTransitionIter {
            it: self.table[start..end].iter().enumerate(),
            cur: None,
        }
    }
    fn pattern_epsilons(&self, sid: StateID) -> PatternEpsilons {
        let offset = sid.as_usize() << self.stride2();
        PatternEpsilons(self.table[offset + self.pateps_offset].0)
    }
    fn set_pattern_epsilons(&mut self, sid: StateID, pateps: PatternEpsilons) {
        let offset = sid.as_usize() << self.stride2();
        self.table[offset + self.pateps_offset] = Transition(pateps.0);
    }
    fn prev_state_id(&self, id: StateID) -> Option<StateID> {
        if id == DEAD {
            None
        } else {
            Some(StateID::new_unchecked(
                id.as_usize().checked_sub(1).unwrap(),
            ))
        }
    }
    fn last_state_id(&self) -> StateID {
        StateID::new_unchecked((self.table.len() >> self.stride2()).checked_sub(1).unwrap())
    }
    pub(super) fn swap_states(&mut self, id1: StateID, id2: StateID) {
        let o1 = id1.as_usize() << self.stride2();
        let o2 = id2.as_usize() << self.stride2();
        for b in 0..self.stride() {
            self.table.swap(o1 + b, o2 + b);
        }
    }
    pub(super) fn remap(&mut self, map: impl Fn(StateID) -> StateID) {
        for i in 0..self.state_len() {
            let offset = i << self.stride2();
            for b in 0..self.alphabet_len() {
                let next = self.table[offset + b].state_id();
                self.table[offset + b].set_state_id(map(next));
            }
        }
        for i in 0..self.starts.len() {
            self.starts[i] = map(self.starts[i]);
        }
    }
}
impl core::fmt::Debug for DFA {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        fn debug_state_transitions(
            f: &mut core::fmt::Formatter,
            dfa: &DFA,
            sid: StateID,
        ) -> core::fmt::Result {
            for (i, (start, end, trans)) in dfa.sparse_transitions(sid).enumerate() {
                let next = trans.state_id();
                if i > 0 {
                    write!(f, ", ")?;
                }
                if start == end {
                    write!(f, "{:?} => {:?}", DebugByte(start), next.as_usize(),)?;
                } else {
                    write!(
                        f,
                        "{:?}-{:?} => {:?}",
                        DebugByte(start),
                        DebugByte(end),
                        next.as_usize(),
                    )?;
                }
                if trans.match_wins() {
                    write!(f, " (MW)")?;
                }
                if !trans.epsilons().is_empty() {
                    write!(f, " ({:?})", trans.epsilons())?;
                }
            }
            Ok(())
        }
        writeln!(f, "onepass::DFA(")?;
        for index in 0..self.state_len() {
            let sid = StateID::must(index);
            let pateps = self.pattern_epsilons(sid);
            if sid == DEAD {
                write!(f, "D ")?;
            } else if pateps.pattern_id().is_some() {
                write!(f, "* ")?;
            } else {
                write!(f, "  ")?;
            }
            write!(f, "{:06?}", sid.as_usize())?;
            if !pateps.is_empty() {
                write!(f, " ({pateps:?})")?;
            }
            write!(f, ": ")?;
            debug_state_transitions(f, self, sid)?;
            write!(f, "\n")?;
        }
        writeln!(f, "")?;
        for (i, &sid) in self.starts.iter().enumerate() {
            if i == 0 {
                writeln!(f, "START(ALL): {:?}", sid.as_usize())?;
            } else {
                writeln!(f, "START(pattern: {:?}): {:?}", i - 1, sid.as_usize(),)?;
            }
        }
        writeln!(f, "state length: {:?}", self.state_len())?;
        writeln!(f, "pattern length: {:?}", self.pattern_len())?;
        writeln!(f, ")")?;
        Ok(())
    }
}
#[derive(Debug)]
struct SparseTransitionIter<'a> {
    it: core::iter::Enumerate<core::slice::Iter<'a, Transition>>,
    cur: Option<(u8, u8, Transition)>,
}
impl<'a> Iterator for SparseTransitionIter<'a> {
    type Item = (u8, u8, Transition);
    fn next(&mut self) -> Option<(u8, u8, Transition)> {
        while let Some((b, &trans)) = self.it.next() {
            let b = b.as_u8();
            let (prev_start, prev_end, prev_trans) = match self.cur {
                Some(t) => t,
                None => {
                    self.cur = Some((b, b, trans));
                    continue;
                }
            };
            if prev_trans == trans {
                self.cur = Some((prev_start, b, prev_trans));
            } else {
                self.cur = Some((b, b, trans));
                if prev_trans.state_id() != DEAD {
                    return Some((prev_start, prev_end, prev_trans));
                }
            }
        }
        if let Some((start, end, trans)) = self.cur.take() {
            if trans.state_id() != DEAD {
                return Some((start, end, trans));
            }
        }
        None
    }
}
#[derive(Clone, Debug)]
pub struct Cache {
    explicit_slots: Vec<Option<NonMaxUsize>>,
    explicit_slot_len: usize,
}
impl Cache {
    pub fn new(re: &DFA) -> Cache {
        let mut cache = Cache {
            explicit_slots: vec![],
            explicit_slot_len: 0,
        };
        cache.reset(re);
        cache
    }
    pub fn reset(&mut self, re: &DFA) {
        let explicit_slot_len = re.get_nfa().group_info().explicit_slot_len();
        self.explicit_slots.resize(explicit_slot_len, None);
        self.explicit_slot_len = explicit_slot_len;
    }
    pub fn memory_usage(&self) -> usize {
        self.explicit_slots.len() * core::mem::size_of::<Option<NonMaxUsize>>()
    }
    fn explicit_slots(&mut self) -> &mut [Option<NonMaxUsize>] {
        &mut self.explicit_slots[..self.explicit_slot_len]
    }
    fn setup_search(&mut self, explicit_slot_len: usize) {
        self.explicit_slot_len = explicit_slot_len;
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct Transition(u64);
impl Transition {
    const STATE_ID_BITS: u64 = 21;
    const STATE_ID_SHIFT: u64 = 64 - Transition::STATE_ID_BITS;
    const STATE_ID_LIMIT: u64 = 1 << Transition::STATE_ID_BITS;
    const MATCH_WINS_SHIFT: u64 = 64 - (Transition::STATE_ID_BITS + 1);
    const INFO_MASK: u64 = 0x000003FF_FFFFFFFF;
    fn new(match_wins: bool, sid: StateID, epsilons: Epsilons) -> Transition {
        let match_wins = if match_wins {
            1 << Transition::MATCH_WINS_SHIFT
        } else {
            0
        };
        let sid = sid.as_u64() << Transition::STATE_ID_SHIFT;
        Transition(sid | match_wins | epsilons.0)
    }
    fn is_dead(self) -> bool {
        self.state_id() == DEAD
    }
    fn match_wins(&self) -> bool {
        (self.0 >> Transition::MATCH_WINS_SHIFT & 1) == 1
    }
    fn state_id(&self) -> StateID {
        StateID::new_unchecked((self.0 >> Transition::STATE_ID_SHIFT).as_usize())
    }
    fn set_state_id(&mut self, sid: StateID) {
        *self = Transition::new(self.match_wins(), sid, self.epsilons());
    }
    fn epsilons(&self) -> Epsilons {
        Epsilons(self.0 & Transition::INFO_MASK)
    }
}
impl core::fmt::Debug for Transition {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.is_dead() {
            return write!(f, "0");
        }
        write!(f, "{}", self.state_id().as_usize())?;
        if self.match_wins() {
            write!(f, "-MW")?;
        }
        if !self.epsilons().is_empty() {
            write!(f, "-{:?}", self.epsilons())?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
struct PatternEpsilons(u64);
impl PatternEpsilons {
    const PATTERN_ID_BITS: u64 = 22;
    const PATTERN_ID_SHIFT: u64 = 64 - PatternEpsilons::PATTERN_ID_BITS;
    const PATTERN_ID_NONE: u64 = 0x00000000_003FFFFF;
    const PATTERN_ID_LIMIT: u64 = PatternEpsilons::PATTERN_ID_NONE;
    const PATTERN_ID_MASK: u64 = 0xFFFFFC00_00000000;
    const EPSILONS_MASK: u64 = 0x000003FF_FFFFFFFF;
    fn empty() -> PatternEpsilons {
        PatternEpsilons(PatternEpsilons::PATTERN_ID_NONE << PatternEpsilons::PATTERN_ID_SHIFT)
    }
    fn is_empty(self) -> bool {
        self.pattern_id().is_none() && self.epsilons().is_empty()
    }
    fn pattern_id(self) -> Option<PatternID> {
        let pid = self.0 >> PatternEpsilons::PATTERN_ID_SHIFT;
        if pid == PatternEpsilons::PATTERN_ID_LIMIT {
            None
        } else {
            Some(PatternID::new_unchecked(pid.as_usize()))
        }
    }
    fn pattern_id_unchecked(self) -> PatternID {
        let pid = self.0 >> PatternEpsilons::PATTERN_ID_SHIFT;
        PatternID::new_unchecked(pid.as_usize())
    }
    fn set_pattern_id(self, pid: PatternID) -> PatternEpsilons {
        PatternEpsilons(
            (pid.as_u64() << PatternEpsilons::PATTERN_ID_SHIFT)
                | (self.0 & PatternEpsilons::EPSILONS_MASK),
        )
    }
    fn epsilons(self) -> Epsilons {
        Epsilons(self.0 & PatternEpsilons::EPSILONS_MASK)
    }
    fn set_epsilons(self, epsilons: Epsilons) -> PatternEpsilons {
        PatternEpsilons(
            (self.0 & PatternEpsilons::PATTERN_ID_MASK)
                | (u64::from(epsilons.0) & PatternEpsilons::EPSILONS_MASK),
        )
    }
}
impl core::fmt::Debug for PatternEpsilons {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.is_empty() {
            return write!(f, "N/A");
        }
        if let Some(pid) = self.pattern_id() {
            write!(f, "{}", pid.as_usize())?;
        }
        if !self.epsilons().is_empty() {
            if self.pattern_id().is_some() {
                write!(f, "/")?;
            }
            write!(f, "{:?}", self.epsilons())?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
struct Epsilons(u64);
impl Epsilons {
    const SLOT_MASK: u64 = 0x000003FF_FFFFFC00;
    const SLOT_SHIFT: u64 = 10;
    const LOOK_MASK: u64 = 0x00000000_000003FF;
    fn empty() -> Epsilons {
        Epsilons(0)
    }
    fn is_empty(self) -> bool {
        self.0 == 0
    }
    fn slots(self) -> Slots {
        Slots((self.0 >> Epsilons::SLOT_SHIFT).low_u32())
    }
    fn set_slots(self, slots: Slots) -> Epsilons {
        Epsilons((u64::from(slots.0) << Epsilons::SLOT_SHIFT) | (self.0 & Epsilons::LOOK_MASK))
    }
    fn looks(self) -> LookSet {
        LookSet {
            bits: (self.0 & Epsilons::LOOK_MASK).low_u32(),
        }
    }
    fn set_looks(self, look_set: LookSet) -> Epsilons {
        Epsilons((self.0 & Epsilons::SLOT_MASK) | (u64::from(look_set.bits) & Epsilons::LOOK_MASK))
    }
}
impl core::fmt::Debug for Epsilons {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut wrote = false;
        if !self.slots().is_empty() {
            write!(f, "{:?}", self.slots())?;
            wrote = true;
        }
        if !self.looks().is_empty() {
            if wrote {
                write!(f, "/")?;
            }
            write!(f, "{:?}", self.looks())?;
            wrote = true;
        }
        if !wrote {
            write!(f, "N/A")?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
struct Slots(u32);
impl Slots {
    const LIMIT: usize = 32;
    fn insert(self, slot: usize) -> Slots {
        debug_assert!(slot < Slots::LIMIT);
        Slots(self.0 | (1 << slot.as_u32()))
    }
    fn remove(self, slot: usize) -> Slots {
        debug_assert!(slot < Slots::LIMIT);
        Slots(self.0 & !(1 << slot.as_u32()))
    }
    fn is_empty(self) -> bool {
        self.0 == 0
    }
    fn iter(self) -> SlotsIter {
        SlotsIter { slots: self }
    }
    fn apply(self, at: usize, caller_explicit_slots: &mut [Option<NonMaxUsize>]) {
        if self.is_empty() {
            return;
        }
        let at = NonMaxUsize::new(at);
        for slot in self.iter() {
            if slot >= caller_explicit_slots.len() {
                break;
            }
            caller_explicit_slots[slot] = at;
        }
    }
}
impl core::fmt::Debug for Slots {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "S")?;
        for slot in self.iter() {
            write!(f, "-{slot:?}")?;
        }
        Ok(())
    }
}
#[derive(Debug)]
struct SlotsIter {
    slots: Slots,
}
impl Iterator for SlotsIter {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        let slot = self.slots.0.trailing_zeros().as_usize();
        if slot >= Slots::LIMIT {
            return None;
        }
        self.slots = self.slots.remove(slot);
        Some(slot)
    }
}
#[derive(Clone, Debug)]
pub struct BuildError {
    kind: BuildErrorKind,
}
#[derive(Clone, Debug)]
enum BuildErrorKind {
    NFA(crate::regex_engine::automata::nfa::thompson::BuildError),
    Word(UnicodeWordBoundaryError),
    TooManyStates { limit: u64 },
    TooManyPatterns { limit: u64 },
    UnsupportedLook { look: Look },
    ExceededSizeLimit { limit: usize },
    NotOnePass { msg: &'static str },
}
impl BuildError {
    fn nfa(err: crate::regex_engine::automata::nfa::thompson::BuildError) -> BuildError {
        BuildError {
            kind: BuildErrorKind::NFA(err),
        }
    }
    fn word(err: UnicodeWordBoundaryError) -> BuildError {
        BuildError {
            kind: BuildErrorKind::Word(err),
        }
    }
    fn too_many_states(limit: u64) -> BuildError {
        BuildError {
            kind: BuildErrorKind::TooManyStates { limit },
        }
    }
    fn too_many_patterns(limit: u64) -> BuildError {
        BuildError {
            kind: BuildErrorKind::TooManyPatterns { limit },
        }
    }
    fn unsupported_look(look: Look) -> BuildError {
        BuildError {
            kind: BuildErrorKind::UnsupportedLook { look },
        }
    }
    fn exceeded_size_limit(limit: usize) -> BuildError {
        BuildError {
            kind: BuildErrorKind::ExceededSizeLimit { limit },
        }
    }
    fn not_one_pass(msg: &'static str) -> BuildError {
        BuildError {
            kind: BuildErrorKind::NotOnePass { msg },
        }
    }
}
impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        use self::BuildErrorKind::*;
        match self.kind {
            NFA(ref err) => Some(err),
            Word(ref err) => Some(err),
            _ => None,
        }
    }
}
impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use self::BuildErrorKind::*;
        match self.kind {
            NFA(_) => write!(f, "error building NFA"),
            Word(_) => write!(f, "NFA contains Unicode word boundary"),
            TooManyStates { limit } => write!(
                f,
                "one-pass DFA exceeded a limit of {limit:?} \
                 for number of states",
            ),
            TooManyPatterns { limit } => write!(
                f,
                "one-pass DFA exceeded a limit of {limit:?} \
                 for number of patterns",
            ),
            UnsupportedLook { look } => {
                write!(f, "one-pass DFA does not support the {look:?} assertion",)
            }
            ExceededSizeLimit { limit } => write!(
                f,
                "one-pass DFA exceeded size limit of {limit:?} during building",
            ),
            NotOnePass { msg } => write!(
                f,
                "one-pass DFA could not be built because \
                 pattern is not one-pass: {}",
                msg,
            ),
        }
    }
}
