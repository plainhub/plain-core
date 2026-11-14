use crate::regex_engine::aho::{
    automaton::Automaton,
    nfa::noncontiguous,
    util::{
        alphabet::ByteClasses,
        error::{BuildError, MatchError},
        prefilter::Prefilter,
        primitives::{IteratorIndexExt, PatternID, SmallIndex, StateID},
        search::{Anchored, MatchKind},
        special::Special,
    },
};
use crate::regex_engine::int::{U16, U32, Usize};
use std::{vec, vec::Vec};
#[derive(Clone)]
pub struct NFA {
    repr: Vec<u32>,
    pattern_lens: Vec<SmallIndex>,
    state_len: usize,
    prefilter: Option<Prefilter>,
    match_kind: MatchKind,
    alphabet_len: usize,
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
    const DEAD: StateID = StateID::new_unchecked(0);
    const FAIL: StateID = StateID::new_unchecked(1);
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
        let repr = &self.repr;
        let class = self.byte_classes.get(byte);
        let u32tosid = StateID::from_u32_unchecked;
        loop {
            let o = sid.as_usize();
            let kind = repr[o] & 0xFF;
            if kind == State::KIND_DENSE {
                let next = u32tosid(repr[o + 2 + usize::from(class)]);
                if next != NFA::FAIL {
                    return next;
                }
            } else if kind == State::KIND_ONE {
                if class == repr[o].low_u16().high_u8() {
                    return u32tosid(repr[o + 2]);
                }
            } else {
                let trans_len = kind.as_usize();
                let classes_len = u32_len(trans_len);
                let trans_offset = o + 2 + classes_len;
                for (i, &chunk) in repr[o + 2..][..classes_len].iter().enumerate() {
                    let classes = chunk.to_ne_bytes();
                    if classes[0] == class {
                        return u32tosid(repr[trans_offset + i * 4]);
                    }
                    if classes[1] == class {
                        return u32tosid(repr[trans_offset + i * 4 + 1]);
                    }
                    if classes[2] == class {
                        return u32tosid(repr[trans_offset + i * 4 + 2]);
                    }
                    if classes[3] == class {
                        return u32tosid(repr[trans_offset + i * 4 + 3]);
                    }
                }
            }
            if anchored.is_anchored() {
                return NFA::DEAD;
            }
            sid = u32tosid(repr[o + 1]);
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
        State::match_len(self.alphabet_len, &self.repr[sid.as_usize()..])
    }
    #[inline(always)]
    fn match_pattern(&self, sid: StateID, index: usize) -> PatternID {
        State::match_pattern(self.alphabet_len, &self.repr[sid.as_usize()..], index)
    }
    #[inline(always)]
    fn memory_usage(&self) -> usize {
        use core::mem::size_of;
        (self.repr.len() * size_of::<u32>())
            + (self.pattern_lens.len() * size_of::<SmallIndex>())
            + self.prefilter.as_ref().map_or(0, |p| p.memory_usage())
    }
    #[inline(always)]
    fn prefilter(&self) -> Option<&Prefilter> {
        self.prefilter.as_ref()
    }
}
impl core::fmt::Debug for NFA {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::regex_engine::aho::automaton::fmt_state_indicator;
        writeln!(f, "contiguous::NFA(")?;
        let mut sid = NFA::DEAD;
        loop {
            let raw = &self.repr[sid.as_usize()..];
            if raw.is_empty() {
                break;
            }
            let is_match = self.is_match(sid);
            let state = State::read(self.alphabet_len, is_match, raw);
            fmt_state_indicator(f, self, sid)?;
            write!(f, "{:06}({:06}): ", sid.as_usize(), state.fail.as_usize())?;
            state.fmt(f)?;
            write!(f, "\n")?;
            if self.is_match(sid) {
                write!(f, "         matches: ")?;
                for i in 0..state.match_len {
                    let pid = State::match_pattern(self.alphabet_len, raw, i);
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", pid.as_usize())?;
                }
                write!(f, "\n")?;
            }
            if sid == NFA::DEAD {
                writeln!(f, "F {:06}:", NFA::FAIL.as_usize())?;
            }
            let len = State::len(self.alphabet_len, is_match, raw);
            sid = StateID::new(sid.as_usize().checked_add(len).unwrap()).unwrap();
        }
        writeln!(f, "match kind: {:?}", self.match_kind)?;
        writeln!(f, "prefilter: {:?}", self.prefilter.is_some())?;
        writeln!(f, "state length: {:?}", self.state_len)?;
        writeln!(f, "pattern length: {:?}", self.patterns_len())?;
        writeln!(f, "shortest pattern length: {:?}", self.min_pattern_len)?;
        writeln!(f, "longest pattern length: {:?}", self.max_pattern_len)?;
        writeln!(f, "alphabet length: {:?}", self.alphabet_len)?;
        writeln!(f, "byte classes: {:?}", self.byte_classes)?;
        writeln!(f, "memory usage: {:?}", self.memory_usage())?;
        writeln!(f, ")")?;
        Ok(())
    }
}
#[derive(Clone)]
struct State<'a> {
    fail: StateID,
    match_len: usize,
    trans: StateTrans<'a>,
}
#[derive(Clone)]
enum StateTrans<'a> {
    Sparse {
        classes: &'a [u32],
        nexts: &'a [u32],
    },
    One {
        class: u8,
        next: u32,
    },
    Dense {
        class_to_next: &'a [u32],
    },
}
impl<'a> State<'a> {
    const KIND: usize = 0;
    const KIND_DENSE: u32 = 0xFF;
    const KIND_ONE: u32 = 0xFE;
    const MAX_SPARSE_TRANSITIONS: usize = 127;
    fn remap(
        alphabet_len: usize,
        old_to_new: &[StateID],
        state: &mut [u32],
    ) -> Result<(), BuildError> {
        let kind = State::kind(state);
        if kind == State::KIND_DENSE {
            state[1] = old_to_new[state[1].as_usize()].as_u32();
            for next in state[2..][..alphabet_len].iter_mut() {
                *next = old_to_new[next.as_usize()].as_u32();
            }
        } else if kind == State::KIND_ONE {
            state[1] = old_to_new[state[1].as_usize()].as_u32();
            state[2] = old_to_new[state[2].as_usize()].as_u32();
        } else {
            let trans_len = State::sparse_trans_len(state);
            let classes_len = u32_len(trans_len);
            state[1] = old_to_new[state[1].as_usize()].as_u32();
            for next in state[2 + classes_len..][..trans_len].iter_mut() {
                *next = old_to_new[next.as_usize()].as_u32();
            }
        }
        Ok(())
    }
    fn len(alphabet_len: usize, is_match: bool, state: &[u32]) -> usize {
        let kind_len = 1;
        let fail_len = 1;
        let kind = State::kind(state);
        let (classes_len, trans_len) = if kind == State::KIND_DENSE {
            (0, alphabet_len)
        } else if kind == State::KIND_ONE {
            (0, 1)
        } else {
            let trans_len = State::sparse_trans_len(state);
            let classes_len = u32_len(trans_len);
            (classes_len, trans_len)
        };
        let match_len = if !is_match {
            0
        } else if State::match_len(alphabet_len, state) == 1 {
            1
        } else {
            1 + State::match_len(alphabet_len, state)
        };
        kind_len + fail_len + classes_len + trans_len + match_len
    }
    #[inline(always)]
    fn kind(state: &[u32]) -> u32 {
        state[State::KIND] & 0xFF
    }
    #[inline(always)]
    fn sparse_trans_len(state: &[u32]) -> usize {
        (state[State::KIND] & 0xFF).as_usize()
    }
    #[inline(always)]
    fn match_len(alphabet_len: usize, state: &[u32]) -> usize {
        let packed = if State::kind(state) == State::KIND_DENSE {
            let start = 2 + alphabet_len;
            state[start].as_usize()
        } else {
            let trans_len = State::sparse_trans_len(state);
            let classes_len = u32_len(trans_len);
            let start = 2 + classes_len + trans_len;
            state[start].as_usize()
        };
        if packed & (1 << 31) == 0 { packed } else { 1 }
    }
    #[inline(always)]
    fn match_pattern(alphabet_len: usize, state: &[u32], index: usize) -> PatternID {
        let start = if State::kind(state) == State::KIND_DENSE {
            2 + alphabet_len
        } else {
            let trans_len = State::sparse_trans_len(state);
            let classes_len = u32_len(trans_len);
            2 + classes_len + trans_len
        };
        let packed = state[start];
        let pid = if packed & (1 << 31) == 0 {
            state[start + 1 + index]
        } else {
            assert_eq!(0, index);
            packed & !(1 << 31)
        };
        PatternID::from_u32_unchecked(pid)
    }
    fn read(alphabet_len: usize, is_match: bool, state: &'a [u32]) -> State<'a> {
        let kind = State::kind(state);
        let match_len = if !is_match {
            0
        } else {
            State::match_len(alphabet_len, state)
        };
        let (trans, fail) = if kind == State::KIND_DENSE {
            let fail = StateID::from_u32_unchecked(state[1]);
            let class_to_next = &state[2..][..alphabet_len];
            (StateTrans::Dense { class_to_next }, fail)
        } else if kind == State::KIND_ONE {
            let fail = StateID::from_u32_unchecked(state[1]);
            let class = state[State::KIND].low_u16().high_u8();
            let next = state[2];
            (StateTrans::One { class, next }, fail)
        } else {
            let fail = StateID::from_u32_unchecked(state[1]);
            let trans_len = State::sparse_trans_len(state);
            let classes_len = u32_len(trans_len);
            let classes = &state[2..][..classes_len];
            let nexts = &state[2 + classes_len..][..trans_len];
            (StateTrans::Sparse { classes, nexts }, fail)
        };
        State {
            fail,
            match_len,
            trans,
        }
    }
    fn write(
        nnfa: &noncontiguous::NFA,
        oldsid: StateID,
        old: &noncontiguous::State,
        classes: &ByteClasses,
        dst: &mut Vec<u32>,
        force_dense: bool,
    ) -> Result<StateID, BuildError> {
        let sid = StateID::new(dst.len())
            .map_err(|e| BuildError::state_id_overflow(StateID::MAX.as_u64(), e.attempted()))?;
        let old_len = nnfa.iter_trans(oldsid).count();
        let kind = if force_dense || old_len > State::MAX_SPARSE_TRANSITIONS {
            State::KIND_DENSE
        } else if old_len == 1 && !old.is_match() {
            State::KIND_ONE
        } else {
            u32::try_from(old_len).unwrap()
        };
        if kind == State::KIND_DENSE {
            dst.push(kind);
            dst.push(old.fail().as_u32());
            State::write_dense_trans(nnfa, oldsid, classes, dst)?;
        } else if kind == State::KIND_ONE {
            let t = nnfa.iter_trans(oldsid).next().unwrap();
            let class = u32::from(classes.get(t.byte()));
            dst.push(kind | (class << 8));
            dst.push(old.fail().as_u32());
            dst.push(t.next().as_u32());
        } else {
            dst.push(kind);
            dst.push(old.fail().as_u32());
            State::write_sparse_trans(nnfa, oldsid, classes, dst)?;
        }
        if old.is_match() {
            let matches_len = nnfa.iter_matches(oldsid).count();
            if matches_len == 1 {
                let pid = nnfa.iter_matches(oldsid).next().unwrap().as_u32();
                assert_eq!(0, pid & (1 << 31));
                dst.push((1 << 31) | pid);
            } else {
                assert_eq!(0, matches_len & (1 << 31));
                dst.push(matches_len.as_u32());
                dst.extend(nnfa.iter_matches(oldsid).map(|pid| pid.as_u32()));
            }
        }
        Ok(sid)
    }
    fn write_sparse_trans(
        nnfa: &noncontiguous::NFA,
        oldsid: StateID,
        classes: &ByteClasses,
        dst: &mut Vec<u32>,
    ) -> Result<(), BuildError> {
        let (mut chunk, mut len) = ([0; 4], 0);
        for t in nnfa.iter_trans(oldsid) {
            chunk[len] = classes.get(t.byte());
            len += 1;
            if len == 4 {
                dst.push(u32::from_ne_bytes(chunk));
                chunk = [0; 4];
                len = 0;
            }
        }
        if len > 0 {
            let repeat = chunk[len - 1];
            while len < 4 {
                chunk[len] = repeat;
                len += 1;
            }
            dst.push(u32::from_ne_bytes(chunk));
        }
        for t in nnfa.iter_trans(oldsid) {
            dst.push(t.next().as_u32());
        }
        Ok(())
    }
    fn write_dense_trans(
        nnfa: &noncontiguous::NFA,
        oldsid: StateID,
        classes: &ByteClasses,
        dst: &mut Vec<u32>,
    ) -> Result<(), BuildError> {
        let start = dst.len();
        dst.extend(
            core::iter::repeat(noncontiguous::NFA::FAIL.as_u32()).take(classes.alphabet_len()),
        );
        assert!(start < dst.len(), "equivalence classes are never empty");
        for t in nnfa.iter_trans(oldsid) {
            dst[start + usize::from(classes.get(t.byte()))] = t.next().as_u32();
        }
        Ok(())
    }
    fn transitions(&self) -> impl Iterator<Item = (u8, StateID)> + '_ {
        let mut i = 0;
        core::iter::from_fn(move || match self.trans {
            StateTrans::Sparse { classes, nexts } => {
                if i >= nexts.len() {
                    return None;
                }
                let chunk = classes[i / 4];
                let class = chunk.to_ne_bytes()[i % 4];
                let next = StateID::from_u32_unchecked(nexts[i]);
                i += 1;
                Some((class, next))
            }
            StateTrans::One { class, next } => {
                if i == 0 {
                    i += 1;
                    Some((class, StateID::from_u32_unchecked(next)))
                } else {
                    None
                }
            }
            StateTrans::Dense { class_to_next } => {
                if i >= class_to_next.len() {
                    return None;
                }
                let class = i.as_u8();
                let next = StateID::from_u32_unchecked(class_to_next[i]);
                i += 1;
                Some((class, next))
            }
        })
    }
}
impl<'a> core::fmt::Debug for State<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use crate::regex_engine::aho::{automaton::sparse_transitions, util::debug::DebugByte};
        let it = sparse_transitions(self.transitions())
            .filter(|&(_, _, sid)| sid != NFA::FAIL)
            .enumerate();
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
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Builder {
    noncontiguous: noncontiguous::Builder,
    dense_depth: usize,
    byte_classes: bool,
}
impl Default for Builder {
    fn default() -> Builder {
        Builder {
            noncontiguous: noncontiguous::Builder::new(),
            dense_depth: 2,
            byte_classes: true,
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
        let nnfa = self.noncontiguous.build(patterns)?;
        self.build_from_noncontiguous(&nnfa)
    }
    pub fn build_from_noncontiguous(&self, nnfa: &noncontiguous::NFA) -> Result<NFA, BuildError> {
        debug!("building contiguous NFA");
        let byte_classes = if self.byte_classes {
            nnfa.byte_classes().clone()
        } else {
            ByteClasses::singletons()
        };
        let mut index_to_state_id = vec![NFA::DEAD; nnfa.states().len()];
        let mut nfa = NFA {
            repr: vec![],
            pattern_lens: nnfa.pattern_lens_raw().to_vec(),
            state_len: nnfa.states().len(),
            prefilter: nnfa.prefilter().map(|p| p.clone()),
            match_kind: nnfa.match_kind(),
            alphabet_len: byte_classes.alphabet_len(),
            byte_classes,
            min_pattern_len: nnfa.min_pattern_len(),
            max_pattern_len: nnfa.max_pattern_len(),
            special: Special::zero(),
        };
        for (oldsid, state) in nnfa.states().iter().with_state_ids() {
            if oldsid == noncontiguous::NFA::FAIL {
                index_to_state_id[oldsid] = NFA::FAIL;
                continue;
            }
            let force_dense = state.depth().as_usize() < self.dense_depth;
            let newsid = State::write(
                nnfa,
                oldsid,
                state,
                &nfa.byte_classes,
                &mut nfa.repr,
                force_dense,
            )?;
            index_to_state_id[oldsid] = newsid;
        }
        for &newsid in index_to_state_id.iter() {
            if newsid == NFA::FAIL {
                continue;
            }
            let state = &mut nfa.repr[newsid.as_usize()..];
            State::remap(nfa.alphabet_len, &index_to_state_id, state)?;
        }
        let remap = &index_to_state_id;
        let old = nnfa.special();
        let new = &mut nfa.special;
        new.max_special_id = remap[old.max_special_id];
        new.max_match_id = remap[old.max_match_id];
        new.start_unanchored_id = remap[old.start_unanchored_id];
        new.start_anchored_id = remap[old.start_anchored_id];
        debug!(
            "contiguous NFA built, <states: {:?}, size: {:?}, \
             alphabet len: {:?}>",
            nfa.state_len,
            nfa.memory_usage(),
            nfa.byte_classes.alphabet_len(),
        );
        nfa.repr.shrink_to_fit();
        nfa.pattern_lens.shrink_to_fit();
        Ok(nfa)
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
    pub fn dense_depth(&mut self, depth: usize) -> &mut Builder {
        self.dense_depth = depth;
        self
    }
    pub fn byte_classes(&mut self, yes: bool) -> &mut Builder {
        self.byte_classes = yes;
        self
    }
}
fn u32_len(ntrans: usize) -> usize {
    if ntrans % 4 == 0 {
        ntrans >> 2
    } else {
        (ntrans >> 2) + 1
    }
}
