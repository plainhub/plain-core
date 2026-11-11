use crate::regex_engine::automata::{
    hybrid::{
        error::{BuildError, CacheError, StartError},
        id::{LazyStateID, LazyStateIDError},
        search,
    },
    nfa::thompson,
    util::{
        alphabet::{self, ByteClasses, ByteSet},
        determinize::{self, State, StateBuilderEmpty, StateBuilderNFA},
        empty,
        prefilter::Prefilter,
        primitives::{PatternID, StateID as NFAStateID},
        search::{Anchored, HalfMatch, Input, MatchError, MatchKind, PatternSet},
        sparse_set::SparseSets,
        start::{self, Start, StartByteMap},
    },
};
use core::{iter, mem::size_of};
use std::vec::Vec;
const MIN_STATES: usize = SENTINEL_STATES + 2;
const SENTINEL_STATES: usize = 3;
#[derive(Clone, Debug)]
pub struct DFA {
    config: Config,
    nfa: thompson::NFA,
    stride2: usize,
    start_map: StartByteMap,
    classes: ByteClasses,
    quitset: ByteSet,
    cache_capacity: usize,
}
impl DFA {
    pub fn new(pattern: &str) -> Result<DFA, BuildError> {
        DFA::builder().build(pattern)
    }
    pub fn new_many<P: AsRef<str>>(patterns: &[P]) -> Result<DFA, BuildError> {
        DFA::builder().build_many(patterns)
    }
    pub fn always_match() -> Result<DFA, BuildError> {
        let nfa = thompson::NFA::always_match();
        Builder::new().build_from_nfa(nfa)
    }
    pub fn never_match() -> Result<DFA, BuildError> {
        let nfa = thompson::NFA::never_match();
        Builder::new().build_from_nfa(nfa)
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
    pub fn reset_cache(&self, cache: &mut Cache) {
        Lazy::new(self, cache).reset_cache()
    }
    pub fn pattern_len(&self) -> usize {
        self.nfa.pattern_len()
    }
    pub fn byte_classes(&self) -> &ByteClasses {
        &self.classes
    }
    pub fn get_config(&self) -> &Config {
        &self.config
    }
    pub fn get_nfa(&self) -> &thompson::NFA {
        &self.nfa
    }
    fn stride2(&self) -> usize {
        self.stride2
    }
    fn stride(&self) -> usize {
        1 << self.stride2()
    }
    pub fn memory_usage(&self) -> usize {
        0
    }
}
impl DFA {
    #[inline]
    pub fn try_search_fwd(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, MatchError> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        let hm = match search::find_fwd(self, cache, input)? {
            None => return Ok(None),
            Some(hm) if !utf8empty => return Ok(Some(hm)),
            Some(hm) => hm,
        };
        empty::skip_splits_fwd(input, hm, hm.offset(), |input| {
            let got = search::find_fwd(self, cache, input)?;
            Ok(got.map(|hm| (hm, hm.offset())))
        })
    }
    #[inline]
    pub fn try_search_rev(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, MatchError> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        let hm = match search::find_rev(self, cache, input)? {
            None => return Ok(None),
            Some(hm) if !utf8empty => return Ok(Some(hm)),
            Some(hm) => hm,
        };
        empty::skip_splits_rev(input, hm, hm.offset(), |input| {
            let got = search::find_rev(self, cache, input)?;
            Ok(got.map(|hm| (hm, hm.offset())))
        })
    }
    #[inline]
    pub fn try_search_overlapping_fwd(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        search::find_overlapping_fwd(self, cache, input, state)?;
        match state.get_match() {
            None => Ok(()),
            Some(_) if !utf8empty => Ok(()),
            Some(_) => skip_empty_utf8_splits_overlapping(input, state, |input, state| {
                search::find_overlapping_fwd(self, cache, input, state)
            }),
        }
    }
    #[inline]
    pub fn try_search_overlapping_rev(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        search::find_overlapping_rev(self, cache, input, state)?;
        match state.get_match() {
            None => Ok(()),
            Some(_) if !utf8empty => Ok(()),
            Some(_) => skip_empty_utf8_splits_overlapping(input, state, |input, state| {
                search::find_overlapping_rev(self, cache, input, state)
            }),
        }
    }
    #[inline]
    pub fn try_which_overlapping_matches(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) -> Result<(), MatchError> {
        let mut state = OverlappingState::start();
        while let Some(m) = {
            self.try_search_overlapping_fwd(cache, input, &mut state)?;
            state.get_match()
        } {
            let _ = patset.try_insert(m.pattern());
            if patset.is_full() || input.get_earliest() {
                break;
            }
        }
        Ok(())
    }
}
impl DFA {
    #[inline]
    pub fn next_state(
        &self,
        cache: &mut Cache,
        current: LazyStateID,
        input: u8,
    ) -> Result<LazyStateID, CacheError> {
        let class = usize::from(self.classes.get(input));
        let offset = current.as_usize_untagged() + class;
        let sid = cache.trans[offset];
        if !sid.is_unknown() {
            return Ok(sid);
        }
        let unit = alphabet::Unit::u8(input);
        Lazy::new(self, cache).cache_next_state(current, unit)
    }

