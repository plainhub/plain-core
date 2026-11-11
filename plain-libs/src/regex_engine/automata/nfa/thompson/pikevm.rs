use crate::regex_engine::automata::{
    nfa::thompson::{self, BuildError, NFA, State},
    util::{
        captures::Captures,
        empty, iter,
        prefilter::Prefilter,
        primitives::{NonMaxUsize, PatternID, SmallIndex, StateID},
        search::{Anchored, HalfMatch, Input, Match, MatchKind, PatternSet, Span},
        sparse_set::SparseSet,
    },
};
use std::{vec, vec::Vec};
macro_rules! instrument {
    ($ fun : expr) => {};
}
#[derive(Clone, Debug, Default)]
pub struct Config {
    match_kind: Option<MatchKind>,
    pre: Option<Option<Prefilter>>,
}
impl Config {
    pub fn new() -> Config {
        Config::default()
    }
    pub fn match_kind(mut self, kind: MatchKind) -> Config {
        self.match_kind = Some(kind);
        self
    }
    pub fn prefilter(mut self, pre: Option<Prefilter>) -> Config {
        self.pre = Some(pre);
        self
    }
    pub fn get_match_kind(&self) -> MatchKind {
        self.match_kind.unwrap_or(MatchKind::LeftmostFirst)
    }
    pub fn get_prefilter(&self) -> Option<&Prefilter> {
        self.pre.as_ref().unwrap_or(&None).as_ref()
    }
    pub(crate) fn overwrite(&self, o: Config) -> Config {
        Config {
            match_kind: o.match_kind.or(self.match_kind),
            pre: o.pre.or_else(|| self.pre.clone()),
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
    pub fn build(&self, pattern: &str) -> Result<PikeVM, BuildError> {
        self.build_many(&[pattern])
    }
    pub fn build_many<P: AsRef<str>>(&self, patterns: &[P]) -> Result<PikeVM, BuildError> {
        let nfa = self.thompson.build_many(patterns)?;
        self.build_from_nfa(nfa)
    }
    pub fn build_from_nfa(&self, nfa: NFA) -> Result<PikeVM, BuildError> {
        nfa.look_set_any().available().map_err(BuildError::word)?;
        Ok(PikeVM {
            config: self.config.clone(),
            nfa,
        })
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
#[derive(Clone, Debug)]
pub struct PikeVM {
    config: Config,
    nfa: NFA,
}
impl PikeVM {
    pub fn new(pattern: &str) -> Result<PikeVM, BuildError> {
        PikeVM::builder().build(pattern)
    }
    pub fn new_many<P: AsRef<str>>(patterns: &[P]) -> Result<PikeVM, BuildError> {
        PikeVM::builder().build_many(patterns)
    }
    pub fn new_from_nfa(nfa: NFA) -> Result<PikeVM, BuildError> {
        PikeVM::builder().build_from_nfa(nfa)
    }
    pub fn always_match() -> Result<PikeVM, BuildError> {
        let nfa = thompson::NFA::always_match();
        PikeVM::new_from_nfa(nfa)
    }
    pub fn never_match() -> Result<PikeVM, BuildError> {
        let nfa = thompson::NFA::never_match();
        PikeVM::new_from_nfa(nfa)
    }
    pub fn config() -> Config {
        Config::new()
    }
    pub fn builder() -> Builder {
        Builder::new()
    }
    pub fn create_captures(&self) -> Captures {
        Captures::all(self.get_nfa().group_info().clone())
    }
    pub fn create_cache(&self) -> Cache {
        Cache::new(self)
    }
    pub fn reset_cache(&self, cache: &mut Cache) {
        cache.reset(self);
    }
    pub fn pattern_len(&self) -> usize {
        self.nfa.pattern_len()
    }
    #[inline]
    pub fn get_config(&self) -> &Config {
        &self.config
    }
    #[inline]
    pub fn get_nfa(&self) -> &NFA {
        &self.nfa
    }
}
impl PikeVM {
    #[inline]
    pub fn is_match<'h, I: Into<Input<'h>>>(&self, cache: &mut Cache, input: I) -> bool {
        let input = input.into().earliest(true);
        self.search_slots(cache, &input, &mut []).is_some()
    }
    #[inline]
    pub fn find<'h, I: Into<Input<'h>>>(&self, cache: &mut Cache, input: I) -> Option<Match> {
        let input = input.into();
        if self.get_nfa().pattern_len() == 1 {
            let mut slots = [None, None];
            let pid = self.search_slots(cache, &input, &mut slots)?;
            let start = slots[0]?.get();
            let end = slots[1]?.get();
            return Some(Match::new(pid, Span { start, end }));
        }
        let ginfo = self.get_nfa().group_info();
        let slots_len = ginfo.implicit_slot_len();
        let mut slots = vec![None; slots_len];
        let pid = self.search_slots(cache, &input, &mut slots)?;
        let start = slots[pid.as_usize() * 2]?.get();
        let end = slots[pid.as_usize() * 2 + 1]?.get();
        Some(Match::new(pid, Span { start, end }))
    }
    #[inline]
    pub fn captures<'h, I: Into<Input<'h>>>(
        &self,
        cache: &mut Cache,
        input: I,
        caps: &mut Captures,
    ) {
        self.search(cache, &input.into(), caps)
    }
    #[inline]
    pub fn find_iter<'r, 'c, 'h, I: Into<Input<'h>>>(
        &'r self,
        cache: &'c mut Cache,
        input: I,
    ) -> FindMatches<'r, 'c, 'h> {
        let caps = Captures::matches(self.get_nfa().group_info().clone());
        let it = iter::Searcher::new(input.into());
        FindMatches {
            re: self,
            cache,
            caps,
            it,
        }
    }
    #[inline]
    pub fn captures_iter<'r, 'c, 'h, I: Into<Input<'h>>>(
        &'r self,
        cache: &'c mut Cache,
        input: I,
    ) -> CapturesMatches<'r, 'c, 'h> {
        let caps = self.create_captures();
        let it = iter::Searcher::new(input.into());
        CapturesMatches {
            re: self,
            cache,
            caps,
            it,
        }
    }
}
impl PikeVM {
    #[inline]
    pub fn search(&self, cache: &mut Cache, input: &Input<'_>, caps: &mut Captures) {
        caps.set_pattern(None);
        let pid = self.search_slots(cache, input, caps.slots_mut());
        caps.set_pattern(pid);
    }
    #[inline]
    pub fn search_slots(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        if !utf8empty {
            let hm = self.search_slots_imp(cache, input, slots)?;
            return Some(hm.pattern());
        }
        let min = self.get_nfa().group_info().implicit_slot_len();
        if slots.len() >= min {
            let hm = self.search_slots_imp(cache, input, slots)?;
            return Some(hm.pattern());
        }
        if self.get_nfa().pattern_len() == 1 {
            let mut enough = [None, None];
            let got = self.search_slots_imp(cache, input, &mut enough);
            slots.copy_from_slice(&enough[..slots.len()]);
            return got.map(|hm| hm.pattern());
        }
        let mut enough = vec![None; min];
        let got = self.search_slots_imp(cache, input, &mut enough);
        slots.copy_from_slice(&enough[..slots.len()]);
        got.map(|hm| hm.pattern())
    }
    #[inline(never)]
    fn search_slots_imp(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<HalfMatch> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        let hm = match self.search_imp(cache, input, slots) {
            None => return None,
            Some(hm) if !utf8empty => return Some(hm),
            Some(hm) => hm,
        };
        empty::skip_splits_fwd(input, hm, hm.offset(), |input| {
            Ok(self
                .search_imp(cache, input, slots)
                .map(|hm| (hm, hm.offset())))
        })
        .unwrap()
    }
    #[inline]
    pub fn which_overlapping_matches(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) {
        self.which_overlapping_imp(cache, input, patset)
    }
}
impl PikeVM {
    fn search_imp(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<HalfMatch> {
        cache.setup_search(slots.len());
        if input.is_done() {
            return None;
        }
        assert!(
            input.haystack().len() < core::usize::MAX,
            "byte slice lengths must be less than usize MAX",
        );
        instrument!(|c| c.reset(&self.nfa));
        let allmatches = self.config.get_match_kind().continue_past_first_match();
        let (anchored, start_id) = match self.start_config(input) {
            None => return None,
            Some(config) => config,
        };
        let pre = if anchored {
            None
        } else {
            self.get_config().get_prefilter()
        };
        let Cache { stack, curr, next } = cache;
        let mut hm = None;
        let mut at = input.start();
        while at <= input.end() {
            if curr.set.is_empty() {
                if hm.is_some() && !allmatches {
                    break;
                }
                if anchored && at > input.start() {
                    break;
                }
                if let Some(pre) = pre {
                    let span = Span::from(at..input.end());
                    match pre.find(input.haystack(), span) {
                        None => break,
                        Some(ref span) => at = span.start,
                    }
                }
            }
            if (hm.is_none() || allmatches) && (!anchored || at == input.start()) {
                let slots = next.slot_table.all_absent();
                self.epsilon_closure(stack, slots, curr, input, at, start_id);
            }
            if let Some(pid) = self.nexts(stack, curr, next, input, at, slots) {
                hm = Some(HalfMatch::new(pid, at));
            }
            if input.get_earliest() && hm.is_some() {
                break;
            }
            core::mem::swap(curr, next);
            next.set.clear();
            at += 1;
        }
        instrument!(|c| c.eprint(&self.nfa));
        hm
    }
    fn which_overlapping_imp(&self, cache: &mut Cache, input: &Input<'_>, patset: &mut PatternSet) {
        cache.setup_search(0);
        if input.is_done() {
            return;
        }
        assert!(
            input.haystack().len() < core::usize::MAX,
            "byte slice lengths must be less than usize MAX",
        );
        instrument!(|c| c.reset(&self.nfa));
        let allmatches = self.config.get_match_kind().continue_past_first_match();
        let (anchored, start_id) = match self.start_config(input) {
            None => return,
            Some(config) => config,
        };
        let Cache { stack, curr, next } = cache;
        for at in input.start()..=input.end() {
            let any_matches = !patset.is_empty();
            if curr.set.is_empty() {
                if any_matches && !allmatches {
                    break;
                }
                if anchored && at > input.start() {
                    break;
                }
            }
            if !any_matches || allmatches {
                let slots = &mut [];
                self.epsilon_closure(stack, slots, curr, input, at, start_id);
            }
            self.nexts_overlapping(stack, curr, next, input, at, patset);
            if patset.is_full() || input.get_earliest() {
                break;
            }
            core::mem::swap(curr, next);
            next.set.clear();
        }
        instrument!(|c| c.eprint(&self.nfa));
    }
    #[cfg_attr(all(), inline(always))]
    fn nexts(
        &self,
        stack: &mut Vec<FollowEpsilon>,
        curr: &mut ActiveStates,
        next: &mut ActiveStates,
        input: &Input<'_>,
        at: usize,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        instrument!(|c| c.record_state_set(&curr.set));
        let mut pid = None;
        let ActiveStates {
            ref set,
            ref mut slot_table,
        } = *curr;
        for sid in set.iter() {
            pid = match self.next(stack, slot_table, next, input, at, sid) {
                None => continue,
                Some(pid) => Some(pid),
            };
            slots.copy_from_slice(slot_table.for_state(sid));
            if !self.config.get_match_kind().continue_past_first_match() {
                break;
            }
        }
        pid
    }
    #[cfg_attr(all(), inline(always))]
    fn nexts_overlapping(
        &self,
        stack: &mut Vec<FollowEpsilon>,
        curr: &mut ActiveStates,
        next: &mut ActiveStates,
        input: &Input<'_>,
        at: usize,
        patset: &mut PatternSet,
    ) {
        instrument!(|c| c.record_state_set(&curr.set));
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        let ActiveStates {
            ref set,
            ref mut slot_table,
        } = *curr;
        for sid in set.iter() {
            let pid = match self.next(stack, slot_table, next, input, at, sid) {
                None => continue,
                Some(pid) => pid,
            };
            if utf8empty && !input.is_char_boundary(at) {
                continue;
            }
            let _ = patset.try_insert(pid);
            if !self.config.get_match_kind().continue_past_first_match() {
                break;
            }
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn next(
        &self,
        stack: &mut Vec<FollowEpsilon>,
        curr_slot_table: &mut SlotTable,
        next: &mut ActiveStates,
        input: &Input<'_>,
        at: usize,
        sid: StateID,
    ) -> Option<PatternID> {
        instrument!(|c| c.record_step(sid));
        match *self.nfa.state(sid) {
            State::Fail
            | State::Look { .. }
            | State::Union { .. }
            | State::BinaryUnion { .. }
            | State::Capture { .. } => None,
            State::ByteRange { ref trans } => {
                if trans.matches(input.haystack(), at) {
                    let slots = curr_slot_table.for_state(sid);
                    let at = at.wrapping_add(1);
                    self.epsilon_closure(stack, slots, next, input, at, trans.next);
                }
                None
            }
            State::Sparse(ref sparse) => {
                if let Some(next_sid) = sparse.matches(input.haystack(), at) {
                    let slots = curr_slot_table.for_state(sid);
                    let at = at.wrapping_add(1);
                    self.epsilon_closure(stack, slots, next, input, at, next_sid);
                }
                None
            }
            State::Dense(ref dense) => {
                if let Some(next_sid) = dense.matches(input.haystack(), at) {
                    let slots = curr_slot_table.for_state(sid);
                    let at = at.wrapping_add(1);
                    self.epsilon_closure(stack, slots, next, input, at, next_sid);
                }
                None
            }
            State::Match { pattern_id } => Some(pattern_id),
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn epsilon_closure(
        &self,
        stack: &mut Vec<FollowEpsilon>,
        curr_slots: &mut [Option<NonMaxUsize>],
        next: &mut ActiveStates,
        input: &Input<'_>,
        at: usize,
        sid: StateID,
    ) {
        instrument!(|c| {
            c.record_closure(sid);
            c.record_stack_push(sid);
        });
        stack.push(FollowEpsilon::Explore(sid));
        while let Some(frame) = stack.pop() {
            match frame {
                FollowEpsilon::RestoreCapture { slot, offset: pos } => {
                    curr_slots[slot] = pos;
                }
                FollowEpsilon::Explore(sid) => {
                    self.epsilon_closure_explore(stack, curr_slots, next, input, at, sid);
                }
            }
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn epsilon_closure_explore(
        &self,
        stack: &mut Vec<FollowEpsilon>,
        curr_slots: &mut [Option<NonMaxUsize>],
        next: &mut ActiveStates,
        input: &Input<'_>,
        at: usize,
        mut sid: StateID,
    ) {
        loop {
            instrument!(|c| c.record_set_insert(sid));
            if !next.set.insert(sid) {
                return;
            }
            match *self.nfa.state(sid) {
                State::Fail
                | State::Match { .. }
                | State::ByteRange { .. }
                | State::Sparse { .. }
                | State::Dense { .. } => {
                    next.slot_table.for_state(sid).copy_from_slice(curr_slots);
                    return;
                }
                State::Look { look, next } => {
                    if !self
                        .nfa
                        .look_matcher()
                        .matches_inline(look, input.haystack(), at)
                    {
                        return;
                    }
                    sid = next;
                }
                State::Union { ref alternates } => {
                    sid = match alternates.get(0) {
                        None => return,
                        Some(&sid) => sid,
                    };
                    instrument!(|c| {
                        for &alt in &alternates[1..] {
                            c.record_stack_push(alt);
                        }
                    });
                    stack.extend(
                        alternates[1..]
                            .iter()
                            .copied()
                            .rev()
                            .map(FollowEpsilon::Explore),
                    );
                }
                State::BinaryUnion { alt1, alt2 } => {
                    sid = alt1;
                    instrument!(|c| c.record_stack_push(sid));
                    stack.push(FollowEpsilon::Explore(alt2));
                }
                State::Capture { next, slot, .. } => {
                    if slot.as_usize() < curr_slots.len() {
                        instrument!(|c| c.record_stack_push(sid));
                        stack.push(FollowEpsilon::RestoreCapture {
                            slot,
                            offset: curr_slots[slot],
                        });
                        curr_slots[slot] = Some(NonMaxUsize::new(at).unwrap());
                    }
                    sid = next;
                }
            }
        }
    }
    fn start_config(&self, input: &Input<'_>) -> Option<(bool, StateID)> {
        match input.get_anchored() {
            Anchored::No => Some((
                self.nfa.is_always_start_anchored(),
                self.nfa.start_anchored(),
            )),
            Anchored::Yes => Some((true, self.nfa.start_anchored())),
            Anchored::Pattern(pid) => Some((true, self.nfa.start_pattern(pid)?)),
        }
    }
}
#[derive(Debug)]
pub struct FindMatches<'r, 'c, 'h> {
    re: &'r PikeVM,
    cache: &'c mut Cache,
    caps: Captures,
    it: iter::Searcher<'h>,
}
impl<'r, 'c, 'h> Iterator for FindMatches<'r, 'c, 'h> {
    type Item = Match;
    #[inline]
    fn next(&mut self) -> Option<Match> {
        let FindMatches {
            re,
            ref mut cache,
            ref mut caps,
            ref mut it,
        } = *self;
        it.advance(|input| {
            re.search(cache, input, caps);
            Ok(caps.get_match())
        })
    }
}
#[derive(Debug)]
pub struct CapturesMatches<'r, 'c, 'h> {
    re: &'r PikeVM,
    cache: &'c mut Cache,
    caps: Captures,
    it: iter::Searcher<'h>,
}
impl<'r, 'c, 'h> Iterator for CapturesMatches<'r, 'c, 'h> {
    type Item = Captures;
    #[inline]
    fn next(&mut self) -> Option<Captures> {
        let CapturesMatches {
            re,
            ref mut cache,
            ref mut caps,
            ref mut it,
        } = *self;
        it.advance(|input| {
            re.search(cache, input, caps);
            Ok(caps.get_match())
        });
        if caps.is_match() {
            Some(caps.clone())
        } else {
            None
        }
    }
}
#[derive(Clone, Debug)]
pub struct Cache {
    stack: Vec<FollowEpsilon>,
    curr: ActiveStates,
    next: ActiveStates,
}
impl Cache {
    pub fn new(re: &PikeVM) -> Cache {
        Cache {
            stack: vec![],
            curr: ActiveStates::new(re),
            next: ActiveStates::new(re),
        }
    }
    pub fn reset(&mut self, re: &PikeVM) {
        self.curr.reset(re);
        self.next.reset(re);
    }
    pub fn memory_usage(&self) -> usize {
        use core::mem::size_of;
        (self.stack.len() * size_of::<FollowEpsilon>())
            + self.curr.memory_usage()
            + self.next.memory_usage()
    }
    fn setup_search(&mut self, captures_slot_len: usize) {
        self.stack.clear();
        self.curr.setup_search(captures_slot_len);
        self.next.setup_search(captures_slot_len);
    }
}
#[derive(Clone, Debug)]
struct ActiveStates {
    set: SparseSet,
    slot_table: SlotTable,
}
impl ActiveStates {
    fn new(re: &PikeVM) -> ActiveStates {
        let mut active = ActiveStates {
            set: SparseSet::new(0),
            slot_table: SlotTable::new(),
        };
        active.reset(re);
        active
    }
    fn reset(&mut self, re: &PikeVM) {
        self.set.resize(re.get_nfa().states().len());
        self.slot_table.reset(re);
    }
    fn memory_usage(&self) -> usize {
        self.set.memory_usage() + self.slot_table.memory_usage()
    }
    fn setup_search(&mut self, captures_slot_len: usize) {
        self.set.clear();
        self.slot_table.setup_search(captures_slot_len);
    }
}
#[derive(Clone, Debug)]
struct SlotTable {
    table: Vec<Option<NonMaxUsize>>,
    slots_per_state: usize,
    slots_for_captures: usize,
}
impl SlotTable {
    fn new() -> SlotTable {
        SlotTable {
            table: vec![],
            slots_for_captures: 0,
            slots_per_state: 0,
        }
    }
    fn reset(&mut self, re: &PikeVM) {
        let nfa = re.get_nfa();
        self.slots_per_state = nfa.group_info().slot_len();
        self.slots_for_captures = core::cmp::max(
            self.slots_per_state,
            nfa.pattern_len().checked_mul(2).unwrap(),
        );
        let len = nfa
            .states()
            .len()
            .checked_mul(self.slots_per_state)
            .and_then(|x| x.checked_add(self.slots_for_captures))
            .expect("slot table length doesn't overflow");
        trace!(
            "resizing PikeVM active states table to {} entries \
             (slots_per_state={})",
            len, self.slots_per_state,
        );
        self.table.resize(len, None);
    }
    fn memory_usage(&self) -> usize {
        self.table.len() * core::mem::size_of::<Option<NonMaxUsize>>()
    }
    fn setup_search(&mut self, captures_slot_len: usize) {
        self.slots_for_captures = captures_slot_len;
    }
    fn for_state(&mut self, sid: StateID) -> &mut [Option<NonMaxUsize>] {
        let i = sid.as_usize() * self.slots_per_state;
        &mut self.table[i..i + self.slots_for_captures]
    }
    fn all_absent(&mut self) -> &mut [Option<NonMaxUsize>] {
        let i = self.table.len() - self.slots_for_captures;
        &mut self.table[i..i + self.slots_for_captures]
    }
}
#[derive(Clone, Debug)]
enum FollowEpsilon {
    Explore(StateID),
    RestoreCapture {
        slot: SmallIndex,
        offset: Option<NonMaxUsize>,
    },
}
