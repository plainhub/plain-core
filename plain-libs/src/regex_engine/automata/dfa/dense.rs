use crate::memchr::ext::Pointer;
use crate::regex_engine::automata::{
    dfa::{
        DEAD,
        accel::Accels,
        automaton::{Automaton, StartError, fmt_state_indicator},
        special::Special,
        start::StartKind,
    },
    util::{
        alphabet::{self, ByteClasses, ByteSet},
        prefilter::Prefilter,
        primitives::{PatternID, StateID},
        search::Anchored,
        start::{self, Start, StartByteMap},
        wire::{self, DeserializeError, Endian, SerializeError},
    },
};
use crate::regex_engine::automata::{
    dfa::{accel::Accel, determinize, minimize::Minimizer, remapper::Remapper},
    nfa::thompson,
    util::{look::LookMatcher, search::MatchKind},
};
use crate::regex_engine::int::Usize;
use core::cmp;
use core::{fmt, iter, mem::size_of, slice};
use std::{
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};
const LABEL: &str = "rust-regex-automata-dfa-dense";
const VERSION: u32 = 2;
#[derive(Clone, Debug, Default)]
pub struct Config {
    accelerate: Option<bool>,
    pre: Option<Option<Prefilter>>,
    minimize: Option<bool>,
    match_kind: Option<MatchKind>,
    start_kind: Option<StartKind>,
    starts_for_each_pattern: Option<bool>,
    byte_classes: Option<bool>,
    unicode_word_boundary: Option<bool>,
    quitset: Option<ByteSet>,
    specialize_start_states: Option<bool>,
    dfa_size_limit: Option<Option<usize>>,
    determinize_size_limit: Option<Option<usize>>,
}
impl Config {
    pub fn new() -> Config {
        Config::default()
    }
    pub fn accelerate(mut self, yes: bool) -> Config {
        self.accelerate = Some(yes);
        self
    }
    pub fn prefilter(mut self, pre: Option<Prefilter>) -> Config {
        self.pre = Some(pre);
        if self.specialize_start_states.is_none() {
            self.specialize_start_states = Some(self.get_prefilter().is_some());
        }
        self
    }
    pub fn minimize(mut self, yes: bool) -> Config {
        self.minimize = Some(yes);
        self
    }
    pub fn match_kind(mut self, kind: MatchKind) -> Config {
        self.match_kind = Some(kind);
        self
    }
    pub fn start_kind(mut self, kind: StartKind) -> Config {
        self.start_kind = Some(kind);
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
    pub fn dfa_size_limit(mut self, bytes: Option<usize>) -> Config {
        self.dfa_size_limit = Some(bytes);
        self
    }
    pub fn determinize_size_limit(mut self, bytes: Option<usize>) -> Config {
        self.determinize_size_limit = Some(bytes);
        self
    }
    pub fn get_accelerate(&self) -> bool {
        self.accelerate.unwrap_or(true)
    }
    pub fn get_prefilter(&self) -> Option<&Prefilter> {
        self.pre.as_ref().unwrap_or(&None).as_ref()
    }
    pub fn get_minimize(&self) -> bool {
        self.minimize.unwrap_or(false)
    }
    pub fn get_match_kind(&self) -> MatchKind {
        self.match_kind.unwrap_or(MatchKind::LeftmostFirst)
    }
    pub fn get_starts(&self) -> StartKind {
        self.start_kind.unwrap_or(StartKind::Both)
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
    pub fn get_dfa_size_limit(&self) -> Option<usize> {
        self.dfa_size_limit.unwrap_or(None)
    }
    pub fn get_determinize_size_limit(&self) -> Option<usize> {
        self.determinize_size_limit.unwrap_or(None)
    }
    pub(crate) fn overwrite(&self, o: Config) -> Config {
        Config {
            accelerate: o.accelerate.or(self.accelerate),
            pre: o.pre.or_else(|| self.pre.clone()),
            minimize: o.minimize.or(self.minimize),
            match_kind: o.match_kind.or(self.match_kind),
            start_kind: o.start_kind.or(self.start_kind),
            starts_for_each_pattern: o.starts_for_each_pattern.or(self.starts_for_each_pattern),
            byte_classes: o.byte_classes.or(self.byte_classes),
            unicode_word_boundary: o.unicode_word_boundary.or(self.unicode_word_boundary),
            quitset: o.quitset.or(self.quitset),
            specialize_start_states: o.specialize_start_states.or(self.specialize_start_states),
            dfa_size_limit: o.dfa_size_limit.or(self.dfa_size_limit),
            determinize_size_limit: o.determinize_size_limit.or(self.determinize_size_limit),
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
    pub fn build(&self, pattern: &str) -> Result<OwnedDFA, BuildError> {
        self.build_many(&[pattern])
    }
    pub fn build_many<P: AsRef<str>>(&self, patterns: &[P]) -> Result<OwnedDFA, BuildError> {
        let nfa = self
            .thompson
            .clone()
            .configure(thompson::Config::new().which_captures(thompson::WhichCaptures::None))
            .build_many(patterns)
            .map_err(BuildError::nfa)?;
        self.build_from_nfa(&nfa)
    }
    pub fn build_from_nfa(&self, nfa: &thompson::NFA) -> Result<OwnedDFA, BuildError> {
        let mut quitset = self.config.quitset.unwrap_or(ByteSet::empty());
        if self.config.get_unicode_word_boundary() && nfa.look_set_any().contains_word_unicode() {
            for b in 0x80..=0xFF {
                quitset.add(b);
            }
        }
        let classes = if !self.config.get_byte_classes() {
            ByteClasses::singletons()
        } else {
            let mut set = nfa.byte_class_set().clone();
            if !quitset.is_empty() {
                set.add_set(&quitset);
            }
            set.byte_classes()
        };
        let mut dfa = DFA::initial(
            classes,
            nfa.pattern_len(),
            self.config.get_starts(),
            nfa.look_matcher(),
            self.config.get_starts_for_each_pattern(),
            self.config.get_prefilter().map(|p| p.clone()),
            quitset,
            Flags::from_nfa(&nfa),
        )?;
        determinize::Config::new()
            .match_kind(self.config.get_match_kind())
            .quit(quitset)
            .dfa_size_limit(self.config.get_dfa_size_limit())
            .determinize_size_limit(self.config.get_determinize_size_limit())
            .run(nfa, &mut dfa)?;
        if self.config.get_minimize() {
            dfa.minimize();
        }
        if self.config.get_accelerate() {
            dfa.accelerate();
        }
        if !self.config.get_specialize_start_states() {
            dfa.special.set_no_special_start_states();
        }
        dfa.set_universal_starts();
        dfa.tt.table.shrink_to_fit();
        dfa.st.table.shrink_to_fit();
        dfa.ms.slices.shrink_to_fit();
        dfa.ms.pattern_ids.shrink_to_fit();
        Ok(dfa)
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
impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}
pub(crate) type OwnedDFA = DFA<std::vec::Vec<u32>>;
#[derive(Clone)]
pub struct DFA<T> {
    tt: TransitionTable<T>,
    st: StartTable<T>,
    ms: MatchStates<T>,
    special: Special,
    accels: Accels<T>,
    pre: Option<Prefilter>,
    quitset: ByteSet,
    flags: Flags,
}
impl OwnedDFA {
    pub fn new(pattern: &str) -> Result<OwnedDFA, BuildError> {
        Builder::new().build(pattern)
    }
    pub fn new_many<P: AsRef<str>>(patterns: &[P]) -> Result<OwnedDFA, BuildError> {
        Builder::new().build_many(patterns)
    }
}
impl OwnedDFA {
    pub fn always_match() -> Result<OwnedDFA, BuildError> {
        let nfa = thompson::NFA::always_match();
        Builder::new().build_from_nfa(&nfa)
    }
    pub fn never_match() -> Result<OwnedDFA, BuildError> {
        let nfa = thompson::NFA::never_match();
        Builder::new().build_from_nfa(&nfa)
    }
    fn initial(
        classes: ByteClasses,
        pattern_len: usize,
        starts: StartKind,
        lookm: &LookMatcher,
        starts_for_each_pattern: bool,
        pre: Option<Prefilter>,
        quitset: ByteSet,
        flags: Flags,
    ) -> Result<OwnedDFA, BuildError> {
        let start_pattern_len = if starts_for_each_pattern {
            Some(pattern_len)
        } else {
            None
        };
        Ok(DFA {
            tt: TransitionTable::minimal(classes),
            st: StartTable::dead(starts, lookm, start_pattern_len)?,
            ms: MatchStates::empty(pattern_len),
            special: Special::new(),
            accels: Accels::empty(),
            pre,
            quitset,
            flags,
        })
    }
}
impl DFA<&[u32]> {
    pub fn config() -> Config {
        Config::new()
    }
    pub fn builder() -> Builder {
        Builder::new()
    }
}
impl<T: AsRef<[u32]>> DFA<T> {
    pub fn as_ref(&self) -> DFA<&'_ [u32]> {
        DFA {
            tt: self.tt.as_ref(),
            st: self.st.as_ref(),
            ms: self.ms.as_ref(),
            special: self.special,
            accels: self.accels(),
            pre: self.pre.clone(),
            quitset: self.quitset,
            flags: self.flags,
        }
    }
    pub fn to_owned(&self) -> OwnedDFA {
        DFA {
            tt: self.tt.to_owned(),
            st: self.st.to_owned(),
            ms: self.ms.to_owned(),
            special: self.special,
            accels: self.accels().to_owned(),
            pre: self.pre.clone(),
            quitset: self.quitset,
            flags: self.flags,
        }
    }
    pub fn start_kind(&self) -> StartKind {
        self.st.kind
    }
    pub(crate) fn start_map(&self) -> &StartByteMap {
        &self.st.start_map
    }
    pub fn starts_for_each_pattern(&self) -> bool {
        self.st.pattern_len.is_some()
    }
    pub fn byte_classes(&self) -> &ByteClasses {
        &self.tt.classes
    }
    pub fn alphabet_len(&self) -> usize {
        self.tt.alphabet_len()
    }
    pub fn stride2(&self) -> usize {
        self.tt.stride2
    }
    pub fn stride(&self) -> usize {
        self.tt.stride()
    }
    pub fn memory_usage(&self) -> usize {
        self.tt.memory_usage()
            + self.st.memory_usage()
            + self.ms.memory_usage()
            + self.accels.memory_usage()
    }
}
impl<T: AsRef<[u32]>> DFA<T> {
    fn to_bytes<E: Endian>(&self) -> (Vec<u8>, usize) {
        let len = self.write_to_len();
        let (mut buf, padding) = wire::alloc_aligned_buffer::<u32>(len);
        self.as_ref().write_to::<E>(&mut buf[padding..]).unwrap();
        (buf, padding)
    }

    pub fn write_to_len(&self) -> usize {
        wire::write_label_len(LABEL)
            + wire::write_endianness_check_len()
            + wire::write_version_len()
            + size_of::<u32>()
            + self.flags.write_to_len()
            + self.tt.write_to_len()
            + self.st.write_to_len()
            + self.ms.write_to_len()
            + self.special.write_to_len()
            + self.accels.write_to_len()
            + self.quitset.write_to_len()
    }
}
impl<'a> DFA<&'a [u32]> {
    pub fn from_bytes(slice: &'a [u8]) -> Result<(DFA<&'a [u32]>, usize), DeserializeError> {
        let (dfa, nread) = unsafe { DFA::from_bytes_unchecked(slice)? };
        dfa.accels.validate()?;
        dfa.ms.validate(&dfa)?;
        dfa.tt.validate(&dfa)?;
        dfa.st.validate(&dfa)?;
        for state in dfa.states() {
            if dfa.is_accel_state(state.id()) {
                let index = dfa.accelerator_index(state.id());
                if index >= dfa.accels.len() {
                    return Err(DeserializeError::generic(
                        "found DFA state with invalid accelerator index",
                    ));
                }
                let needles = dfa.accels.needles(index);
                if !(1 <= needles.len() && needles.len() <= 3) {
                    return Err(DeserializeError::generic(
                        "accelerator needles has invalid length",
                    ));
                }
            }
        }
        Ok((dfa, nread))
    }
    pub unsafe fn from_bytes_unchecked(
        slice: &'a [u8],
    ) -> Result<(DFA<&'a [u32]>, usize), DeserializeError> {
        unsafe {
            let mut nr = 0;
            nr += wire::skip_initial_padding(slice);
            wire::check_alignment::<StateID>(&slice[nr..])?;
            nr += wire::read_label(&slice[nr..], LABEL)?;
            nr += wire::read_endianness_check(&slice[nr..])?;
            nr += wire::read_version(&slice[nr..], VERSION)?;
            let _unused = wire::try_read_u32(&slice[nr..], "unused space")?;
            nr += size_of::<u32>();
            let (flags, nread) = Flags::from_bytes(&slice[nr..])?;
            nr += nread;
            let (tt, nread) = TransitionTable::from_bytes_unchecked(&slice[nr..])?;
            nr += nread;
            let (st, nread) = StartTable::from_bytes_unchecked(&slice[nr..])?;
            nr += nread;
            let (ms, nread) = MatchStates::from_bytes_unchecked(&slice[nr..])?;
            nr += nread;
            let (special, nread) = Special::from_bytes(&slice[nr..])?;
            nr += nread;
            special.validate_state_len(tt.len(), tt.stride2)?;
            let (accels, nread) = Accels::from_bytes_unchecked(&slice[nr..])?;
            nr += nread;
            let (quitset, nread) = ByteSet::from_bytes(&slice[nr..])?;
            nr += nread;
            let pre = None;
            Ok((
                DFA {
                    tt,
                    st,
                    ms,
                    special,
                    accels,
                    pre,
                    quitset,
                    flags,
                },
                nr,
            ))
        }
    }
    fn write_to<E: Endian>(&self, mut dst: &mut [u8]) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("dense DFA"));
        }
        dst = &mut dst[..nwrite];
        let mut nw = 0;
        nw += wire::write_label(LABEL, &mut dst[nw..])?;
        nw += wire::write_endianness_check::<E>(&mut dst[nw..])?;
        nw += wire::write_version::<E>(VERSION, &mut dst[nw..])?;
        nw += {
            E::write_u32(0, &mut dst[nw..]);
            size_of::<u32>()
        };
        nw += self.flags.write_to::<E>(&mut dst[nw..])?;
        nw += self.tt.write_to::<E>(&mut dst[nw..])?;
        nw += self.st.write_to::<E>(&mut dst[nw..])?;
        nw += self.ms.write_to::<E>(&mut dst[nw..])?;
        nw += self.special.write_to::<E>(&mut dst[nw..])?;
        nw += self.accels.write_to::<E>(&mut dst[nw..])?;
        nw += self.quitset.write_to::<E>(&mut dst[nw..])?;
        Ok(nw)
    }
}
impl<T> DFA<T> {}
impl OwnedDFA {
    pub(crate) fn set_start_state(&mut self, anchored: Anchored, start: Start, id: StateID) {
        assert!(self.tt.is_valid(id), "invalid start state");
        self.st.set_start(anchored, start, id);
    }
    pub(crate) fn set_transition(&mut self, from: StateID, byte: alphabet::Unit, to: StateID) {
        self.tt.set(from, byte, to);
    }
    pub(crate) fn add_empty_state(&mut self) -> Result<StateID, BuildError> {
        self.tt.add_empty_state()
    }
    pub(crate) fn swap_states(&mut self, id1: StateID, id2: StateID) {
        self.tt.swap(id1, id2);
    }
    pub(crate) fn remap(&mut self, map: impl Fn(StateID) -> StateID) {
        for sid in self.tt.table_mut().iter_mut() {
            *sid = map(*sid);
        }
        for sid in self.st.table_mut().iter_mut() {
            *sid = map(*sid);
        }
    }
    pub(crate) fn remap_state(&mut self, id: StateID, map: impl Fn(StateID) -> StateID) {
        self.tt.remap(id, map);
    }
    pub(crate) fn truncate_states(&mut self, len: usize) {
        self.tt.truncate(len);
    }
    pub(crate) fn minimize(&mut self) {
        Minimizer::new(self).run();
    }
    pub(crate) fn set_pattern_map(
        &mut self,
        map: &BTreeMap<StateID, Vec<PatternID>>,
    ) -> Result<(), BuildError> {
        self.ms = self.ms.new_with_map(map)?;
        Ok(())
    }
    pub(crate) fn accelerate(&mut self) {
        if self.state_len() <= 2 {
            return;
        }
        let mut accels = BTreeMap::new();
        let (mut cmatch, mut cstart, mut cnormal) = (0, 0, 0);
        for state in self.states() {
            if let Some(accel) = state.accelerate(self.byte_classes()) {
                debug!(
                    "accelerating full DFA state {}: {:?}",
                    state.id().as_usize(),
                    accel,
                );
                accels.insert(state.id(), accel);
                if self.is_match_state(state.id()) {
                    cmatch += 1;
                } else if self.is_start_state(state.id()) {
                    cstart += 1;
                } else {
                    assert!(!self.is_dead_state(state.id()));
                    assert!(!self.is_quit_state(state.id()));
                    cnormal += 1;
                }
            }
        }
        if accels.is_empty() {
            return;
        }
        let original_accels_len = accels.len();
        let mut remapper = Remapper::new(self);
        let mut new_matches = self.ms.to_map(self);
        self.special.min_accel = StateID::MAX;
        self.special.max_accel = StateID::ZERO;
        let update_special_accel = |special: &mut Special, accel_id: StateID| {
            special.min_accel = cmp::min(special.min_accel, accel_id);
            special.max_accel = cmp::max(special.max_accel, accel_id);
        };
        if cmatch > 0 && self.special.matches() {
            let mut next_id = self.special.max_match;
            let mut cur_id = next_id;
            while cur_id >= self.special.min_match {
                if let Some(accel) = accels.remove(&cur_id) {
                    accels.insert(next_id, accel);
                    update_special_accel(&mut self.special, next_id);
                    if cur_id != next_id {
                        remapper.swap(self, cur_id, next_id);
                        let cur_pids = new_matches.remove(&cur_id).unwrap();
                        let next_pids = new_matches.remove(&next_id).unwrap();
                        new_matches.insert(cur_id, next_pids);
                        new_matches.insert(next_id, cur_pids);
                    }
                    next_id = self.tt.prev_state_id(next_id);
                }
                cur_id = self.tt.prev_state_id(cur_id);
            }
        }
        if cnormal > 0 {
            let mut next_start_id = self.special.min_start;
            let mut cur_id = self.to_state_id(self.state_len() - 1);
            let mut next_norm_id = self.tt.next_state_id(self.special.max_start);
            while cur_id >= next_norm_id {
                if let Some(accel) = accels.remove(&cur_id) {
                    remapper.swap(self, next_start_id, cur_id);
                    remapper.swap(self, next_norm_id, cur_id);
                    if let Some(accel2) = accels.remove(&next_norm_id) {
                        accels.insert(cur_id, accel2);
                    }
                    if let Some(accel2) = accels.remove(&next_start_id) {
                        accels.insert(next_norm_id, accel2);
                    }
                    accels.insert(next_start_id, accel);
                    update_special_accel(&mut self.special, next_start_id);
                    self.special.min_start = self.tt.next_state_id(self.special.min_start);
                    self.special.max_start = self.tt.next_state_id(self.special.max_start);
                    next_start_id = self.tt.next_state_id(next_start_id);
                    next_norm_id = self.tt.next_state_id(next_norm_id);
                }
                if !accels.contains_key(&cur_id) {
                    cur_id = self.tt.prev_state_id(cur_id);
                }
            }
        }
        if cstart > 0 {
            let mut next_id = self.special.min_start;
            let mut cur_id = next_id;
            while cur_id <= self.special.max_start {
                if let Some(accel) = accels.remove(&cur_id) {
                    remapper.swap(self, cur_id, next_id);
                    accels.insert(next_id, accel);
                    update_special_accel(&mut self.special, next_id);
                    next_id = self.tt.next_state_id(next_id);
                }
                cur_id = self.tt.next_state_id(cur_id);
            }
        }
        remapper.remap(self);
        self.set_pattern_map(&new_matches).unwrap();
        self.special.set_max();
        self.special
            .validate()
            .expect("special state ranges should validate");
        self.special
            .validate_state_len(self.state_len(), self.stride2())
            .expect("special state ranges should be consistent with state length");
        assert_eq!(
            self.special.accel_len(self.stride()),
            original_accels_len,
            "mismatch with expected number of accelerated states",
        );
        let mut prev: Option<StateID> = None;
        for (id, accel) in accels {
            assert!(prev.map_or(true, |p| self.tt.next_state_id(p) == id));
            prev = Some(id);
            self.accels.add(accel);
        }
    }
    pub(crate) fn shuffle(
        &mut self,
        mut matches: BTreeMap<StateID, Vec<PatternID>>,
    ) -> Result<(), BuildError> {
        self.special.quit_id = self.to_state_id(1);
        if self.state_len() <= 2 {
            self.special.set_max();
            return Ok(());
        }
        let mut is_start: BTreeSet<StateID> = BTreeSet::new();
        for (start_id, _, _) in self.starts() {
            if start_id == DEAD {
                continue;
            }
            assert!(
                !matches.contains_key(&start_id),
                "{start_id:?} is both a start and a match state, \
                 which is not allowed",
            );
            is_start.insert(start_id);
        }
        let mut remapper = Remapper::new(self);
        if matches.is_empty() {
            self.special.min_match = DEAD;
            self.special.max_match = DEAD;
        } else {
            let mut next_id = self.to_state_id(2);
            let mut new_matches = BTreeMap::new();
            self.special.min_match = next_id;
            for (id, pids) in matches {
                remapper.swap(self, next_id, id);
                new_matches.insert(next_id, pids);
                if is_start.contains(&next_id) {
                    is_start.remove(&next_id);
                    is_start.insert(id);
                }
                next_id = self.tt.next_state_id(next_id);
            }
            matches = new_matches;
            self.special.max_match =
                cmp::max(self.special.min_match, self.tt.prev_state_id(next_id));
        }
        {
            let mut next_id = self.to_state_id(2);
            if self.special.matches() {
                next_id = self.tt.next_state_id(self.special.max_match);
            }
            self.special.min_start = next_id;
            for id in is_start {
                remapper.swap(self, next_id, id);
                next_id = self.tt.next_state_id(next_id);
            }
            self.special.max_start =
                cmp::max(self.special.min_start, self.tt.prev_state_id(next_id));
        }
        remapper.remap(self);
        self.set_pattern_map(&matches)?;
        self.special.set_max();
        self.special
            .validate()
            .expect("special state ranges should validate");
        self.special
            .validate_state_len(self.state_len(), self.stride2())
            .expect("special state ranges should be consistent with state length");
        Ok(())
    }
    fn set_universal_starts(&mut self) {
        assert_eq!(6, Start::len(), "expected 6 start configurations");
        let start_id = |dfa: &mut OwnedDFA, anchored: Anchored, start: Start| {
            dfa.st
                .start(anchored, start)
                .expect("valid Input configuration")
        };
        if self.start_kind().has_unanchored() {
            let anchor = Anchored::No;
            let sid = start_id(self, anchor, Start::NonWordByte);
            if sid == start_id(self, anchor, Start::WordByte)
                && sid == start_id(self, anchor, Start::Text)
                && sid == start_id(self, anchor, Start::LineLF)
                && sid == start_id(self, anchor, Start::LineCR)
                && sid == start_id(self, anchor, Start::CustomLineTerminator)
            {
                self.st.universal_start_unanchored = Some(sid);
            }
        }
        if self.start_kind().has_anchored() {
            let anchor = Anchored::Yes;
            let sid = start_id(self, anchor, Start::NonWordByte);
            if sid == start_id(self, anchor, Start::WordByte)
                && sid == start_id(self, anchor, Start::Text)
                && sid == start_id(self, anchor, Start::LineLF)
                && sid == start_id(self, anchor, Start::LineCR)
                && sid == start_id(self, anchor, Start::CustomLineTerminator)
            {
                self.st.universal_start_anchored = Some(sid);
            }
        }
    }
}
impl<T: AsRef<[u32]>> DFA<T> {
    pub(crate) fn special(&self) -> &Special {
        &self.special
    }
    pub(crate) fn special_mut(&mut self) -> &mut Special {
        &mut self.special
    }
    pub(crate) fn quitset(&self) -> &ByteSet {
        &self.quitset
    }
    pub(crate) fn flags(&self) -> &Flags {
        &self.flags
    }
    pub(crate) fn states(&self) -> StateIter<'_, T> {
        self.tt.states()
    }
    pub(crate) fn state_len(&self) -> usize {
        self.tt.len()
    }
    pub(crate) fn pattern_id_slice(&self, id: StateID) -> &[PatternID] {
        assert!(self.is_match_state(id));
        self.ms.pattern_id_slice(self.match_state_index(id))
    }
    pub(crate) fn match_pattern_len(&self, id: StateID) -> usize {
        assert!(self.is_match_state(id));
        self.ms.pattern_len(self.match_state_index(id))
    }
    pub(crate) fn pattern_len(&self) -> usize {
        self.ms.pattern_len
    }
    pub(crate) fn pattern_map(&self) -> BTreeMap<StateID, Vec<PatternID>> {
        self.ms.to_map(self)
    }
    pub(crate) fn quit_id(&self) -> StateID {
        self.to_state_id(1)
    }
    pub(crate) fn to_index(&self, id: StateID) -> usize {
        self.tt.to_index(id)
    }
    pub(crate) fn to_state_id(&self, index: usize) -> StateID {
        self.tt.to_state_id(index)
    }
    pub(crate) fn starts(&self) -> StartStateIter<'_> {
        self.st.iter()
    }
    #[cfg_attr(all(), inline(always))]
    fn match_state_index(&self, id: StateID) -> usize {
        debug_assert!(self.is_match_state(id));
        let min = self.special().min_match.as_usize();
        self.to_index(StateID::new_unchecked(id.as_usize() - min))
    }
    fn accelerator_index(&self, id: StateID) -> usize {
        let min = self.special().min_accel.as_usize();
        self.to_index(StateID::new_unchecked(id.as_usize() - min))
    }
    fn accels(&self) -> Accels<&[u32]> {
        self.accels.as_ref()
    }
    fn trans(&self) -> &[StateID] {
        self.tt.table()
    }
}
impl<T: AsRef<[u32]>> fmt::Debug for DFA<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "dense::DFA(")?;
        for state in self.states() {
            fmt_state_indicator(f, self, state.id())?;
            let id = if f.alternate() {
                state.id().as_usize()
            } else {
                self.to_index(state.id())
            };
            write!(f, "{id:06?}: ")?;
            state.fmt(f)?;
            write!(f, "\n")?;
        }
        writeln!(f, "")?;
        for (i, (start_id, anchored, sty)) in self.starts().enumerate() {
            let id = if f.alternate() {
                start_id.as_usize()
            } else {
                self.to_index(start_id)
            };
            if i % self.st.stride == 0 {
                match anchored {
                    Anchored::No => writeln!(f, "START-GROUP(unanchored)")?,
                    Anchored::Yes => writeln!(f, "START-GROUP(anchored)")?,
                    Anchored::Pattern(pid) => writeln!(f, "START_GROUP(pattern: {pid:?})")?,
                }
            }
            writeln!(f, "  {sty:?} => {id:06?}")?;
        }
        if self.pattern_len() > 1 {
            writeln!(f, "")?;
            for i in 0..self.ms.len() {
                let id = self.ms.match_state_id(self, i);
                let id = if f.alternate() {
                    id.as_usize()
                } else {
                    self.to_index(id)
                };
                write!(f, "MATCH({id:06?}): ")?;
                for (i, &pid) in self.ms.pattern_id_slice(i).iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{pid:?}")?;
                }
                writeln!(f, "")?;
            }
        }
        writeln!(f, "state length: {:?}", self.state_len())?;
        writeln!(f, "pattern length: {:?}", self.pattern_len())?;
        writeln!(f, "flags: {:?}", self.flags)?;
        writeln!(f, ")")?;
        Ok(())
    }
}
unsafe impl<T: AsRef<[u32]>> Automaton for DFA<T> {
    #[cfg_attr(all(), inline(always))]
    fn is_special_state(&self, id: StateID) -> bool {
        self.special.is_special_state(id)
    }
    #[cfg_attr(all(), inline(always))]
    fn is_dead_state(&self, id: StateID) -> bool {
        self.special.is_dead_state(id)
    }
    #[cfg_attr(all(), inline(always))]
    fn is_quit_state(&self, id: StateID) -> bool {
        self.special.is_quit_state(id)
    }
    #[cfg_attr(all(), inline(always))]
    fn is_match_state(&self, id: StateID) -> bool {
        self.special.is_match_state(id)
    }
    #[cfg_attr(all(), inline(always))]
    fn is_start_state(&self, id: StateID) -> bool {
        self.special.is_start_state(id)
    }
    #[cfg_attr(all(), inline(always))]
    fn is_accel_state(&self, id: StateID) -> bool {
        self.special.is_accel_state(id)
    }
    #[cfg_attr(all(), inline(always))]
    fn next_state(&self, current: StateID, input: u8) -> StateID {
        let input = self.byte_classes().get(input);
        let o = current.as_usize() + usize::from(input);
        self.trans()[o]
    }
    #[cfg_attr(all(), inline(always))]
    unsafe fn next_state_unchecked(&self, current: StateID, byte: u8) -> StateID {
        unsafe {
            let class = self.byte_classes().get(byte);
            let o = current.as_usize() + usize::from(class);
            let next = *self.trans().get_unchecked(o);
            next
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn next_eoi_state(&self, current: StateID) -> StateID {
        let eoi = self.byte_classes().eoi().as_usize();
        let o = current.as_usize() + eoi;
        self.trans()[o]
    }
    #[cfg_attr(all(), inline(always))]
    fn pattern_len(&self) -> usize {
        self.ms.pattern_len
    }
    #[cfg_attr(all(), inline(always))]
    fn match_len(&self, id: StateID) -> usize {
        self.match_pattern_len(id)
    }
    #[cfg_attr(all(), inline(always))]
    fn match_pattern(&self, id: StateID, match_index: usize) -> PatternID {
        if self.ms.pattern_len == 1 {
            return PatternID::ZERO;
        }
        let state_index = self.match_state_index(id);
        self.ms.pattern_id(state_index, match_index)
    }
    #[cfg_attr(all(), inline(always))]
    fn has_empty(&self) -> bool {
        self.flags.has_empty
    }
    #[cfg_attr(all(), inline(always))]
    fn is_utf8(&self) -> bool {
        self.flags.is_utf8
    }
    #[cfg_attr(all(), inline(always))]
    fn is_always_start_anchored(&self) -> bool {
        self.flags.is_always_start_anchored
    }
    #[cfg_attr(all(), inline(always))]
    fn start_state(&self, config: &start::Config) -> Result<StateID, StartError> {
        let anchored = config.get_anchored();
        let start = match config.get_look_behind() {
            None => Start::Text,
            Some(byte) => {
                if !self.quitset.is_empty() && self.quitset.contains(byte) {
                    return Err(StartError::quit(byte));
                }
                self.st.start_map.get(byte)
            }
        };
        self.st.start(anchored, start)
    }
    #[cfg_attr(all(), inline(always))]
    fn universal_start_state(&self, mode: Anchored) -> Option<StateID> {
        match mode {
            Anchored::No => self.st.universal_start_unanchored,
            Anchored::Yes => self.st.universal_start_anchored,
            Anchored::Pattern(_) => None,
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn accelerator(&self, id: StateID) -> &[u8] {
        if !self.is_accel_state(id) {
            return &[];
        }
        self.accels.needles(self.accelerator_index(id))
    }
    #[cfg_attr(all(), inline(always))]
    fn get_prefilter(&self) -> Option<&Prefilter> {
        self.pre.as_ref()
    }
}
#[derive(Clone)]
pub(crate) struct TransitionTable<T> {
    table: T,
    classes: ByteClasses,
    stride2: usize,
}
impl<'a> TransitionTable<&'a [u32]> {
    unsafe fn from_bytes_unchecked(
        mut slice: &'a [u8],
    ) -> Result<(TransitionTable<&'a [u32]>, usize), DeserializeError> {
        unsafe {
            let slice_start = slice.as_ptr().as_usize();
            let (state_len, nr) = wire::try_read_u32_as_usize(slice, "state length")?;
            slice = &slice[nr..];
            let (stride2, nr) = wire::try_read_u32_as_usize(slice, "stride2")?;
            slice = &slice[nr..];
            let (classes, nr) = ByteClasses::from_bytes(slice)?;
            slice = &slice[nr..];
            if stride2 > 9 {
                return Err(DeserializeError::generic(
                    "dense DFA has invalid stride2 (too big)",
                ));
            }
            if stride2 < 1 {
                return Err(DeserializeError::generic(
                    "dense DFA has invalid stride2 (too small)",
                ));
            }
            let stride = 1usize.checked_shl(u32::try_from(stride2).unwrap()).unwrap();
            if classes.alphabet_len() > stride {
                return Err(DeserializeError::generic(
                    "alphabet size cannot be bigger than transition table stride",
                ));
            }
            let trans_len = wire::shl(state_len, stride2, "dense table transition length")?;
            let table_bytes_len =
                wire::mul(trans_len, StateID::SIZE, "dense table state byte length")?;
            wire::check_slice_len(slice, table_bytes_len, "transition table")?;
            wire::check_alignment::<StateID>(slice)?;
            let table_bytes = &slice[..table_bytes_len];
            slice = &slice[table_bytes_len..];
            let table = core::slice::from_raw_parts(table_bytes.as_ptr().cast::<u32>(), trans_len);
            let tt = TransitionTable {
                table,
                classes,
                stride2,
            };
            Ok((tt, slice.as_ptr().as_usize() - slice_start))
        }
    }
}
impl TransitionTable<Vec<u32>> {
    fn minimal(classes: ByteClasses) -> TransitionTable<Vec<u32>> {
        let mut tt = TransitionTable {
            table: vec![],
            classes,
            stride2: classes.stride2(),
        };
        tt.add_empty_state().unwrap();
        tt.add_empty_state().unwrap();
        tt
    }
    fn set(&mut self, from: StateID, unit: alphabet::Unit, to: StateID) {
        assert!(self.is_valid(from), "invalid 'from' state");
        assert!(self.is_valid(to), "invalid 'to' state");
        self.table[from.as_usize() + self.classes.get_by_unit(unit)] = to.as_u32();
    }
    fn add_empty_state(&mut self) -> Result<StateID, BuildError> {
        let next = self.table.len();
        let id = StateID::new(next).map_err(|_| BuildError::too_many_states())?;
        self.table.extend(iter::repeat(0).take(self.stride()));
        Ok(id)
    }
    fn swap(&mut self, id1: StateID, id2: StateID) {
        assert!(self.is_valid(id1), "invalid 'id1' state: {id1:?}");
        assert!(self.is_valid(id2), "invalid 'id2' state: {id2:?}");
        for b in 0..self.classes.alphabet_len() {
            self.table.swap(id1.as_usize() + b, id2.as_usize() + b);
        }
    }
    fn remap(&mut self, id: StateID, map: impl Fn(StateID) -> StateID) {
        for byte in 0..self.alphabet_len() {
            let i = id.as_usize() + byte;
            let next = self.table()[i];
            self.table_mut()[id.as_usize() + byte] = map(next);
        }
    }
    fn truncate(&mut self, len: usize) {
        self.table.truncate(len << self.stride2);
    }
}
impl<T: AsRef<[u32]>> TransitionTable<T> {
    fn write_to<E: Endian>(&self, mut dst: &mut [u8]) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("transition table"));
        }
        dst = &mut dst[..nwrite];
        E::write_u32(u32::try_from(self.len()).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];
        E::write_u32(u32::try_from(self.stride2).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];
        let n = self.classes.write_to(dst)?;
        dst = &mut dst[n..];
        for &sid in self.table() {
            let n = wire::write_state_id::<E>(sid, &mut dst);
            dst = &mut dst[n..];
        }
        Ok(nwrite)
    }
    fn write_to_len(&self) -> usize {
        size_of::<u32>()
            + size_of::<u32>()
            + self.classes.write_to_len()
            + (self.table().len() * StateID::SIZE)
    }
    fn validate(&self, dfa: &DFA<T>) -> Result<(), DeserializeError> {
        let sp = &dfa.special;
        for state in self.states() {
            if sp.is_special_state(state.id()) {
                let is_actually_special = sp.is_dead_state(state.id())
                    || sp.is_quit_state(state.id())
                    || sp.is_match_state(state.id())
                    || sp.is_start_state(state.id())
                    || sp.is_accel_state(state.id());
                if !is_actually_special {
                    return Err(DeserializeError::generic(
                        "found dense state tagged as special but \
                         wasn't actually special",
                    ));
                }
                if sp.is_match_state(state.id()) && dfa.match_len(state.id()) == 0 {
                    return Err(DeserializeError::generic(
                        "found match state with zero pattern IDs",
                    ));
                }
            }
            for (_, to) in state.transitions() {
                if !self.is_valid(to) {
                    return Err(DeserializeError::generic(
                        "found invalid state ID in transition table",
                    ));
                }
            }
        }
        Ok(())
    }
    fn as_ref(&self) -> TransitionTable<&'_ [u32]> {
        TransitionTable {
            table: self.table.as_ref(),
            classes: self.classes.clone(),
            stride2: self.stride2,
        }
    }
    fn to_owned(&self) -> TransitionTable<std::vec::Vec<u32>> {
        TransitionTable {
            table: self.table.as_ref().to_vec(),
            classes: self.classes.clone(),
            stride2: self.stride2,
        }
    }
    fn state(&self, id: StateID) -> State<'_> {
        assert!(self.is_valid(id));
        let i = id.as_usize();
        State {
            id,
            stride2: self.stride2,
            transitions: &self.table()[i..i + self.alphabet_len()],
        }
    }
    fn states(&self) -> StateIter<'_, T> {
        StateIter {
            tt: self,
            it: self.table().chunks(self.stride()).enumerate(),
        }
    }
    fn to_index(&self, id: StateID) -> usize {
        id.as_usize() >> self.stride2
    }
    fn to_state_id(&self, index: usize) -> StateID {
        StateID::new_unchecked(index << self.stride2)
    }
    fn next_state_id(&self, id: StateID) -> StateID {
        self.to_state_id(self.to_index(id).checked_add(1).unwrap())
    }
    fn prev_state_id(&self, id: StateID) -> StateID {
        self.to_state_id(self.to_index(id).checked_sub(1).unwrap())
    }
    fn table(&self) -> &[StateID] {
        wire::u32s_to_state_ids(self.table.as_ref())
    }
    fn len(&self) -> usize {
        self.table().len() >> self.stride2
    }
    fn stride(&self) -> usize {
        1 << self.stride2
    }
    fn alphabet_len(&self) -> usize {
        self.classes.alphabet_len()
    }
    fn is_valid(&self, id: StateID) -> bool {
        let id = id.as_usize();
        id < self.table().len() && id % self.stride() == 0
    }
    fn memory_usage(&self) -> usize {
        self.table().len() * StateID::SIZE
    }
}
impl<T: AsMut<[u32]>> TransitionTable<T> {
    fn table_mut(&mut self) -> &mut [StateID] {
        wire::u32s_to_state_ids_mut(self.table.as_mut())
    }
}
#[derive(Clone)]
pub(crate) struct StartTable<T> {
    table: T,
    kind: StartKind,
    start_map: StartByteMap,
    stride: usize,
    pattern_len: Option<usize>,
    universal_start_unanchored: Option<StateID>,
    universal_start_anchored: Option<StateID>,
}
impl StartTable<Vec<u32>> {
    fn dead(
        kind: StartKind,
        lookm: &LookMatcher,
        pattern_len: Option<usize>,
    ) -> Result<StartTable<Vec<u32>>, BuildError> {
        if let Some(len) = pattern_len {
            assert!(len <= PatternID::LIMIT);
        }
        let stride = Start::len();
        let starts_len = stride.checked_mul(2).unwrap();
        let pattern_starts_len = match stride.checked_mul(pattern_len.unwrap_or(0)) {
            Some(x) => x,
            None => return Err(BuildError::too_many_start_states()),
        };
        let table_len = match starts_len.checked_add(pattern_starts_len) {
            Some(x) => x,
            None => return Err(BuildError::too_many_start_states()),
        };
        if let Err(_) = isize::try_from(table_len) {
            return Err(BuildError::too_many_start_states());
        }
        let table = vec![DEAD.as_u32(); table_len];
        let start_map = StartByteMap::new(lookm);
        Ok(StartTable {
            table,
            kind,
            start_map,
            stride,
            pattern_len,
            universal_start_unanchored: None,
            universal_start_anchored: None,
        })
    }
}
impl<'a> StartTable<&'a [u32]> {
    unsafe fn from_bytes_unchecked(
        mut slice: &'a [u8],
    ) -> Result<(StartTable<&'a [u32]>, usize), DeserializeError> {
        unsafe {
            let slice_start = slice.as_ptr().as_usize();
            let (kind, nr) = StartKind::from_bytes(slice)?;
            slice = &slice[nr..];
            let (start_map, nr) = StartByteMap::from_bytes(slice)?;
            slice = &slice[nr..];
            let (stride, nr) = wire::try_read_u32_as_usize(slice, "start table stride")?;
            slice = &slice[nr..];
            if stride != Start::len() {
                return Err(DeserializeError::generic("invalid starting table stride"));
            }
            let (maybe_pattern_len, nr) =
                wire::try_read_u32_as_usize(slice, "start table patterns")?;
            slice = &slice[nr..];
            let pattern_len = if maybe_pattern_len.as_u32() == u32::MAX {
                None
            } else {
                Some(maybe_pattern_len)
            };
            if pattern_len.map_or(false, |len| len > PatternID::LIMIT) {
                return Err(DeserializeError::generic("invalid number of patterns"));
            }
            let (universal_unanchored, nr) =
                wire::try_read_u32(slice, "universal unanchored start")?;
            slice = &slice[nr..];
            let universal_start_unanchored = if universal_unanchored == u32::MAX {
                None
            } else {
                Some(StateID::try_from(universal_unanchored).map_err(|e| {
                    DeserializeError::state_id_error(e, "universal unanchored start")
                })?)
            };
            let (universal_anchored, nr) = wire::try_read_u32(slice, "universal anchored start")?;
            slice = &slice[nr..];
            let universal_start_anchored =
                if universal_anchored == u32::MAX {
                    None
                } else {
                    Some(StateID::try_from(universal_anchored).map_err(|e| {
                        DeserializeError::state_id_error(e, "universal anchored start")
                    })?)
                };
            let pattern_table_size =
                wire::mul(stride, pattern_len.unwrap_or(0), "invalid pattern length")?;
            let start_state_len = wire::add(
                wire::mul(2, stride, "start state stride too big")?,
                pattern_table_size,
                "invalid 'any' pattern starts size",
            )?;
            let table_bytes_len =
                wire::mul(start_state_len, StateID::SIZE, "pattern table bytes length")?;
            wire::check_slice_len(slice, table_bytes_len, "start ID table")?;
            wire::check_alignment::<StateID>(slice)?;
            let table_bytes = &slice[..table_bytes_len];
            slice = &slice[table_bytes_len..];
            let table =
                core::slice::from_raw_parts(table_bytes.as_ptr().cast::<u32>(), start_state_len);
            let st = StartTable {
                table,
                kind,
                start_map,
                stride,
                pattern_len,
                universal_start_unanchored,
                universal_start_anchored,
            };
            Ok((st, slice.as_ptr().as_usize() - slice_start))
        }
    }
}
impl<T: AsRef<[u32]>> StartTable<T> {
    fn write_to<E: Endian>(&self, mut dst: &mut [u8]) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("starting table ids"));
        }
        dst = &mut dst[..nwrite];
        let nw = self.kind.write_to::<E>(dst)?;
        dst = &mut dst[nw..];
        let nw = self.start_map.write_to(dst)?;
        dst = &mut dst[nw..];
        E::write_u32(u32::try_from(self.stride).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];
        E::write_u32(
            u32::try_from(self.pattern_len.unwrap_or(0xFFFF_FFFF)).unwrap(),
            dst,
        );
        dst = &mut dst[size_of::<u32>()..];
        E::write_u32(
            self.universal_start_unanchored
                .map_or(u32::MAX, |sid| sid.as_u32()),
            dst,
        );
        dst = &mut dst[size_of::<u32>()..];
        E::write_u32(
            self.universal_start_anchored
                .map_or(u32::MAX, |sid| sid.as_u32()),
            dst,
        );
        dst = &mut dst[size_of::<u32>()..];
        for &sid in self.table() {
            let n = wire::write_state_id::<E>(sid, &mut dst);
            dst = &mut dst[n..];
        }
        Ok(nwrite)
    }
    fn write_to_len(&self) -> usize {
        self.kind.write_to_len()
            + self.start_map.write_to_len()
            + size_of::<u32>()
            + size_of::<u32>()
            + size_of::<u32>()
            + size_of::<u32>()
            + (self.table().len() * StateID::SIZE)
    }
    fn validate(&self, dfa: &DFA<T>) -> Result<(), DeserializeError> {
        let tt = &dfa.tt;
        if !self
            .universal_start_unanchored
            .map_or(true, |s| tt.is_valid(s))
        {
            return Err(DeserializeError::generic(
                "found invalid universal unanchored starting state ID",
            ));
        }
        if !self
            .universal_start_anchored
            .map_or(true, |s| tt.is_valid(s))
        {
            return Err(DeserializeError::generic(
                "found invalid universal anchored starting state ID",
            ));
        }
        for &id in self.table() {
            if !tt.is_valid(id) {
                return Err(DeserializeError::generic("found invalid starting state ID"));
            }
            if dfa.special.is_match_state(id) {
                return Err(DeserializeError::generic(
                    "start states cannot be match states",
                ));
            }
        }
        Ok(())
    }
    fn as_ref(&self) -> StartTable<&'_ [u32]> {
        StartTable {
            table: self.table.as_ref(),
            kind: self.kind,
            start_map: self.start_map.clone(),
            stride: self.stride,
            pattern_len: self.pattern_len,
            universal_start_unanchored: self.universal_start_unanchored,
            universal_start_anchored: self.universal_start_anchored,
        }
    }
    fn to_owned(&self) -> StartTable<std::vec::Vec<u32>> {
        StartTable {
            table: self.table.as_ref().to_vec(),
            kind: self.kind,
            start_map: self.start_map.clone(),
            stride: self.stride,
            pattern_len: self.pattern_len,
            universal_start_unanchored: self.universal_start_unanchored,
            universal_start_anchored: self.universal_start_anchored,
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn start(&self, anchored: Anchored, start: Start) -> Result<StateID, StartError> {
        let start_index = start.as_usize();
        let index = match anchored {
            Anchored::No => {
                if !self.kind.has_unanchored() {
                    return Err(StartError::unsupported_anchored(anchored));
                }
                start_index
            }
            Anchored::Yes => {
                if !self.kind.has_anchored() {
                    return Err(StartError::unsupported_anchored(anchored));
                }
                self.stride + start_index
            }
            Anchored::Pattern(pid) => {
                let len = match self.pattern_len {
                    None => return Err(StartError::unsupported_anchored(anchored)),
                    Some(len) => len,
                };
                if pid.as_usize() >= len {
                    return Ok(DEAD);
                }
                (2 * self.stride) + (self.stride * pid.as_usize()) + start_index
            }
        };
        Ok(self.table()[index])
    }
    fn iter(&self) -> StartStateIter<'_> {
        StartStateIter {
            st: self.as_ref(),
            i: 0,
        }
    }
    fn table(&self) -> &[StateID] {
        wire::u32s_to_state_ids(self.table.as_ref())
    }
    fn memory_usage(&self) -> usize {
        self.table().len() * StateID::SIZE
    }
}
impl<T: AsMut<[u32]>> StartTable<T> {
    fn set_start(&mut self, anchored: Anchored, start: Start, id: StateID) {
        let start_index = start.as_usize();
        let index = match anchored {
            Anchored::No => start_index,
            Anchored::Yes => self.stride + start_index,
            Anchored::Pattern(pid) => {
                let pid = pid.as_usize();
                let len = self
                    .pattern_len
                    .expect("start states for each pattern enabled");
                assert!(pid < len, "invalid pattern ID {pid:?}");
                self.stride
                    .checked_mul(pid)
                    .unwrap()
                    .checked_add(self.stride.checked_mul(2).unwrap())
                    .unwrap()
                    .checked_add(start_index)
                    .unwrap()
            }
        };
        self.table_mut()[index] = id;
    }
    fn table_mut(&mut self) -> &mut [StateID] {
        wire::u32s_to_state_ids_mut(self.table.as_mut())
    }
}
pub(crate) struct StartStateIter<'a> {
    st: StartTable<&'a [u32]>,
    i: usize,
}
impl<'a> Iterator for StartStateIter<'a> {
    type Item = (StateID, Anchored, Start);
    fn next(&mut self) -> Option<(StateID, Anchored, Start)> {
        let i = self.i;
        let table = self.st.table();
        if i >= table.len() {
            return None;
        }
        self.i += 1;
        let start_type = Start::from_usize(i % self.st.stride).unwrap();
        let anchored = if i < self.st.stride {
            Anchored::No
        } else if i < (2 * self.st.stride) {
            Anchored::Yes
        } else {
            let pid = (i - (2 * self.st.stride)) / self.st.stride;
            Anchored::Pattern(PatternID::new(pid).unwrap())
        };
        Some((table[i], anchored, start_type))
    }
}
#[derive(Clone, Debug)]
struct MatchStates<T> {
    slices: T,
    pattern_ids: T,
    pattern_len: usize,
}
impl<'a> MatchStates<&'a [u32]> {
    unsafe fn from_bytes_unchecked(
        mut slice: &'a [u8],
    ) -> Result<(MatchStates<&'a [u32]>, usize), DeserializeError> {
        unsafe {
            let slice_start = slice.as_ptr().as_usize();
            let (state_len, nr) = wire::try_read_u32_as_usize(slice, "match state length")?;
            slice = &slice[nr..];
            let pair_len = wire::mul(2, state_len, "match state offset pairs")?;
            let slices_bytes_len = wire::mul(
                pair_len,
                PatternID::SIZE,
                "match state slice offset byte length",
            )?;
            wire::check_slice_len(slice, slices_bytes_len, "match state slices")?;
            wire::check_alignment::<PatternID>(slice)?;
            let slices_bytes = &slice[..slices_bytes_len];
            slice = &slice[slices_bytes_len..];
            let slices = core::slice::from_raw_parts(slices_bytes.as_ptr().cast::<u32>(), pair_len);
            let (pattern_len, nr) = wire::try_read_u32_as_usize(slice, "pattern length")?;
            slice = &slice[nr..];
            let (idlen, nr) = wire::try_read_u32_as_usize(slice, "pattern ID length")?;
            slice = &slice[nr..];
            let pattern_ids_len = wire::mul(idlen, PatternID::SIZE, "pattern ID byte length")?;
            wire::check_slice_len(slice, pattern_ids_len, "match pattern IDs")?;
            wire::check_alignment::<PatternID>(slice)?;
            let pattern_ids_bytes = &slice[..pattern_ids_len];
            slice = &slice[pattern_ids_len..];
            let pattern_ids =
                core::slice::from_raw_parts(pattern_ids_bytes.as_ptr().cast::<u32>(), idlen);
            let ms = MatchStates {
                slices,
                pattern_ids,
                pattern_len,
            };
            Ok((ms, slice.as_ptr().as_usize() - slice_start))
        }
    }
}
impl MatchStates<Vec<u32>> {
    fn empty(pattern_len: usize) -> MatchStates<Vec<u32>> {
        assert!(pattern_len <= PatternID::LIMIT);
        MatchStates {
            slices: vec![],
            pattern_ids: vec![],
            pattern_len,
        }
    }
    fn new(
        matches: &BTreeMap<StateID, Vec<PatternID>>,
        pattern_len: usize,
    ) -> Result<MatchStates<Vec<u32>>, BuildError> {
        let mut m = MatchStates::empty(pattern_len);
        for (_, pids) in matches.iter() {
            let start = PatternID::new(m.pattern_ids.len())
                .map_err(|_| BuildError::too_many_match_pattern_ids())?;
            m.slices.push(start.as_u32());
            m.slices.push(u32::try_from(pids.len()).unwrap());
            for &pid in pids {
                m.pattern_ids.push(pid.as_u32());
            }
        }
        m.pattern_len = pattern_len;
        Ok(m)
    }
    fn new_with_map(
        &self,
        matches: &BTreeMap<StateID, Vec<PatternID>>,
    ) -> Result<MatchStates<Vec<u32>>, BuildError> {
        MatchStates::new(matches, self.pattern_len)
    }
}
impl<T: AsRef<[u32]>> MatchStates<T> {
    fn write_to<E: Endian>(&self, mut dst: &mut [u8]) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("match states"));
        }
        dst = &mut dst[..nwrite];
        E::write_u32(u32::try_from(self.len()).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];
        for &pid in self.slices() {
            let n = wire::write_pattern_id::<E>(pid, &mut dst);
            dst = &mut dst[n..];
        }
        E::write_u32(u32::try_from(self.pattern_len).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];
        E::write_u32(u32::try_from(self.pattern_ids().len()).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];
        for &pid in self.pattern_ids() {
            let n = wire::write_pattern_id::<E>(pid, &mut dst);
            dst = &mut dst[n..];
        }
        Ok(nwrite)
    }
    fn write_to_len(&self) -> usize {
        size_of::<u32>()
            + (self.slices().len() * PatternID::SIZE)
            + size_of::<u32>()
            + size_of::<u32>()
            + (self.pattern_ids().len() * PatternID::SIZE)
    }
    fn validate(&self, dfa: &DFA<T>) -> Result<(), DeserializeError> {
        if self.len() != dfa.special.match_len(dfa.stride()) {
            return Err(DeserializeError::generic("match state length mismatch"));
        }
        for si in 0..self.len() {
            let start = self.slices()[si * 2].as_usize();
            let len = self.slices()[si * 2 + 1].as_usize();
            if start >= self.pattern_ids().len() {
                return Err(DeserializeError::generic("invalid pattern ID start offset"));
            }
            if start + len > self.pattern_ids().len() {
                return Err(DeserializeError::generic("invalid pattern ID length"));
            }
            for mi in 0..len {
                let pid = self.pattern_id(si, mi);
                if pid.as_usize() >= self.pattern_len {
                    return Err(DeserializeError::generic("invalid pattern ID"));
                }
            }
        }
        Ok(())
    }
    fn to_map(&self, dfa: &DFA<T>) -> BTreeMap<StateID, Vec<PatternID>> {
        let mut map = BTreeMap::new();
        for i in 0..self.len() {
            let mut pids = vec![];
            for j in 0..self.pattern_len(i) {
                pids.push(self.pattern_id(i, j));
            }
            map.insert(self.match_state_id(dfa, i), pids);
        }
        map
    }
    fn as_ref(&self) -> MatchStates<&'_ [u32]> {
        MatchStates {
            slices: self.slices.as_ref(),
            pattern_ids: self.pattern_ids.as_ref(),
            pattern_len: self.pattern_len,
        }
    }
    fn to_owned(&self) -> MatchStates<std::vec::Vec<u32>> {
        MatchStates {
            slices: self.slices.as_ref().to_vec(),
            pattern_ids: self.pattern_ids.as_ref().to_vec(),
            pattern_len: self.pattern_len,
        }
    }
    fn match_state_id(&self, dfa: &DFA<T>, index: usize) -> StateID {
        assert!(dfa.special.matches(), "no match states to index");
        let stride2 = u32::try_from(dfa.stride2()).unwrap();
        let offset = index.checked_shl(stride2).unwrap();
        let id = dfa
            .special
            .min_match
            .as_usize()
            .checked_add(offset)
            .unwrap();
        let sid = StateID::new(id).unwrap();
        assert!(dfa.is_match_state(sid));
        sid
    }
    #[cfg_attr(all(), inline(always))]
    fn pattern_id(&self, state_index: usize, match_index: usize) -> PatternID {
        self.pattern_id_slice(state_index)[match_index]
    }
    #[cfg_attr(all(), inline(always))]
    fn pattern_len(&self, state_index: usize) -> usize {
        self.slices()[state_index * 2 + 1].as_usize()
    }
    #[cfg_attr(all(), inline(always))]
    fn pattern_id_slice(&self, state_index: usize) -> &[PatternID] {
        let start = self.slices()[state_index * 2].as_usize();
        let len = self.pattern_len(state_index);
        &self.pattern_ids()[start..start + len]
    }
    #[cfg_attr(all(), inline(always))]
    fn slices(&self) -> &[PatternID] {
        wire::u32s_to_pattern_ids(self.slices.as_ref())
    }
    #[cfg_attr(all(), inline(always))]
    fn len(&self) -> usize {
        assert_eq!(0, self.slices().len() % 2);
        self.slices().len() / 2
    }
    #[cfg_attr(all(), inline(always))]
    fn pattern_ids(&self) -> &[PatternID] {
        wire::u32s_to_pattern_ids(self.pattern_ids.as_ref())
    }
    fn memory_usage(&self) -> usize {
        (self.slices().len() + self.pattern_ids().len()) * PatternID::SIZE
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Flags {
    pub(crate) has_empty: bool,
    pub(crate) is_utf8: bool,
    pub(crate) is_always_start_anchored: bool,
}
impl Flags {
    fn from_nfa(nfa: &thompson::NFA) -> Flags {
        Flags {
            has_empty: nfa.has_empty(),
            is_utf8: nfa.is_utf8(),
            is_always_start_anchored: nfa.is_always_start_anchored(),
        }
    }
    pub(crate) fn from_bytes(slice: &[u8]) -> Result<(Flags, usize), DeserializeError> {
        let (bits, nread) = wire::try_read_u32(slice, "flag bitset")?;
        let flags = Flags {
            has_empty: bits & (1 << 0) != 0,
            is_utf8: bits & (1 << 1) != 0,
            is_always_start_anchored: bits & (1 << 2) != 0,
        };
        Ok((flags, nread))
    }
    pub(crate) fn write_to<E: Endian>(&self, dst: &mut [u8]) -> Result<usize, SerializeError> {
        fn bool_to_int(b: bool) -> u32 {
            if b { 1 } else { 0 }
        }
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("flag bitset"));
        }
        let bits = (bool_to_int(self.has_empty) << 0)
            | (bool_to_int(self.is_utf8) << 1)
            | (bool_to_int(self.is_always_start_anchored) << 2);
        E::write_u32(bits, dst);
        Ok(nwrite)
    }
    pub(crate) fn write_to_len(&self) -> usize {
        size_of::<u32>()
    }
}
pub(crate) struct StateIter<'a, T> {
    tt: &'a TransitionTable<T>,
    it: iter::Enumerate<slice::Chunks<'a, StateID>>,
}
impl<'a, T: AsRef<[u32]>> Iterator for StateIter<'a, T> {
    type Item = State<'a>;
    fn next(&mut self) -> Option<State<'a>> {
        self.it.next().map(|(index, _)| {
            let id = self.tt.to_state_id(index);
            self.tt.state(id)
        })
    }
}
pub(crate) struct State<'a> {
    id: StateID,
    stride2: usize,
    transitions: &'a [StateID],
}
impl<'a> State<'a> {
    pub(crate) fn transitions(&self) -> StateTransitionIter<'_> {
        StateTransitionIter {
            len: self.transitions.len(),
            it: self.transitions.iter().enumerate(),
        }
    }
    pub(crate) fn sparse_transitions(&self) -> StateSparseTransitionIter<'_> {
        StateSparseTransitionIter {
            dense: self.transitions(),
            cur: None,
        }
    }
    pub(crate) fn id(&self) -> StateID {
        self.id
    }
    fn accelerate(&self, classes: &ByteClasses) -> Option<Accel> {
        let mut accel = Accel::new();
        for (class, id) in self.transitions() {
            if id == self.id() {
                continue;
            }
            for unit in classes.elements(class) {
                if let Some(byte) = unit.as_u8() {
                    if !accel.add(byte) {
                        return None;
                    }
                }
            }
        }
        if accel.is_empty() { None } else { Some(accel) }
    }
}
impl<'a> fmt::Debug for State<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, (start, end, sid)) in self.sparse_transitions().enumerate() {
            let id = if f.alternate() {
                sid.as_usize()
            } else {
                sid.as_usize() >> self.stride2
            };
            if i > 0 {
                write!(f, ", ")?;
            }
            if start == end {
                write!(f, "{start:?} => {id:?}")?;
            } else {
                write!(f, "{start:?}-{end:?} => {id:?}")?;
            }
        }
        Ok(())
    }
}
#[derive(Debug)]
pub(crate) struct StateTransitionIter<'a> {
    len: usize,
    it: iter::Enumerate<slice::Iter<'a, StateID>>,
}
impl<'a> Iterator for StateTransitionIter<'a> {
    type Item = (alphabet::Unit, StateID);
    fn next(&mut self) -> Option<(alphabet::Unit, StateID)> {
        self.it.next().map(|(i, &id)| {
            let unit = if i + 1 == self.len {
                alphabet::Unit::eoi(i)
            } else {
                let b = u8::try_from(i).expect("raw byte alphabet is never exceeded");
                alphabet::Unit::u8(b)
            };
            (unit, id)
        })
    }
}
#[derive(Debug)]
pub(crate) struct StateSparseTransitionIter<'a> {
    dense: StateTransitionIter<'a>,
    cur: Option<(alphabet::Unit, alphabet::Unit, StateID)>,
}
impl<'a> Iterator for StateSparseTransitionIter<'a> {
    type Item = (alphabet::Unit, alphabet::Unit, StateID);
    fn next(&mut self) -> Option<(alphabet::Unit, alphabet::Unit, StateID)> {
        while let Some((unit, next)) = self.dense.next() {
            let (prev_start, prev_end, prev_next) = match self.cur {
                Some(t) => t,
                None => {
                    self.cur = Some((unit, unit, next));
                    continue;
                }
            };
            if prev_next == next && !unit.is_eoi() {
                self.cur = Some((prev_start, unit, prev_next));
            } else {
                self.cur = Some((unit, unit, next));
                if prev_next != DEAD {
                    return Some((prev_start, prev_end, prev_next));
                }
            }
        }
        if let Some((start, end, next)) = self.cur.take() {
            if next != DEAD {
                return Some((start, end, next));
            }
        }
        None
    }
}
#[derive(Clone, Debug)]
pub struct BuildError {
    kind: BuildErrorKind,
}
impl BuildError {}
#[derive(Clone, Debug)]
enum BuildErrorKind {
    NFA(thompson::BuildError),
    Unsupported(&'static str),
    TooManyStates,
    TooManyStartStates,
    TooManyMatchPatternIDs,
    DFAExceededSizeLimit { limit: usize },
    DeterminizeExceededSizeLimit { limit: usize },
}
impl BuildError {
    fn kind(&self) -> &BuildErrorKind {
        &self.kind
    }
    pub(crate) fn nfa(err: thompson::BuildError) -> BuildError {
        BuildError {
            kind: BuildErrorKind::NFA(err),
        }
    }
    pub(crate) fn unsupported_dfa_word_boundary_unicode() -> BuildError {
        let msg = "cannot build DFAs for regexes with Unicode word \
                   boundaries; switch to ASCII word boundaries, or \
                   heuristically enable Unicode word boundaries or use a \
                   different regex engine";
        BuildError {
            kind: BuildErrorKind::Unsupported(msg),
        }
    }
    pub(crate) fn too_many_states() -> BuildError {
        BuildError {
            kind: BuildErrorKind::TooManyStates,
        }
    }
    pub(crate) fn too_many_start_states() -> BuildError {
        BuildError {
            kind: BuildErrorKind::TooManyStartStates,
        }
    }
    pub(crate) fn too_many_match_pattern_ids() -> BuildError {
        BuildError {
            kind: BuildErrorKind::TooManyMatchPatternIDs,
        }
    }
    pub(crate) fn dfa_exceeded_size_limit(limit: usize) -> BuildError {
        BuildError {
            kind: BuildErrorKind::DFAExceededSizeLimit { limit },
        }
    }
    pub(crate) fn determinize_exceeded_size_limit(limit: usize) -> BuildError {
        BuildError {
            kind: BuildErrorKind::DeterminizeExceededSizeLimit { limit },
        }
    }
}
impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind() {
            BuildErrorKind::NFA(err) => Some(err),
            _ => None,
        }
    }
}
impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind() {
            BuildErrorKind::NFA(_) => write!(f, "error building NFA"),
            BuildErrorKind::Unsupported(msg) => {
                write!(f, "unsupported regex feature for DFAs: {msg}")
            }
            BuildErrorKind::TooManyStates => write!(
                f,
                "number of DFA states exceeds limit of {}",
                StateID::LIMIT,
            ),
            BuildErrorKind::TooManyStartStates => {
                let stride = Start::len();
                let max = usize::try_from(core::isize::MAX).unwrap();
                let limit = (max - stride) / stride;
                write!(
                    f,
                    "compiling DFA with start states exceeds pattern \
                     pattern limit of {}",
                    limit,
                )
            }
            BuildErrorKind::TooManyMatchPatternIDs => write!(
                f,
                "compiling DFA with total patterns in all match states \
                 exceeds limit of {}",
                PatternID::LIMIT,
            ),
            BuildErrorKind::DFAExceededSizeLimit { limit } => write!(
                f,
                "DFA exceeded size limit of {limit:?} during determinization",
            ),
            BuildErrorKind::DeterminizeExceededSizeLimit { limit } => {
                write!(f, "determinization exceeded size limit of {limit:?}")
            }
        }
    }
}
