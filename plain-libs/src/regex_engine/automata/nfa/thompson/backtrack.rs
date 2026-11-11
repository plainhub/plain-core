use crate::regex_engine::automata::{
    nfa::thompson::{self, BuildError, NFA, State},
    util::{
        captures::Captures,
        empty, iter,
        prefilter::Prefilter,
        primitives::{NonMaxUsize, PatternID, SmallIndex, StateID},
        search::{Anchored, HalfMatch, Input, Match, MatchError, Span},
    },
};
use std::{vec, vec::Vec};

#[derive(Clone, Debug, Default)]
pub struct Config {
    pre: Option<Option<Prefilter>>,
    visited_capacity: Option<usize>,
}
impl Config {
    pub fn new() -> Config {
        Config::default()
    }
    pub fn prefilter(mut self, pre: Option<Prefilter>) -> Config {
        self.pre = Some(pre);
        self
    }
    pub fn visited_capacity(mut self, capacity: usize) -> Config {
        self.visited_capacity = Some(capacity);
        self
    }
    pub fn get_prefilter(&self) -> Option<&Prefilter> {
        self.pre.as_ref().unwrap_or(&None).as_ref()
    }
    pub fn get_visited_capacity(&self) -> usize {
        const DEFAULT: usize = 256 * (1 << 10);
        self.visited_capacity.unwrap_or(DEFAULT)
    }
    pub(crate) fn overwrite(&self, o: Config) -> Config {
        Config {
            pre: o.pre.or_else(|| self.pre.clone()),
            visited_capacity: o.visited_capacity.or(self.visited_capacity),
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
    pub fn build(&self, pattern: &str) -> Result<BoundedBacktracker, BuildError> {
        self.build_many(&[pattern])
    }
    pub fn build_many<P: AsRef<str>>(
        &self,
        patterns: &[P],
    ) -> Result<BoundedBacktracker, BuildError> {
        let nfa = self.thompson.build_many(patterns)?;
        self.build_from_nfa(nfa)
    }
    pub fn build_from_nfa(&self, nfa: NFA) -> Result<BoundedBacktracker, BuildError> {
        nfa.look_set_any().available().map_err(BuildError::word)?;
        Ok(BoundedBacktracker {
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
pub struct BoundedBacktracker {
    config: Config,
    nfa: NFA,
}
impl BoundedBacktracker {
    pub fn new(pattern: &str) -> Result<BoundedBacktracker, BuildError> {
        BoundedBacktracker::builder().build(pattern)
    }
    pub fn new_many<P: AsRef<str>>(patterns: &[P]) -> Result<BoundedBacktracker, BuildError> {
        BoundedBacktracker::builder().build_many(patterns)
    }
    pub fn new_from_nfa(nfa: NFA) -> Result<BoundedBacktracker, BuildError> {
        BoundedBacktracker::builder().build_from_nfa(nfa)
    }
    pub fn always_match() -> Result<BoundedBacktracker, BuildError> {
        let nfa = thompson::NFA::always_match();
        BoundedBacktracker::new_from_nfa(nfa)
    }
    pub fn never_match() -> Result<BoundedBacktracker, BuildError> {
        let nfa = thompson::NFA::never_match();
        BoundedBacktracker::new_from_nfa(nfa)
    }
    pub fn config() -> Config {
        Config::new()
    }
    pub fn builder() -> Builder {
        Builder::new()
    }
    pub fn create_cache(&self) -> Cache {
        Cache::new(self)
    }
    pub fn create_captures(&self) -> Captures {
        Captures::all(self.get_nfa().group_info().clone())
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
    #[inline]
    pub fn max_haystack_len(&self) -> usize {
        let capacity = 8 * self.get_config().get_visited_capacity();
        let blocks = div_ceil(capacity, Visited::BLOCK_SIZE);
        let real_capacity = blocks.saturating_mul(Visited::BLOCK_SIZE);
        (real_capacity / self.nfa.states().len()).saturating_sub(1)
    }
}
impl BoundedBacktracker {
    #[inline]
    pub fn try_is_match<'h, I: Into<Input<'h>>>(
        &self,
        cache: &mut Cache,
        input: I,
    ) -> Result<bool, MatchError> {
        let input = input.into().earliest(true);
        self.try_search_slots(cache, &input, &mut [])
            .map(|pid| pid.is_some())
    }
    #[inline]
    pub fn try_find<'h, I: Into<Input<'h>>>(
        &self,
        cache: &mut Cache,
        input: I,
    ) -> Result<Option<Match>, MatchError> {
        let input = input.into();
        if self.get_nfa().pattern_len() == 1 {
            let mut slots = [None, None];
            let pid = match self.try_search_slots(cache, &input, &mut slots)? {
                None => return Ok(None),
                Some(pid) => pid,
            };
            let start = match slots[0] {
                None => return Ok(None),
                Some(s) => s.get(),
            };
            let end = match slots[1] {
                None => return Ok(None),
                Some(s) => s.get(),
            };
            return Ok(Some(Match::new(pid, Span { start, end })));
        }
        let ginfo = self.get_nfa().group_info();
        let slots_len = ginfo.implicit_slot_len();
        let mut slots = vec![None; slots_len];
        let pid = match self.try_search_slots(cache, &input, &mut slots)? {
            None => return Ok(None),
            Some(pid) => pid,
        };
        let start = match slots[pid.as_usize() * 2] {
            None => return Ok(None),
            Some(s) => s.get(),
        };
        let end = match slots[pid.as_usize() * 2 + 1] {
            None => return Ok(None),
            Some(s) => s.get(),
        };
        Ok(Some(Match::new(pid, Span { start, end })))
    }

    #[inline]
    pub fn try_find_iter<'r, 'c, 'h, I: Into<Input<'h>>>(
        &'r self,
        cache: &'c mut Cache,
        input: I,
    ) -> TryFindMatches<'r, 'c, 'h> {
        let caps = Captures::matches(self.get_nfa().group_info().clone());
        let it = iter::Searcher::new(input.into());
        TryFindMatches {
            re: self,
            cache,
            caps,
            it,
        }
    }
}
impl BoundedBacktracker {
    #[inline]
    pub fn try_search(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        caps: &mut Captures,
    ) -> Result<(), MatchError> {
        caps.set_pattern(None);
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
            let maybe_hm = self.try_search_slots_imp(cache, input, slots)?;
            return Ok(maybe_hm.map(|hm| hm.pattern()));
        }
        let min = self.get_nfa().group_info().implicit_slot_len();
        if slots.len() >= min {
            let maybe_hm = self.try_search_slots_imp(cache, input, slots)?;
            return Ok(maybe_hm.map(|hm| hm.pattern()));
        }
        if self.get_nfa().pattern_len() == 1 {
            let mut enough = [None, None];
            let got = self.try_search_slots_imp(cache, input, &mut enough)?;
            slots.copy_from_slice(&enough[..slots.len()]);
            return Ok(got.map(|hm| hm.pattern()));
        }
        let mut enough = vec![None; min];
        let got = self.try_search_slots_imp(cache, input, &mut enough)?;
        slots.copy_from_slice(&enough[..slots.len()]);
        Ok(got.map(|hm| hm.pattern()))
    }
    #[inline(never)]
    fn try_search_slots_imp(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Result<Option<HalfMatch>, MatchError> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        let hm = match self.search_imp(cache, input, slots)? {
            None => return Ok(None),
            Some(hm) if !utf8empty => return Ok(Some(hm)),
            Some(hm) => hm,
        };
        empty::skip_splits_fwd(input, hm, hm.offset(), |input| {
            Ok(self
                .search_imp(cache, input, slots)?
                .map(|hm| (hm, hm.offset())))
        })
    }
    fn search_imp(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Result<Option<HalfMatch>, MatchError> {
        for slot in slots.iter_mut() {
            *slot = None;
        }
        cache.setup_search(&self, input)?;
        if input.is_done() {
            return Ok(None);
        }
        let (anchored, start_id) = match input.get_anchored() {
            Anchored::No => (
                self.nfa.is_always_start_anchored(),
                self.nfa.start_anchored(),
            ),
            Anchored::Yes => (true, self.nfa.start_anchored()),
            Anchored::Pattern(pid) => match self.nfa.start_pattern(pid) {
                None => return Ok(None),
                Some(sid) => (true, sid),
            },
        };
        if anchored {
            let at = input.start();
            return Ok(self.backtrack(cache, input, at, start_id, slots));
        }
        let pre = self.get_config().get_prefilter();
        let mut at = input.start();
        while at <= input.end() {
            if let Some(ref pre) = pre {
                let span = Span::from(at..input.end());
                match pre.find(input.haystack(), span) {
                    None => break,
                    Some(ref span) => at = span.start,
                }
            }
            if let Some(hm) = self.backtrack(cache, input, at, start_id, slots) {
                return Ok(Some(hm));
            }
            at += 1;
        }
        Ok(None)
    }
    #[cfg_attr(all(), inline(always))]
    fn backtrack(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        at: usize,
        start_id: StateID,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<HalfMatch> {
        cache.stack.push(Frame::Step { sid: start_id, at });
        while let Some(frame) = cache.stack.pop() {
            match frame {
                Frame::Step { sid, at } => {
                    if let Some(hm) = self.step(cache, input, sid, at, slots) {
                        return Some(hm);
                    }
                }
                Frame::RestoreCapture { slot, offset } => {
                    slots[slot] = offset;
                }
            }
        }
        None
    }
    #[cfg_attr(all(), inline(always))]
    fn step(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        mut sid: StateID,
        mut at: usize,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<HalfMatch> {
        loop {
            if !cache.visited.insert(sid, at - input.start()) {
                return None;
            }
            match *self.nfa.state(sid) {
                State::ByteRange { ref trans } => {
                    if at >= input.end() {
                        return None;
                    }
                    if !trans.matches(input.haystack(), at) {
                        return None;
                    }
                    sid = trans.next;
                    at += 1;
                }
                State::Sparse(ref sparse) => {
                    if at >= input.end() {
                        return None;
                    }
                    sid = sparse.matches(input.haystack(), at)?;
                    at += 1;
                }
                State::Dense(ref dense) => {
                    if at >= input.end() {
                        return None;
                    }
                    sid = dense.matches(input.haystack(), at)?;
                    at += 1;
                }
                State::Look { look, next } => {
                    if !self
                        .nfa
                        .look_matcher()
                        .matches_inline(look, input.haystack(), at)
                    {
                        return None;
                    }
                    sid = next;
                }
                State::Union { ref alternates } => {
                    sid = match alternates.get(0) {
                        None => return None,
                        Some(&sid) => sid,
                    };
                    cache.stack.extend(
                        alternates[1..]
                            .iter()
                            .copied()
                            .rev()
                            .map(|sid| Frame::Step { sid, at }),
                    );
                }
                State::BinaryUnion { alt1, alt2 } => {
                    sid = alt1;
                    cache.stack.push(Frame::Step { sid: alt2, at });
                }
                State::Capture { next, slot, .. } => {
                    if slot.as_usize() < slots.len() {
                        cache.stack.push(Frame::RestoreCapture {
                            slot,
                            offset: slots[slot],
                        });
                        slots[slot] = NonMaxUsize::new(at);
                    }
                    sid = next;
                }
                State::Fail => return None,
                State::Match { pattern_id } => {
                    return Some(HalfMatch::new(pattern_id, at));
                }
            }
        }
    }
}
#[derive(Debug)]
pub struct TryFindMatches<'r, 'c, 'h> {
    re: &'r BoundedBacktracker,
    cache: &'c mut Cache,
    caps: Captures,
    it: iter::Searcher<'h>,
}
impl<'r, 'c, 'h> Iterator for TryFindMatches<'r, 'c, 'h> {
    type Item = Result<Match, MatchError>;
    #[inline]
    fn next(&mut self) -> Option<Result<Match, MatchError>> {
        let TryFindMatches {
            re,
            ref mut cache,
            ref mut caps,
            ref mut it,
        } = *self;
        it.try_advance(|input| {
            re.try_search(cache, input, caps)?;
            Ok(caps.get_match())
        })
        .transpose()
    }
}
#[derive(Debug)]
pub struct TryCapturesMatches<'r, 'c, 'h> {
    re: &'r BoundedBacktracker,
    cache: &'c mut Cache,
    caps: Captures,
    it: iter::Searcher<'h>,
}
impl<'r, 'c, 'h> Iterator for TryCapturesMatches<'r, 'c, 'h> {
    type Item = Result<Captures, MatchError>;
    #[inline]
    fn next(&mut self) -> Option<Result<Captures, MatchError>> {
        let TryCapturesMatches {
            re,
            ref mut cache,
            ref mut caps,
            ref mut it,
        } = *self;
        let _ = it
            .try_advance(|input| {
                re.try_search(cache, input, caps)?;
                Ok(caps.get_match())
            })
            .transpose()?;
        if caps.is_match() {
            Some(Ok(caps.clone()))
        } else {
            None
        }
    }
}
#[derive(Clone, Debug)]
pub struct Cache {
    stack: Vec<Frame>,
    visited: Visited,
}
impl Cache {
    pub fn new(re: &BoundedBacktracker) -> Cache {
        Cache {
            stack: vec![],
            visited: Visited::new(re),
        }
    }
    pub fn reset(&mut self, re: &BoundedBacktracker) {
        self.visited.reset(re);
    }
    pub fn memory_usage(&self) -> usize {
        self.stack.len() * core::mem::size_of::<Frame>() + self.visited.memory_usage()
    }
    fn setup_search(
        &mut self,
        re: &BoundedBacktracker,
        input: &Input<'_>,
    ) -> Result<(), MatchError> {
        self.stack.clear();
        self.visited.setup_search(re, input)?;
        Ok(())
    }
}
#[derive(Clone, Debug)]
enum Frame {
    Step {
        sid: StateID,
        at: usize,
    },
    RestoreCapture {
        slot: SmallIndex,
        offset: Option<NonMaxUsize>,
    },
}
#[derive(Clone, Debug)]
struct Visited {
    bitset: Vec<usize>,
    stride: usize,
}
impl Visited {
    const BLOCK_SIZE: usize = 8 * core::mem::size_of::<usize>();
    fn new(re: &BoundedBacktracker) -> Visited {
        let mut visited = Visited {
            bitset: vec![],
            stride: 0,
        };
        visited.reset(re);
        visited
    }
    fn insert(&mut self, sid: StateID, at: usize) -> bool {
        let table_index = sid.as_usize() * self.stride + at;
        let block_index = table_index / Visited::BLOCK_SIZE;
        let bit = table_index % Visited::BLOCK_SIZE;
        let block_with_bit = 1 << bit;
        if self.bitset[block_index] & block_with_bit != 0 {
            return false;
        }
        self.bitset[block_index] |= block_with_bit;
        true
    }
    fn reset(&mut self, _: &BoundedBacktracker) {
        self.bitset.truncate(0);
    }
    fn setup_search(
        &mut self,
        re: &BoundedBacktracker,
        input: &Input<'_>,
    ) -> Result<(), MatchError> {
        let haylen = input.get_span().len();
        let err = || MatchError::haystack_too_long(haylen);
        self.stride = haylen + 1;
        let needed_capacity = match re.get_nfa().states().len().checked_mul(self.stride) {
            None => return Err(err()),
            Some(capacity) => capacity,
        };
        let max_capacity = 8 * re.get_config().get_visited_capacity();
        if needed_capacity > max_capacity {
            return Err(err());
        }
        let needed_blocks = div_ceil(needed_capacity, Visited::BLOCK_SIZE);
        self.bitset.truncate(needed_blocks);
        for block in self.bitset.iter_mut() {
            *block = 0;
        }
        if needed_blocks > self.bitset.len() {
            self.bitset.resize(needed_blocks, 0);
        }
        Ok(())
    }
    fn memory_usage(&self) -> usize {
        self.bitset.len() * core::mem::size_of::<usize>()
    }
}
fn div_ceil(lhs: usize, rhs: usize) -> usize {
    if lhs % rhs == 0 {
        lhs / rhs
    } else {
        (lhs / rhs) + 1
    }
}
