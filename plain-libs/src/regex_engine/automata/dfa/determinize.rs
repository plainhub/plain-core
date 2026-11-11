use crate::regex_engine::automata::{
    dfa::{
        DEAD,
        dense::{self, BuildError},
    },
    nfa::thompson,
    util::{
        self,
        alphabet::{self, ByteSet},
        determinize::{State, StateBuilderEmpty, StateBuilderNFA},
        primitives::{PatternID, StateID},
        search::{Anchored, MatchKind},
        sparse_set::SparseSets,
        start::Start,
    },
};
use std::{collections::BTreeMap, vec::Vec};
#[derive(Clone, Debug)]
pub(crate) struct Config {
    match_kind: MatchKind,
    quit: ByteSet,
    dfa_size_limit: Option<usize>,
    determinize_size_limit: Option<usize>,
}
impl Config {
    pub fn new() -> Config {
        Config {
            match_kind: MatchKind::LeftmostFirst,
            quit: ByteSet::empty(),
            dfa_size_limit: None,
            determinize_size_limit: None,
        }
    }
    pub fn run(&self, nfa: &thompson::NFA, dfa: &mut dense::OwnedDFA) -> Result<(), BuildError> {
        let dead = State::dead();
        let quit = State::dead();
        let mut cache = StateMap::default();
        cache.insert(dead.clone(), DEAD);
        let runner = Runner {
            config: self.clone(),
            nfa,
            dfa,
            builder_states: std::vec![dead, quit],
            cache,
            memory_usage_state: 0,
            sparses: SparseSets::new(nfa.states().len()),
            stack: std::vec![],
            scratch_state_builder: StateBuilderEmpty::new(),
        };
        runner.run()
    }
    pub fn match_kind(&mut self, kind: MatchKind) -> &mut Config {
        self.match_kind = kind;
        self
    }
    pub fn quit(&mut self, set: ByteSet) -> &mut Config {
        self.quit = set;
        self
    }
    pub fn dfa_size_limit(&mut self, bytes: Option<usize>) -> &mut Config {
        self.dfa_size_limit = bytes;
        self
    }
    pub fn determinize_size_limit(&mut self, bytes: Option<usize>) -> &mut Config {
        self.determinize_size_limit = bytes;
        self
    }
}
#[derive(Debug)]
struct Runner<'a> {
    config: Config,
    nfa: &'a thompson::NFA,
    dfa: &'a mut dense::OwnedDFA,
    builder_states: Vec<State>,
    cache: StateMap,
    memory_usage_state: usize,
    sparses: SparseSets,
    stack: Vec<StateID>,
    scratch_state_builder: StateBuilderEmpty,
}
type StateMap = std::collections::HashMap<State, StateID>;
impl<'a> Runner<'a> {
    fn run(mut self) -> Result<(), BuildError> {
        if self.nfa.look_set_any().contains_word_unicode()
            && !self.config.quit.contains_range(0x80, 0xFF)
        {
            return Err(BuildError::unsupported_dfa_word_boundary_unicode());
        }
        let representatives: Vec<alphabet::Unit> =
            self.dfa.byte_classes().representatives(..).collect();
        let mut uncompiled = std::vec![];
        self.add_all_starts(&mut uncompiled)?;
        while let Some(dfa_id) = uncompiled.pop() {
            for &unit in &representatives {
                if unit.as_u8().map_or(false, |b| self.config.quit.contains(b)) {
                    continue;
                }
                let (next_dfa_id, is_new) = self.cached_state(dfa_id, unit)?;
                self.dfa.set_transition(dfa_id, unit, next_dfa_id);
                if is_new {
                    uncompiled.push(next_dfa_id);
                }
            }
        }
        debug!(
            "determinization complete, memory usage: {}, \
             dense DFA size: {}, \
             is reverse? {}",
            self.memory_usage(),
            self.dfa.memory_usage(),
            self.nfa.is_reverse(),
        );
        let mut matches: BTreeMap<StateID, Vec<PatternID>> = BTreeMap::new();
        self.cache.clear();
        for (i, state) in self.builder_states.into_iter().enumerate() {
            if let Some(pat_ids) = state.match_pattern_ids() {
                let id = self.dfa.to_state_id(i);
                log! { total_pat_len += pat_ids . len () ; }
                matches.insert(id, pat_ids);
            }
        }
        log! { use core :: mem :: size_of ; let per_elem = size_of ::< StateID > () + size_of ::< Vec < PatternID >> () ; let pats = total_pat_len * size_of ::< PatternID > () ; let mem = (matches . len () * per_elem) + pats ; log :: debug ! ("matches map built, memory usage: {mem}") ; }
        self.dfa.shuffle(matches)?;
        Ok(())
    }
    fn cached_state(
        &mut self,
        dfa_id: StateID,
        unit: alphabet::Unit,
    ) -> Result<(StateID, bool), BuildError> {
        let empty_builder = self.get_state_builder();
        let builder = util::determinize::next(
            self.nfa,
            self.config.match_kind,
            &mut self.sparses,
            &mut self.stack,
            &self.builder_states[self.dfa.to_index(dfa_id)],
            unit,
            empty_builder,
        );
        self.maybe_add_state(builder)
    }
    fn add_all_starts(&mut self, dfa_state_ids: &mut Vec<StateID>) -> Result<(), BuildError> {
        assert!(dfa_state_ids.is_empty());
        if self.dfa.start_kind().has_unanchored() {
            self.add_start_group(Anchored::No, dfa_state_ids)?;
        }
        if self.dfa.start_kind().has_anchored() {
            self.add_start_group(Anchored::Yes, dfa_state_ids)?;
        }
        if self.dfa.starts_for_each_pattern() {
            for pid in self.nfa.patterns() {
                self.add_start_group(Anchored::Pattern(pid), dfa_state_ids)?;
            }
        }
        Ok(())
    }
    fn add_start_group(
        &mut self,
        anchored: Anchored,
        dfa_state_ids: &mut Vec<StateID>,
    ) -> Result<(), BuildError> {
        let nfa_start = match anchored {
            Anchored::No => self.nfa.start_unanchored(),
            Anchored::Yes => self.nfa.start_anchored(),
            Anchored::Pattern(pid) => self.nfa.start_pattern(pid).expect("valid pattern ID"),
        };
        let (id, is_new) = self.add_one_start(nfa_start, Start::NonWordByte)?;
        self.dfa.set_start_state(anchored, Start::NonWordByte, id);
        if is_new {
            dfa_state_ids.push(id);
        }
        if !self.nfa.look_set_prefix_any().contains_word() {
            self.dfa.set_start_state(anchored, Start::WordByte, id);
        } else {
            let (id, is_new) = self.add_one_start(nfa_start, Start::WordByte)?;
            self.dfa.set_start_state(anchored, Start::WordByte, id);
            if is_new {
                dfa_state_ids.push(id);
            }
        }
        if !self.nfa.look_set_prefix_any().contains_anchor() {
            self.dfa.set_start_state(anchored, Start::Text, id);
            self.dfa.set_start_state(anchored, Start::LineLF, id);
            self.dfa.set_start_state(anchored, Start::LineCR, id);
            self.dfa
                .set_start_state(anchored, Start::CustomLineTerminator, id);
        } else {
            let (id, is_new) = self.add_one_start(nfa_start, Start::Text)?;
            self.dfa.set_start_state(anchored, Start::Text, id);
            if is_new {
                dfa_state_ids.push(id);
            }
            let (id, is_new) = self.add_one_start(nfa_start, Start::LineLF)?;
            self.dfa.set_start_state(anchored, Start::LineLF, id);
            if is_new {
                dfa_state_ids.push(id);
            }
            let (id, is_new) = self.add_one_start(nfa_start, Start::LineCR)?;
            self.dfa.set_start_state(anchored, Start::LineCR, id);
            if is_new {
                dfa_state_ids.push(id);
            }
            let (id, is_new) = self.add_one_start(nfa_start, Start::CustomLineTerminator)?;
            self.dfa
                .set_start_state(anchored, Start::CustomLineTerminator, id);
            if is_new {
                dfa_state_ids.push(id);
            }
        }
        Ok(())
    }
    fn add_one_start(
        &mut self,
        nfa_start: StateID,
        start: Start,
    ) -> Result<(StateID, bool), BuildError> {
        let mut builder_matches = self.get_state_builder().into_matches();
        util::determinize::set_lookbehind_from_start(self.nfa, &start, &mut builder_matches);
        self.sparses.set1.clear();
        util::determinize::epsilon_closure(
            self.nfa,
            nfa_start,
            builder_matches.look_have(),
            &mut self.stack,
            &mut self.sparses.set1,
        );
        let mut builder = builder_matches.into_nfa();
        util::determinize::add_nfa_states(&self.nfa, &self.sparses.set1, &mut builder);
        self.maybe_add_state(builder)
    }
    fn maybe_add_state(&mut self, builder: StateBuilderNFA) -> Result<(StateID, bool), BuildError> {
        if let Some(&cached_id) = self.cache.get(builder.as_bytes()) {
            self.put_state_builder(builder);
            return Ok((cached_id, false));
        }
        self.add_state(builder).map(|sid| (sid, true))
    }
    fn add_state(&mut self, builder: StateBuilderNFA) -> Result<StateID, BuildError> {
        let id = self.dfa.add_empty_state()?;
        if !self.config.quit.is_empty() {
            for b in self.config.quit.iter() {
                self.dfa
                    .set_transition(id, alphabet::Unit::u8(b), self.dfa.quit_id());
            }
        }
        let state = builder.to_state();
        self.memory_usage_state += state.memory_usage();
        self.builder_states.push(state.clone());
        self.cache.insert(state, id);
        self.put_state_builder(builder);
        if let Some(limit) = self.config.dfa_size_limit {
            if self.dfa.memory_usage() > limit {
                return Err(BuildError::dfa_exceeded_size_limit(limit));
            }
        }
        if let Some(limit) = self.config.determinize_size_limit {
            if self.memory_usage() > limit {
                return Err(BuildError::determinize_exceeded_size_limit(limit));
            }
        }
        Ok(id)
    }
    fn get_state_builder(&mut self) -> StateBuilderEmpty {
        core::mem::replace(&mut self.scratch_state_builder, StateBuilderEmpty::new())
    }
    fn put_state_builder(&mut self, builder: StateBuilderNFA) {
        let _ = core::mem::replace(&mut self.scratch_state_builder, builder.clear());
    }
    fn memory_usage(&self) -> usize {
        use core::mem::size_of;
        self.builder_states.len() * size_of::<State>()
            + self.cache.len() * (size_of::<State>() + size_of::<StateID>())
            + self.memory_usage_state
            + self.stack.capacity() * size_of::<StateID>()
            + self.scratch_state_builder.capacity()
    }
}