    #[inline]
    pub unsafe fn next_state_untagged_unchecked(
        &self,
        cache: &Cache,
        current: LazyStateID,
        input: u8,
    ) -> LazyStateID {
        unsafe {
            debug_assert!(!current.is_tagged());
            let class = usize::from(self.classes.get(input));
            let offset = current.as_usize_unchecked() + class;
            *cache.trans.get_unchecked(offset)
        }
    }
    #[inline]
    pub fn next_eoi_state(
        &self,
        cache: &mut Cache,
        current: LazyStateID,
    ) -> Result<LazyStateID, CacheError> {
        let eoi = self.classes.eoi().as_usize();
        let offset = current.as_usize_untagged() + eoi;
        let sid = cache.trans[offset];
        if !sid.is_unknown() {
            return Ok(sid);
        }
        let unit = self.classes.eoi();
        Lazy::new(self, cache).cache_next_state(current, unit)
    }
    #[cfg_attr(all(), inline(always))]
    pub fn start_state(
        &self,
        cache: &mut Cache,
        config: &start::Config,
    ) -> Result<LazyStateID, StartError> {
        let lazy = LazyRef::new(self, cache);
        let anchored = config.get_anchored();
        let start = match config.get_look_behind() {
            None => Start::Text,
            Some(byte) => {
                if !self.quitset.is_empty() && self.quitset.contains(byte) {
                    return Err(StartError::quit(byte));
                }
                self.start_map.get(byte)
            }
        };
        let start_id = lazy.get_cached_start_id(anchored, start)?;
        if !start_id.is_unknown() {
            return Ok(start_id);
        }
        Lazy::new(self, cache).cache_start_group(anchored, start)
    }
    #[cfg_attr(all(), inline(always))]
    pub fn start_state_forward(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<LazyStateID, MatchError> {
        let config = start::Config::from_input_forward(input);
        self.start_state(cache, &config).map_err(|err| match err {
            StartError::Cache { .. } => MatchError::gave_up(input.start()),
            StartError::Quit { byte } => {
                let offset = input
                    .start()
                    .checked_sub(1)
                    .expect("no quit in start without look-behind");
                MatchError::quit(byte, offset)
            }
            StartError::UnsupportedAnchored { mode } => MatchError::unsupported_anchored(mode),
        })
    }
    #[cfg_attr(all(), inline(always))]
    pub fn start_state_reverse(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<LazyStateID, MatchError> {
        let config = start::Config::from_input_reverse(input);
        self.start_state(cache, &config).map_err(|err| match err {
            StartError::Cache { .. } => MatchError::gave_up(input.end()),
            StartError::Quit { byte } => {
                let offset = input.end();
                MatchError::quit(byte, offset)
            }
            StartError::UnsupportedAnchored { mode } => MatchError::unsupported_anchored(mode),
        })
    }
    #[inline]
    pub fn match_len(&self, cache: &Cache, id: LazyStateID) -> usize {
        assert!(id.is_match());
        LazyRef::new(self, cache).get_cached_state(id).match_len()
    }
    #[inline]
    pub fn match_pattern(&self, cache: &Cache, id: LazyStateID, match_index: usize) -> PatternID {
        if self.pattern_len() == 1 {
            return PatternID::ZERO;
        }
        LazyRef::new(self, cache)
            .get_cached_state(id)
            .match_pattern(match_index)
    }
}
#[derive(Clone, Debug)]
pub struct Cache {
    trans: Vec<LazyStateID>,
    starts: Vec<LazyStateID>,
    states: Vec<State>,
    states_to_id: StateMap,
    sparses: SparseSets,
    stack: Vec<NFAStateID>,
    scratch_state_builder: StateBuilderEmpty,
    state_saver: StateSaver,
    memory_usage_state: usize,
    clear_count: usize,
    bytes_searched: usize,
    progress: Option<SearchProgress>,
}
impl Cache {
    pub fn new(dfa: &DFA) -> Cache {
        let mut cache = Cache {
            trans: std::vec![],
            starts: std::vec![],
            states: std::vec![],
            states_to_id: StateMap::new(),
            sparses: SparseSets::new(dfa.get_nfa().states().len()),
            stack: std::vec![],
            scratch_state_builder: StateBuilderEmpty::new(),
            state_saver: StateSaver::none(),
            memory_usage_state: 0,
            clear_count: 0,
            bytes_searched: 0,
            progress: None,
        };
        debug!("pre-init lazy DFA cache size: {}", cache.memory_usage());
        Lazy {
            dfa,
            cache: &mut cache,
        }
        .init_cache();
        debug!("post-init lazy DFA cache size: {}", cache.memory_usage());
        cache
    }
    pub fn reset(&mut self, dfa: &DFA) {
        Lazy::new(dfa, self).reset_cache()
    }
    #[inline]
    pub fn search_start(&mut self, at: usize) {
        if let Some(p) = self.progress.take() {
            self.bytes_searched += p.len();
        }
        self.progress = Some(SearchProgress { start: at, at });
    }
    #[inline]
    pub fn search_update(&mut self, at: usize) {
        let p = self
            .progress
            .as_mut()
            .expect("no in-progress search to update");
        p.at = at;
    }
    #[inline]
    pub fn search_finish(&mut self, at: usize) {
        let mut p = self
            .progress
            .take()
            .expect("no in-progress search to finish");
        p.at = at;
        self.bytes_searched += p.len();
    }
    pub fn search_total_len(&self) -> usize {
        self.bytes_searched + self.progress.as_ref().map_or(0, |p| p.len())
    }
    pub fn clear_count(&self) -> usize {
        self.clear_count
    }
    pub fn memory_usage(&self) -> usize {
        const ID_SIZE: usize = size_of::<LazyStateID>();
        const STATE_SIZE: usize = size_of::<State>();
        self.trans.len() * ID_SIZE
            + self.starts.len() * ID_SIZE
            + self.states.len() * STATE_SIZE
            + self.states_to_id.len() * (STATE_SIZE + ID_SIZE)
            + self.sparses.memory_usage()
            + self.stack.capacity() * ID_SIZE
            + self.scratch_state_builder.capacity()
            + self.memory_usage_state
    }
}
#[derive(Clone, Debug)]
struct SearchProgress {
    start: usize,
    at: usize,
}
impl SearchProgress {
    fn len(&self) -> usize {
        if self.start <= self.at {
            self.at - self.start
        } else {
            self.start - self.at
        }
    }
}
type StateMap = std::collections::HashMap<State, LazyStateID>;
#[derive(Debug)]
struct Lazy<'i, 'c> {
    dfa: &'i DFA,
    cache: &'c mut Cache,
}
impl<'i, 'c> Lazy<'i, 'c> {
    fn new(dfa: &'i DFA, cache: &'c mut Cache) -> Lazy<'i, 'c> {
        Lazy { dfa, cache }
    }
    fn as_ref<'a>(&'a self) -> LazyRef<'i, 'a> {
        LazyRef::new(self.dfa, self.cache)
    }
    #[cold]
    #[inline(never)]
    fn cache_next_state(
        &mut self,
        mut current: LazyStateID,
        unit: alphabet::Unit,
    ) -> Result<LazyStateID, CacheError> {
        let stride2 = self.dfa.stride2();
        let empty_builder = self.get_state_builder();
        let builder = determinize::next(
            self.dfa.get_nfa(),
            self.dfa.get_config().get_match_kind(),
            &mut self.cache.sparses,
            &mut self.cache.stack,
            &self.cache.states[current.as_usize_untagged() >> stride2],
            unit,
            empty_builder,
        );
        let save_state = !self.as_ref().state_builder_fits_in_cache(&builder)
            || self.cache.trans.len() >= LazyStateID::MAX;
        if save_state {
            self.save_state(current);
        }
        let next = self.add_builder_state(builder, |sid| sid)?;
        if save_state {
            current = self.saved_state_id();
        }
        self.set_transition(current, unit, next);
        Ok(next)
    }
    #[cold]
    #[inline(never)]
    fn cache_start_group(
        &mut self,
        anchored: Anchored,
        start: Start,
    ) -> Result<LazyStateID, StartError> {
        let nfa_start_id = match anchored {
            Anchored::No => self.dfa.get_nfa().start_unanchored(),
            Anchored::Yes => self.dfa.get_nfa().start_anchored(),
            Anchored::Pattern(pid) => {
                if !self.dfa.get_config().get_starts_for_each_pattern() {
                    return Err(StartError::unsupported_anchored(anchored));
                }
                match self.dfa.get_nfa().start_pattern(pid) {
                    None => return Ok(self.as_ref().dead_id()),
                    Some(sid) => sid,
                }
            }
        };
        let id = self
            .cache_start_one(nfa_start_id, start)
            .map_err(StartError::cache)?;
        self.set_start_state(anchored, start, id);
        Ok(id)
    }
    fn cache_start_one(
        &mut self,
        nfa_start_id: NFAStateID,
        start: Start,
    ) -> Result<LazyStateID, CacheError> {
        let mut builder_matches = self.get_state_builder().into_matches();
        determinize::set_lookbehind_from_start(self.dfa.get_nfa(), &start, &mut builder_matches);
        self.cache.sparses.set1.clear();
        determinize::epsilon_closure(
            self.dfa.get_nfa(),
            nfa_start_id,
            builder_matches.look_have(),
            &mut self.cache.stack,
            &mut self.cache.sparses.set1,
        );
        let mut builder = builder_matches.into_nfa();
        determinize::add_nfa_states(&self.dfa.get_nfa(), &self.cache.sparses.set1, &mut builder);
        let tag_starts = self.dfa.get_config().get_specialize_start_states();
        self.add_builder_state(builder, |id| if tag_starts { id.to_start() } else { id })
    }
    fn add_builder_state(
        &mut self,
        builder: StateBuilderNFA,
        idmap: impl Fn(LazyStateID) -> LazyStateID,
    ) -> Result<LazyStateID, CacheError> {
        if let Some(&cached_id) = self.cache.states_to_id.get(builder.as_bytes()) {
            self.put_state_builder(builder);
            return Ok(cached_id);
        }
        let result = self.add_state(builder.to_state(), idmap);
        self.put_state_builder(builder);
        result
    }
    fn add_state(
        &mut self,
        state: State,
        idmap: impl Fn(LazyStateID) -> LazyStateID,
    ) -> Result<LazyStateID, CacheError> {
        if !self.as_ref().state_fits_in_cache(&state) {
            self.try_clear_cache()?;
        }
        let mut id = idmap(self.next_state_id()?);
        if state.is_match() {
            id = id.to_match();
        }
        self.cache
            .trans
            .extend(iter::repeat(self.as_ref().unknown_id()).take(self.dfa.stride()));
        if !self.dfa.quitset.is_empty() && !self.as_ref().is_sentinel(id) {
            let quit_id = self.as_ref().quit_id();
            for b in self.dfa.quitset.iter() {
                self.set_transition(id, alphabet::Unit::u8(b), quit_id);
            }
        }
        self.cache.memory_usage_state += state.memory_usage();
        self.cache.states.push(state.clone());
        self.cache.states_to_id.insert(state, id);
        Ok(id)
    }
    fn next_state_id(&mut self) -> Result<LazyStateID, CacheError> {
        let sid = match LazyStateID::new(self.cache.trans.len()) {
            Ok(sid) => sid,
            Err(_) => {
                self.try_clear_cache()?;
                LazyStateID::new(self.cache.trans.len()).unwrap()
            }
        };
        Ok(sid)
    }
    fn try_clear_cache(&mut self) -> Result<(), CacheError> {
        let c = self.dfa.get_config();
        if let Some(min_count) = c.get_minimum_cache_clear_count() {
            if self.cache.clear_count >= min_count {
                if let Some(min_bytes_per) = c.get_minimum_bytes_per_state() {
                    let len = self.cache.search_total_len();
                    let min_bytes = min_bytes_per.saturating_mul(self.cache.states.len());
                    if len == 0 {
                        trace!(
                            "number of bytes searched is 0, but \
                             a minimum bytes per state searched ({}) is \
                             enabled, maybe Cache::search_update \
                             is not being used?",
                            min_bytes_per,
                        );
                    }
                    if len < min_bytes {
                        trace!(
                            "lazy DFA cache has been cleared {} times, \
                             which exceeds the limit of {}, \
                             AND its bytes searched per state is less \
                             than the configured minimum of {}, \
                             therefore lazy DFA is giving up \
                             (bytes searched since cache clear = {}, \
                              number of states = {})",
                            self.cache.clear_count,
                            min_count,
                            min_bytes_per,
                            len,
                            self.cache.states.len(),
                        );
                        return Err(CacheError::bad_efficiency());
                    } else {
                        trace!(
                            "lazy DFA cache has been cleared {} times, \
                             which exceeds the limit of {}, \
                             AND its bytes searched per state is greater \
                             than the configured minimum of {}, \
                             therefore lazy DFA is continuing! \
                             (bytes searched since cache clear = {}, \
                              number of states = {})",
                            self.cache.clear_count,
                            min_count,
                            min_bytes_per,
                            len,
                            self.cache.states.len(),
                        );
                    }
                } else {
                    trace!(
                        "lazy DFA cache has been cleared {} times, \
                         which exceeds the limit of {}, \
                         since there is no configured bytes per state \
                         minimum, lazy DFA is giving up",
                        self.cache.clear_count, min_count,
                    );
                    return Err(CacheError::too_many_cache_clears());
                }
            }
        }
        self.clear_cache();
        Ok(())
    }
    fn reset_cache(&mut self) {
        self.cache.state_saver = StateSaver::none();
        self.clear_cache();
        self.cache.sparses.resize(self.dfa.get_nfa().states().len());
        self.cache.clear_count = 0;
        self.cache.progress = None;
    }
    fn clear_cache(&mut self) {
        self.cache.trans.clear();
        self.cache.starts.clear();
        self.cache.states.clear();
        self.cache.states_to_id.clear();
        self.cache.memory_usage_state = 0;
        self.cache.clear_count += 1;
        self.cache.bytes_searched = 0;
        if let Some(ref mut progress) = self.cache.progress {
            progress.start = progress.at;
        }
        trace!(
            "lazy DFA cache has been cleared (count: {})",
            self.cache.clear_count
        );
        self.init_cache();
        if let Some((old_id, state)) = self.cache.state_saver.take_to_save() {
            assert!(
                !self.as_ref().is_sentinel(old_id),
                "cannot save sentinel state"
            );
            let new_id = self
                .add_state(
                    state,
                    |id| {
                        if old_id.is_start() { id.to_start() } else { id }
                    },
                )
                .expect("adding one state after cache clear must work");
            self.cache.state_saver = StateSaver::Saved(new_id);
        }
    }
    fn init_cache(&mut self) {
        let mut starts_len = Start::len().checked_mul(2).unwrap();
        if self.dfa.get_config().get_starts_for_each_pattern() {
            starts_len += Start::len() * self.dfa.pattern_len();
        }
        self.cache
            .starts
            .extend(iter::repeat(self.as_ref().unknown_id()).take(starts_len));
        let dead = State::dead();
        let unk_id = self.add_state(dead.clone(), |id| id.to_unknown()).unwrap();
        let dead_id = self.add_state(dead.clone(), |id| id.to_dead()).unwrap();
        let quit_id = self.add_state(dead.clone(), |id| id.to_quit()).unwrap();
        assert_eq!(unk_id, self.as_ref().unknown_id());
        assert_eq!(dead_id, self.as_ref().dead_id());
        assert_eq!(quit_id, self.as_ref().quit_id());
        self.set_all_transitions(unk_id, unk_id);
        self.set_all_transitions(dead_id, dead_id);
        self.set_all_transitions(quit_id, quit_id);
        self.cache.states_to_id.insert(dead, dead_id);
    }
    fn save_state(&mut self, id: LazyStateID) {
        let state = self.as_ref().get_cached_state(id).clone();
        self.cache.state_saver = StateSaver::ToSave { id, state };
    }
    fn saved_state_id(&mut self) -> LazyStateID {
        self.cache
            .state_saver
            .take_saved()
            .expect("state saver does not have saved state ID")
    }
    fn set_all_transitions(&mut self, from: LazyStateID, to: LazyStateID) {
        for unit in self.dfa.classes.representatives(..) {
            self.set_transition(from, unit, to);
        }
    }
    fn set_transition(&mut self, from: LazyStateID, unit: alphabet::Unit, to: LazyStateID) {
        assert!(self.as_ref().is_valid(from), "invalid 'from' id: {from:?}");
        assert!(self.as_ref().is_valid(to), "invalid 'to' id: {to:?}");
        let offset = from.as_usize_untagged() + self.dfa.classes.get_by_unit(unit);
        self.cache.trans[offset] = to;
    }
    fn set_start_state(&mut self, anchored: Anchored, start: Start, id: LazyStateID) {
        assert!(self.as_ref().is_valid(id));
        let start_index = start.as_usize();
        let index = match anchored {
            Anchored::No => start_index,
            Anchored::Yes => Start::len() + start_index,
            Anchored::Pattern(pid) => {
                assert!(
                    self.dfa.get_config().get_starts_for_each_pattern(),
                    "attempted to search for a specific pattern \
                     without enabling starts_for_each_pattern",
                );
                let pid = pid.as_usize();
                (2 * Start::len()) + (Start::len() * pid) + start_index
            }
        };
        self.cache.starts[index] = id;
    }
    fn get_state_builder(&mut self) -> StateBuilderEmpty {
        core::mem::replace(
            &mut self.cache.scratch_state_builder,
            StateBuilderEmpty::new(),
        )
    }
    fn put_state_builder(&mut self, builder: StateBuilderNFA) {
        let _ = core::mem::replace(&mut self.cache.scratch_state_builder, builder.clear());
    }
}
#[derive(Debug)]
struct LazyRef<'i, 'c> {
    dfa: &'i DFA,
    cache: &'c Cache,
}
impl<'i, 'c> LazyRef<'i, 'c> {
    fn new(dfa: &'i DFA, cache: &'c Cache) -> LazyRef<'i, 'c> {
        LazyRef { dfa, cache }
    }
    #[cfg_attr(all(), inline(always))]
    fn get_cached_start_id(
        &self,
        anchored: Anchored,
        start: Start,
    ) -> Result<LazyStateID, StartError> {
        let start_index = start.as_usize();
        let index = match anchored {
            Anchored::No => start_index,
            Anchored::Yes => Start::len() + start_index,
            Anchored::Pattern(pid) => {
                if !self.dfa.get_config().get_starts_for_each_pattern() {
                    return Err(StartError::unsupported_anchored(anchored));
                }
                if pid.as_usize() >= self.dfa.pattern_len() {
                    return Ok(self.dead_id());
                }
                (2 * Start::len()) + (Start::len() * pid.as_usize()) + start_index
            }
        };
        Ok(self.cache.starts[index])
    }
    fn get_cached_state(&self, sid: LazyStateID) -> &State {
        let index = sid.as_usize_untagged() >> self.dfa.stride2();
        &self.cache.states[index]
    }
    fn is_sentinel(&self, id: LazyStateID) -> bool {
        id == self.unknown_id() || id == self.dead_id() || id == self.quit_id()
    }
    fn unknown_id(&self) -> LazyStateID {
        LazyStateID::new(0).unwrap().to_unknown()
    }
    fn dead_id(&self) -> LazyStateID {
        LazyStateID::new(1 << self.dfa.stride2()).unwrap().to_dead()
    }
    fn quit_id(&self) -> LazyStateID {
        LazyStateID::new(2 << self.dfa.stride2()).unwrap().to_quit()
    }
    fn is_valid(&self, id: LazyStateID) -> bool {
        let id = id.as_usize_untagged();
        id < self.cache.trans.len() && id % self.dfa.stride() == 0
    }
    fn state_fits_in_cache(&self, state: &State) -> bool {
        let needed =
            self.cache.memory_usage() + self.memory_usage_for_one_more_state(state.memory_usage());
        trace!(
            "lazy DFA cache capacity state check: {:?} ?<=? {:?}",
            needed, self.dfa.cache_capacity
        );
        needed <= self.dfa.cache_capacity
    }
    fn state_builder_fits_in_cache(&self, state: &StateBuilderNFA) -> bool {
        let needed = self.cache.memory_usage()
            + self.memory_usage_for_one_more_state(state.as_bytes().len());
        trace!(
            "lazy DFA cache capacity state builder check: {:?} ?<=? {:?}",
            needed, self.dfa.cache_capacity
        );
        needed <= self.dfa.cache_capacity
    }
    fn memory_usage_for_one_more_state(&self, state_heap_size: usize) -> usize {
        const ID_SIZE: usize = size_of::<LazyStateID>();
        const STATE_SIZE: usize = size_of::<State>();
        self.dfa.stride() * ID_SIZE + STATE_SIZE + (STATE_SIZE + ID_SIZE) + state_heap_size
    }
}
#[derive(Clone, Debug)]
enum StateSaver {
    None,
    ToSave { id: LazyStateID, state: State },
    Saved(LazyStateID),
}
impl StateSaver {
    fn none() -> StateSaver {
        StateSaver::None
    }
    fn take_to_save(&mut self) -> Option<(LazyStateID, State)> {
        match core::mem::replace(self, StateSaver::None) {
            StateSaver::None | StateSaver::Saved(_) => None,
            StateSaver::ToSave { id, state } => Some((id, state)),
        }
    }
    fn take_saved(&mut self) -> Option<LazyStateID> {
        match core::mem::replace(self, StateSaver::None) {
            StateSaver::None => None,
            StateSaver::Saved(id) | StateSaver::ToSave { id, .. } => Some(id),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Config {
    match_kind: Option<MatchKind>,
    pre: Option<Option<Prefilter>>,
    starts_for_each_pattern: Option<bool>,
    byte_classes: Option<bool>,
    unicode_word_boundary: Option<bool>,
    quitset: Option<ByteSet>,
    specialize_start_states: Option<bool>,
    cache_capacity: Option<usize>,
    skip_cache_capacity_check: Option<bool>,
    minimum_cache_clear_count: Option<Option<usize>>,
    minimum_bytes_per_state: Option<Option<usize>>,
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
        if self.specialize_start_states.is_none() {
            self.specialize_start_states = Some(self.get_prefilter().is_some());
        }
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
    pub fn unicode_word_boundary(mut self, yes: bool) -> Config {
        self.unicode_word_boundary = Some(yes);
        self
    }
    pub fn quit(mut self, byte: u8, yes: bool) -> Config {
        if self.get_unicode_word_boundary() && !byte.is_ascii() && !yes {
            panic!(
                "cannot set non-ASCII byte to be non-quit when \
                 Unicode word boundaries are enabled"
            );
        }
        if self.quitset.is_none() {
            self.quitset = Some(ByteSet::empty());
        }
        if yes {
            self.quitset.as_mut().unwrap().add(byte);
        } else {
            self.quitset.as_mut().unwrap().remove(byte);
        }
        self
    }
    pub fn specialize_start_states(mut self, yes: bool) -> Config {
        self.specialize_start_states = Some(yes);
        self
    }
    pub fn cache_capacity(mut self, bytes: usize) -> Config {
        self.cache_capacity = Some(bytes);
        self
    }
    pub fn skip_cache_capacity_check(mut self, yes: bool) -> Config {
        self.skip_cache_capacity_check = Some(yes);
        self
    }
    pub fn minimum_cache_clear_count(mut self, min: Option<usize>) -> Config {
        self.minimum_cache_clear_count = Some(min);
        self
    }
    pub fn minimum_bytes_per_state(mut self, min: Option<usize>) -> Config {
        self.minimum_bytes_per_state = Some(min);
        self
    }
    pub fn get_match_kind(&self) -> MatchKind {
        self.match_kind.unwrap_or(MatchKind::LeftmostFirst)
    }
    pub fn get_prefilter(&self) -> Option<&Prefilter> {
        self.pre.as_ref().unwrap_or(&None).as_ref()
    }
    pub fn get_starts_for_each_pattern(&self) -> bool {
        self.starts_for_each_pattern.unwrap_or(false)
    }
    pub fn get_byte_classes(&self) -> bool {
        self.byte_classes.unwrap_or(true)
    }
    pub fn get_unicode_word_boundary(&self) -> bool {
        self.unicode_word_boundary.unwrap_or(false)
    }
    pub fn get_quit(&self, byte: u8) -> bool {
        self.quitset.map_or(false, |q| q.contains(byte))
    }
    pub fn get_specialize_start_states(&self) -> bool {
        self.specialize_start_states.unwrap_or(false)
    }
    pub fn get_cache_capacity(&self) -> usize {
        self.cache_capacity.unwrap_or(2 * (1 << 20))
    }
    pub fn get_skip_cache_capacity_check(&self) -> bool {
        self.skip_cache_capacity_check.unwrap_or(false)
    }
    pub fn get_minimum_cache_clear_count(&self) -> Option<usize> {
        self.minimum_cache_clear_count.unwrap_or(None)
    }
    pub fn get_minimum_bytes_per_state(&self) -> Option<usize> {
        self.minimum_bytes_per_state.unwrap_or(None)
    }

    fn byte_classes_from_nfa(&self, nfa: &thompson::NFA, quit: &ByteSet) -> ByteClasses {
        if !self.get_byte_classes() {
            ByteClasses::singletons()
        } else {
            let mut set = nfa.byte_class_set().clone();
            if !quit.is_empty() {
                set.add_set(&quit);
            }
            set.byte_classes()
        }
    }
    fn quit_set_from_nfa(&self, nfa: &thompson::NFA) -> Result<ByteSet, BuildError> {
        let mut quit = self.quitset.unwrap_or(ByteSet::empty());
        if nfa.look_set_any().contains_word_unicode() {
            if self.get_unicode_word_boundary() {
                for b in 0x80..=0xFF {
                    quit.add(b);
                }
            } else {
                if !quit.contains_range(0x80, 0xFF) {
                    return Err(BuildError::unsupported_dfa_word_boundary_unicode());
                }
            }
        }
        Ok(quit)
    }
    fn overwrite(&self, o: Config) -> Config {
        Config {
            match_kind: o.match_kind.or(self.match_kind),
            pre: o.pre.or_else(|| self.pre.clone()),
            starts_for_each_pattern: o.starts_for_each_pattern.or(self.starts_for_each_pattern),
            byte_classes: o.byte_classes.or(self.byte_classes),
            unicode_word_boundary: o.unicode_word_boundary.or(self.unicode_word_boundary),
            quitset: o.quitset.or(self.quitset),
            specialize_start_states: o.specialize_start_states.or(self.specialize_start_states),
            cache_capacity: o.cache_capacity.or(self.cache_capacity),
            skip_cache_capacity_check: o
                .skip_cache_capacity_check
                .or(self.skip_cache_capacity_check),
            minimum_cache_clear_count: o
                .minimum_cache_clear_count
                .or(self.minimum_cache_clear_count),
            minimum_bytes_per_state: o.minimum_bytes_per_state.or(self.minimum_bytes_per_state),
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
            .clone()
            .configure(thompson::Config::new().which_captures(thompson::WhichCaptures::None))
            .build_many(patterns)
            .map_err(BuildError::nfa)?;
        self.build_from_nfa(nfa)
    }
    pub fn build_from_nfa(&self, nfa: thompson::NFA) -> Result<DFA, BuildError> {
        let quitset = self.config.quit_set_from_nfa(&nfa)?;
        let classes = self.config.byte_classes_from_nfa(&nfa, &quitset);
        let min_cache =
            minimum_cache_capacity(&nfa, &classes, self.config.get_starts_for_each_pattern());
        let mut cache_capacity = self.config.get_cache_capacity();
        if cache_capacity < min_cache {
            if self.config.get_skip_cache_capacity_check() {
                debug!(
                    "given capacity ({cache_capacity}) is too small, \
                     since skip_cache_capacity_check is enabled, \
                     setting cache capacity to minimum ({min_cache})",
                );
                cache_capacity = min_cache;
            } else {
                return Err(BuildError::insufficient_cache_capacity(
                    min_cache,
                    cache_capacity,
                ));
            }
        }
        if let Err(err) = minimum_lazy_state_id(&classes) {
            return Err(BuildError::insufficient_state_id_capacity(err));
        }
        let stride2 = classes.stride2();
        let start_map = StartByteMap::new(nfa.look_matcher());
        Ok(DFA {
            config: self.config.clone(),
            nfa,
            stride2,
            start_map,
            classes,
            quitset,
            cache_capacity,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlappingState {
    pub(crate) mat: Option<HalfMatch>,
    pub(crate) id: Option<LazyStateID>,
    pub(crate) at: usize,
    pub(crate) next_match_index: Option<usize>,
    pub(crate) rev_eoi: bool,
}
impl OverlappingState {
    pub fn start() -> OverlappingState {
        OverlappingState {
            mat: None,
            id: None,
            at: 0,
            next_match_index: None,
            rev_eoi: false,
        }
    }
    pub fn get_match(&self) -> Option<HalfMatch> {
        self.mat
    }
}
#[cold]
#[inline(never)]
fn skip_empty_utf8_splits_overlapping<F>(
    input: &Input<'_>,
    state: &mut OverlappingState,
    mut search: F,
) -> Result<(), MatchError>
where
    F: FnMut(&Input<'_>, &mut OverlappingState) -> Result<(), MatchError>,
{
    let mut hm = match state.get_match() {
        None => return Ok(()),
        Some(hm) => hm,
    };
    if input.get_anchored().is_anchored() {
        if !input.is_char_boundary(hm.offset()) {
            state.mat = None;
        }
        return Ok(());
    }
    while !input.is_char_boundary(hm.offset()) {
        search(input, state)?;
        hm = match state.get_match() {
            None => return Ok(()),
            Some(hm) => hm,
        };
    }
    Ok(())
}
fn minimum_lazy_state_id(classes: &ByteClasses) -> Result<LazyStateID, LazyStateIDError> {
    let stride = 1 << classes.stride2();
    let min_state_index = MIN_STATES.checked_sub(1).unwrap();
    LazyStateID::new(min_state_index * stride)
}
fn minimum_cache_capacity(
    nfa: &thompson::NFA,
    classes: &ByteClasses,
    starts_for_each_pattern: bool,
) -> usize {
    const ID_SIZE: usize = size_of::<LazyStateID>();
    const STATE_SIZE: usize = size_of::<State>();
    let stride = 1 << classes.stride2();
    let states_len = nfa.states().len();
    let sparses = 2 * states_len * NFAStateID::SIZE;
    let trans = MIN_STATES * stride * ID_SIZE;
    let mut starts = Start::len() * ID_SIZE;
    if starts_for_each_pattern {
        starts += (Start::len() * nfa.pattern_len()) * ID_SIZE;
    }
    assert!(
        MIN_STATES >= 5,
        "minimum number of states has to be at least 5"
    );
    let non_sentinel = MIN_STATES.checked_sub(SENTINEL_STATES).unwrap();
    let dead_state_size = State::dead().memory_usage();
    let max_state_size = 5 + 4 + (nfa.pattern_len() * 4) + (states_len * 5);
    let states = (SENTINEL_STATES * (STATE_SIZE + dead_state_size))
        + (non_sentinel * (STATE_SIZE + max_state_size));
    let states_to_sid = (MIN_STATES * STATE_SIZE) + (MIN_STATES * ID_SIZE);
    let stack = states_len * NFAStateID::SIZE;
    let scratch_state_builder = max_state_size;
    trans + starts + states + states_to_sid + sparses + stack + scratch_state_builder
}
