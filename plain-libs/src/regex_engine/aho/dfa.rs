use crate::regex_engine::aho::{
    automaton::Automaton,
    nfa::noncontiguous,
    util::{
        alphabet::ByteClasses,
        error::{BuildError, MatchError},
        prefilter::Prefilter,
        primitives::{IteratorIndexExt, PatternID, SmallIndex, StateID},
        search::{Anchored, MatchKind, StartKind},
        special::Special,
    },
};
use crate::regex_engine::int::{U32, Usize};
use std::{vec, vec::Vec};
#[derive(Clone)]
pub struct DFA {
    trans: Vec<StateID>,
    matches: Vec<Vec<PatternID>>,
    matches_memory_usage: usize,
    pattern_lens: Vec<SmallIndex>,
    prefilter: Option<Prefilter>,
    match_kind: MatchKind,
    state_len: usize,
    alphabet_len: usize,
    stride2: usize,
    byte_classes: ByteClasses,
    min_pattern_len: usize,
    max_pattern_len: usize,
    special: Special,
}
impl DFA {
    pub fn new<I, P>(patterns: I) -> Result<DFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        DFA::builder().build(patterns)
    }
    pub fn builder() -> Builder {
        Builder::new()
    }
}
impl DFA {
    const DEAD: StateID = StateID::new_unchecked(0);
    fn set_matches(&mut self, sid: StateID, pids: impl Iterator<Item = PatternID>) {
        let index = (sid.as_usize() >> self.stride2).checked_sub(2).unwrap();
        let mut at_least_one = false;
        for pid in pids {
            self.matches[index].push(pid);
            self.matches_memory_usage += PatternID::SIZE;
            at_least_one = true;
        }
        assert!(at_least_one, "match state must have non-empty pids");
    }
}
unsafe impl Automaton for DFA {
    #[inline(always)]
    fn start_state(&self, anchored: Anchored) -> Result<StateID, MatchError> {
        match anchored {
            Anchored::No => {
                let start = self.special.start_unanchored_id;
                if start == DFA::DEAD {
                    Err(MatchError::invalid_input_unanchored())
                } else {
                    Ok(start)
                }
            }
            Anchored::Yes => {
                let start = self.special.start_anchored_id;
                if start == DFA::DEAD {
                    Err(MatchError::invalid_input_anchored())
                } else {
                    Ok(start)
                }
            }
        }
    }
    #[inline(always)]
    fn next_state(&self, _anchored: Anchored, sid: StateID, byte: u8) -> StateID {
        let class = self.byte_classes.get(byte);
        self.trans[(sid.as_u32() + u32::from(class)).as_usize()]
    }
    #[inline(always)]
    fn is_special(&self, sid: StateID) -> bool {
        sid <= self.special.max_special_id
    }
    #[inline(always)]
    fn is_dead(&self, sid: StateID) -> bool {
        sid == DFA::DEAD
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
        debug_assert!(self.is_match(sid));
        let offset = (sid.as_usize() >> self.stride2) - 2;
        self.matches[offset].len()
    }
    #[inline(always)]
    fn match_pattern(&self, sid: StateID, index: usize) -> PatternID {
        debug_assert!(self.is_match(sid));
        let offset = (sid.as_usize() >> self.stride2) - 2;
        self.matches[offset][index]
    }
    #[inline(always)]
    fn memory_usage(&self) -> usize {
        use core::mem::size_of;
        (self.trans.len() * size_of::<u32>())
            + (self.matches.len() * size_of::<Vec<PatternID>>())
            + self.matches_memory_usage
            + (self.pattern_lens.len() * size_of::<SmallIndex>())
            + self.prefilter.as_ref().map_or(0, |p| p.memory_usage())
    }
    #[inline(always)]
    fn prefilter(&self) -> Option<&Prefilter> {
        self.prefilter.as_ref()
    }
}
impl core::fmt::Debug for DFA {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::regex_engine::aho::{
            automaton::{fmt_state_indicator, sparse_transitions},
            util::debug::DebugByte,
        };
        writeln!(f, "dfa::DFA(")?;
        for index in 0..self.state_len {
            let sid = StateID::new_unchecked(index << self.stride2);
            if index == 1 {
                writeln!(f, "F {:06}:", sid.as_usize())?;
                continue;
            }
            fmt_state_indicator(f, self, sid)?;
            write!(f, "{:06}: ", sid.as_usize())?;
            let it = (0..self.byte_classes.alphabet_len())
                .map(|class| (class.as_u8(), self.trans[sid.as_usize() + class]));
            for (i, (start, end, next)) in sparse_transitions(it).enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                if start == end {
                    write!(f, "{:?} => {:?}", DebugByte(start), next.as_usize())?;
                } else {
                    write!(
                        f,
                        "{:?}-{:?} => {:?}",
                        DebugByte(start),
                        DebugByte(end),
                        next.as_usize()
                    )?;
                }
            }
            write!(f, "\n")?;
            if self.is_match(sid) {
                write!(f, " matches: ")?;
                for i in 0..self.match_len(sid) {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    let pid = self.match_pattern(sid, i);
                    write!(f, "{}", pid.as_usize())?;
                }
                write!(f, "\n")?;
            }
        }
        writeln!(f, "match kind: {:?}", self.match_kind)?;
        writeln!(f, "prefilter: {:?}", self.prefilter.is_some())?;
        writeln!(f, "state length: {:?}", self.state_len)?;
        writeln!(f, "pattern length: {:?}", self.patterns_len())?;
        writeln!(f, "shortest pattern length: {:?}", self.min_pattern_len)?;
        writeln!(f, "longest pattern length: {:?}", self.max_pattern_len)?;
        writeln!(f, "alphabet length: {:?}", self.alphabet_len)?;
        writeln!(f, "stride: {:?}", 1 << self.stride2)?;
        writeln!(f, "byte classes: {:?}", self.byte_classes)?;
        writeln!(f, "memory usage: {:?}", self.memory_usage())?;
        writeln!(f, ")")?;
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Builder {
    noncontiguous: noncontiguous::Builder,
    start_kind: StartKind,
    byte_classes: bool,
}
impl Default for Builder {
    fn default() -> Builder {
        Builder {
            noncontiguous: noncontiguous::Builder::new(),
            start_kind: StartKind::Unanchored,
            byte_classes: true,
        }
    }
}
impl Builder {
    pub fn new() -> Builder {
        Builder::default()
    }
    pub fn build<I, P>(&self, patterns: I) -> Result<DFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        let nnfa = self.noncontiguous.build(patterns)?;
        self.build_from_noncontiguous(&nnfa)
    }
    pub fn build_from_noncontiguous(&self, nnfa: &noncontiguous::NFA) -> Result<DFA, BuildError> {
        debug!("building DFA");
        let byte_classes = if self.byte_classes {
            nnfa.byte_classes().clone()
        } else {
            ByteClasses::singletons()
        };
        let state_len = match self.start_kind {
            StartKind::Unanchored | StartKind::Anchored => nnfa.states().len(),
            StartKind::Both => nnfa
                .states()
                .len()
                .checked_mul(2)
                .unwrap()
                .checked_sub(4)
                .unwrap(),
        };
        let trans_len = match state_len.checked_shl(byte_classes.stride2().as_u32()) {
            Some(trans_len) => trans_len,
            None => {
                return Err(BuildError::state_id_overflow(
                    StateID::MAX.as_u64(),
                    usize::MAX.as_u64(),
                ));
            }
        };
        StateID::new(trans_len.checked_sub(byte_classes.stride()).unwrap())
            .map_err(|e| BuildError::state_id_overflow(StateID::MAX.as_u64(), e.attempted()))?;
        let num_match_states = match self.start_kind {
            StartKind::Unanchored | StartKind::Anchored => nnfa
                .special()
                .max_match_id
                .as_usize()
                .checked_sub(1)
                .unwrap(),
            StartKind::Both => nnfa
                .special()
                .max_match_id
                .as_usize()
                .checked_sub(1)
                .unwrap()
                .checked_mul(2)
                .unwrap(),
        };
        let mut dfa = DFA {
            trans: vec![DFA::DEAD; trans_len],
            matches: vec![vec![]; num_match_states],
            matches_memory_usage: 0,
            pattern_lens: nnfa.pattern_lens_raw().to_vec(),
            prefilter: nnfa.prefilter().cloned(),
            match_kind: nnfa.match_kind(),
            state_len,
            alphabet_len: byte_classes.alphabet_len(),
            stride2: byte_classes.stride2(),
            byte_classes,
            min_pattern_len: nnfa.min_pattern_len(),
            max_pattern_len: nnfa.max_pattern_len(),
            special: Special::zero(),
        };
        match self.start_kind {
            StartKind::Both => {
                self.finish_build_both_starts(nnfa, &mut dfa);
            }
            StartKind::Unanchored => {
                self.finish_build_one_start(Anchored::No, nnfa, &mut dfa);
            }
            StartKind::Anchored => self.finish_build_one_start(Anchored::Yes, nnfa, &mut dfa),
        }
        debug!(
            "DFA built, <states: {:?}, size: {:?}, \
             alphabet len: {:?}, stride: {:?}>",
            dfa.state_len,
            dfa.memory_usage(),
            dfa.byte_classes.alphabet_len(),
            dfa.byte_classes.stride(),
        );
        dfa.trans.shrink_to_fit();
        dfa.pattern_lens.shrink_to_fit();
        dfa.matches.shrink_to_fit();
        Ok(dfa)
    }
    fn finish_build_one_start(&self, anchored: Anchored, nnfa: &noncontiguous::NFA, dfa: &mut DFA) {
        let stride2 = dfa.stride2;
        let old2new = |oldsid: StateID| StateID::new_unchecked(oldsid.as_usize() << stride2);
        for (oldsid, state) in nnfa.states().iter().with_state_ids() {
            let newsid = old2new(oldsid);
            if state.is_match() {
                dfa.set_matches(newsid, nnfa.iter_matches(oldsid));
            }
            sparse_iter(
                nnfa,
                oldsid,
                &dfa.byte_classes,
                |byte, class, mut oldnextsid| {
                    if oldnextsid == noncontiguous::NFA::FAIL {
                        if anchored.is_anchored() {
                            oldnextsid = noncontiguous::NFA::DEAD;
                        } else if state.fail() == noncontiguous::NFA::DEAD {
                            oldnextsid = noncontiguous::NFA::DEAD;
                        } else {
                            oldnextsid = nnfa.next_state(Anchored::No, state.fail(), byte);
                        }
                    }
                    dfa.trans[newsid.as_usize() + usize::from(class)] = old2new(oldnextsid);
                },
            );
        }
        let old = nnfa.special();
        let new = &mut dfa.special;
        new.max_special_id = old2new(old.max_special_id);
        new.max_match_id = old2new(old.max_match_id);
        if anchored.is_anchored() {
            new.start_unanchored_id = DFA::DEAD;
            new.start_anchored_id = old2new(old.start_anchored_id);
        } else {
            new.start_unanchored_id = old2new(old.start_unanchored_id);
            new.start_anchored_id = DFA::DEAD;
        }
    }
    fn finish_build_both_starts(&self, nnfa: &noncontiguous::NFA, dfa: &mut DFA) {
        let stride2 = dfa.stride2;
        let stride = 1 << stride2;
        let mut remap_unanchored = vec![DFA::DEAD; nnfa.states().len()];
        let mut remap_anchored = vec![DFA::DEAD; nnfa.states().len()];
        let mut is_anchored = vec![false; dfa.state_len];
        let mut newsid = DFA::DEAD;
        let next_dfa_id = |sid: StateID| StateID::new_unchecked(sid.as_usize() + stride);
        for (oldsid, state) in nnfa.states().iter().with_state_ids() {
            if oldsid == noncontiguous::NFA::DEAD || oldsid == noncontiguous::NFA::FAIL {
                remap_unanchored[oldsid] = newsid;
                remap_anchored[oldsid] = newsid;
                newsid = next_dfa_id(newsid);
            } else if oldsid == nnfa.special().start_unanchored_id
                || oldsid == nnfa.special().start_anchored_id
            {
                if oldsid == nnfa.special().start_unanchored_id {
                    remap_unanchored[oldsid] = newsid;
                    remap_anchored[oldsid] = DFA::DEAD;
                } else {
                    remap_unanchored[oldsid] = DFA::DEAD;
                    remap_anchored[oldsid] = newsid;
                    is_anchored[newsid.as_usize() >> stride2] = true;
                }
                if state.is_match() {
                    dfa.set_matches(newsid, nnfa.iter_matches(oldsid));
                }
                sparse_iter(nnfa, oldsid, &dfa.byte_classes, |_, class, oldnextsid| {
                    let class = usize::from(class);
                    if oldnextsid == noncontiguous::NFA::FAIL {
                        dfa.trans[newsid.as_usize() + class] = DFA::DEAD;
                    } else {
                        dfa.trans[newsid.as_usize() + class] = oldnextsid;
                    }
                });
                newsid = next_dfa_id(newsid);
            } else {
                let unewsid = newsid;
                newsid = next_dfa_id(newsid);
                let anewsid = newsid;
                newsid = next_dfa_id(newsid);
                remap_unanchored[oldsid] = unewsid;
                remap_anchored[oldsid] = anewsid;
                is_anchored[anewsid.as_usize() >> stride2] = true;
                if state.is_match() {
                    dfa.set_matches(unewsid, nnfa.iter_matches(oldsid));
                    dfa.set_matches(anewsid, nnfa.iter_matches(oldsid));
                }
                sparse_iter(
                    nnfa,
                    oldsid,
                    &dfa.byte_classes,
                    |byte, class, oldnextsid| {
                        let class = usize::from(class);
                        if oldnextsid == noncontiguous::NFA::FAIL {
                            let oldnextsid = if state.fail() == noncontiguous::NFA::DEAD {
                                noncontiguous::NFA::DEAD
                            } else {
                                nnfa.next_state(Anchored::No, state.fail(), byte)
                            };
                            dfa.trans[unewsid.as_usize() + class] = oldnextsid;
                        } else {
                            dfa.trans[unewsid.as_usize() + class] = oldnextsid;
                            dfa.trans[anewsid.as_usize() + class] = oldnextsid;
                        }
                    },
                );
            }
        }
        for i in 0..dfa.state_len {
            let sid = i << stride2;
            if is_anchored[i] {
                for next in dfa.trans[sid..][..stride].iter_mut() {
                    *next = remap_anchored[*next];
                }
            } else {
                for next in dfa.trans[sid..][..stride].iter_mut() {
                    *next = remap_unanchored[*next];
                }
            }
        }
        let old = nnfa.special();
        let new = &mut dfa.special;
        new.max_special_id = remap_anchored[old.max_special_id];
        new.max_match_id = remap_anchored[old.max_match_id];
        new.start_unanchored_id = remap_unanchored[old.start_unanchored_id];
        new.start_anchored_id = remap_anchored[old.start_anchored_id];
    }
    pub fn match_kind(&mut self, kind: MatchKind) -> &mut Builder {
        self.noncontiguous.match_kind(kind);
        self
    }
    pub fn ascii_case_insensitive(&mut self, yes: bool) -> &mut Builder {
        self.noncontiguous.ascii_case_insensitive(yes);
        self
    }
    pub fn prefilter(&mut self, yes: bool) -> &mut Builder {
        self.noncontiguous.prefilter(yes);
        self
    }
    pub fn start_kind(&mut self, kind: StartKind) -> &mut Builder {
        self.start_kind = kind;
        self
    }
    pub fn byte_classes(&mut self, yes: bool) -> &mut Builder {
        self.byte_classes = yes;
        self
    }
}
fn sparse_iter<F: FnMut(u8, u8, StateID)>(
    nnfa: &noncontiguous::NFA,
    oldsid: StateID,
    classes: &ByteClasses,
    mut f: F,
) {
    let mut prev_class = None;
    let mut byte = 0usize;
    for t in nnfa.iter_trans(oldsid) {
        while byte < usize::from(t.byte()) {
            let rep = byte.as_u8();
            let class = classes.get(rep);
            byte += 1;
            if prev_class != Some(class) {
                f(rep, class, noncontiguous::NFA::FAIL);
                prev_class = Some(class);
            }
        }
        let rep = t.byte();
        let class = classes.get(rep);
        byte += 1;
        if prev_class != Some(class) {
            f(rep, class, t.next());
            prev_class = Some(class);
        }
    }
    for b in byte..=255 {
        let rep = b.as_u8();
        let class = classes.get(rep);
        if prev_class != Some(class) {
            f(rep, class, noncontiguous::NFA::FAIL);
            prev_class = Some(class);
        }
    }
}
