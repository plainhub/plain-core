use crate::regex_engine::automata::util::{
    int::{I32, U32},
    look::LookSet,
    primitives::{PatternID, StateID},
    wire::{self, Endian},
};
use core::mem;
use std::{sync::Arc, vec::Vec};
#[derive(Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub(crate) struct State(Arc<[u8]>);
impl core::borrow::Borrow<[u8]> for State {
    fn borrow(&self) -> &[u8] {
        &self.0
    }
}
impl core::fmt::Debug for State {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_tuple("State").field(&self.repr()).finish()
    }
}
impl State {
    pub(crate) fn dead() -> State {
        StateBuilderEmpty::new()
            .into_matches()
            .into_nfa()
            .to_state()
    }
    pub(crate) fn is_match(&self) -> bool {
        self.repr().is_match()
    }
    pub(crate) fn is_from_word(&self) -> bool {
        self.repr().is_from_word()
    }
    pub(crate) fn is_half_crlf(&self) -> bool {
        self.repr().is_half_crlf()
    }
    pub(crate) fn look_have(&self) -> LookSet {
        self.repr().look_have()
    }
    pub(crate) fn look_need(&self) -> LookSet {
        self.repr().look_need()
    }
    pub(crate) fn match_len(&self) -> usize {
        self.repr().match_len()
    }
    pub(crate) fn match_pattern(&self, index: usize) -> PatternID {
        self.repr().match_pattern(index)
    }
    pub(crate) fn match_pattern_ids(&self) -> Option<Vec<PatternID>> {
        self.repr().match_pattern_ids()
    }
    pub(crate) fn iter_nfa_state_ids<F: FnMut(StateID)>(&self, f: F) {
        self.repr().iter_nfa_state_ids(f)
    }
    pub(crate) fn memory_usage(&self) -> usize {
        self.0.len()
    }
    fn repr(&self) -> Repr<'_> {
        Repr(&self.0)
    }
}
#[derive(Clone, Debug)]
pub(crate) struct StateBuilderEmpty(Vec<u8>);
impl StateBuilderEmpty {
    pub(crate) fn new() -> StateBuilderEmpty {
        StateBuilderEmpty(std::vec![])
    }
    pub(crate) fn into_matches(mut self) -> StateBuilderMatches {
        self.0.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0]);
        StateBuilderMatches(self.0)
    }
    fn clear(&mut self) {
        self.0.clear();
    }
    pub(crate) fn capacity(&self) -> usize {
        self.0.capacity()
    }
}
#[derive(Clone)]
pub(crate) struct StateBuilderMatches(Vec<u8>);
impl core::fmt::Debug for StateBuilderMatches {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_tuple("StateBuilderMatches")
            .field(&self.repr())
            .finish()
    }
}
impl StateBuilderMatches {
    pub(crate) fn into_nfa(mut self) -> StateBuilderNFA {
        self.repr_vec().close_match_pattern_ids();
        StateBuilderNFA {
            repr: self.0,
            prev_nfa_state_id: StateID::ZERO,
        }
    }
    pub(crate) fn set_is_from_word(&mut self) {
        self.repr_vec().set_is_from_word()
    }
    pub(crate) fn set_is_half_crlf(&mut self) {
        self.repr_vec().set_is_half_crlf()
    }
    pub(crate) fn look_have(&self) -> LookSet {
        LookSet::read_repr(&self.0[1..])
    }
    pub(crate) fn set_look_have(&mut self, set: impl FnMut(LookSet) -> LookSet) {
        self.repr_vec().set_look_have(set)
    }
    pub(crate) fn add_match_pattern_id(&mut self, pid: PatternID) {
        self.repr_vec().add_match_pattern_id(pid)
    }
    fn repr(&self) -> Repr<'_> {
        Repr(&self.0)
    }
    fn repr_vec(&mut self) -> ReprVec<'_> {
        ReprVec(&mut self.0)
    }
}
#[derive(Clone)]
pub(crate) struct StateBuilderNFA {
    repr: Vec<u8>,
    prev_nfa_state_id: StateID,
}
impl core::fmt::Debug for StateBuilderNFA {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_tuple("StateBuilderNFA")
            .field(&self.repr())
            .finish()
    }
}
impl StateBuilderNFA {
    pub(crate) fn to_state(&self) -> State {
        State(Arc::from(&*self.repr))
    }
    pub(crate) fn clear(self) -> StateBuilderEmpty {
        let mut builder = StateBuilderEmpty(self.repr);
        builder.clear();
        builder
    }
    pub(crate) fn look_need(&self) -> LookSet {
        self.repr().look_need()
    }
    pub(crate) fn set_look_have(&mut self, set: impl FnMut(LookSet) -> LookSet) {
        self.repr_vec().set_look_have(set)
    }
    pub(crate) fn set_look_need(&mut self, set: impl FnMut(LookSet) -> LookSet) {
        self.repr_vec().set_look_need(set)
    }
    pub(crate) fn add_nfa_state_id(&mut self, sid: StateID) {
        ReprVec(&mut self.repr).add_nfa_state_id(&mut self.prev_nfa_state_id, sid)
    }
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.repr
    }
    fn repr(&self) -> Repr<'_> {
        Repr(&self.repr)
    }
    fn repr_vec(&mut self) -> ReprVec<'_> {
        ReprVec(&mut self.repr)
    }
}
struct Repr<'a>(&'a [u8]);
impl<'a> Repr<'a> {
    fn is_match(&self) -> bool {
        self.0[0] & (1 << 0) > 0
    }
    fn has_pattern_ids(&self) -> bool {
        self.0[0] & (1 << 1) > 0
    }
    fn is_from_word(&self) -> bool {
        self.0[0] & (1 << 2) > 0
    }
    fn is_half_crlf(&self) -> bool {
        self.0[0] & (1 << 3) > 0
    }
    fn look_have(&self) -> LookSet {
        LookSet::read_repr(&self.0[1..])
    }
    fn look_need(&self) -> LookSet {
        LookSet::read_repr(&self.0[5..])
    }
    fn match_len(&self) -> usize {
        if !self.is_match() {
            0
        } else if !self.has_pattern_ids() {
            1
        } else {
            self.encoded_pattern_len()
        }
    }
    fn match_pattern(&self, index: usize) -> PatternID {
        if !self.has_pattern_ids() {
            PatternID::ZERO
        } else {
            let offset = 13 + index * PatternID::SIZE;
            wire::read_pattern_id_unchecked(&self.0[offset..]).0
        }
    }
    fn match_pattern_ids(&self) -> Option<Vec<PatternID>> {
        if !self.is_match() {
            return None;
        }
        let mut pids = std::vec![];
        self.iter_match_pattern_ids(|pid| pids.push(pid));
        Some(pids)
    }
    fn iter_match_pattern_ids<F: FnMut(PatternID)>(&self, mut f: F) {
        if !self.is_match() {
            return;
        }
        if !self.has_pattern_ids() {
            f(PatternID::ZERO);
            return;
        }
        let mut pids = &self.0[13..self.pattern_offset_end()];
        while !pids.is_empty() {
            let pid = wire::read_u32(pids);
            pids = &pids[PatternID::SIZE..];
            f(PatternID::new_unchecked(usize::try_from(pid).unwrap()));
        }
    }
    fn iter_nfa_state_ids<F: FnMut(StateID)>(&self, mut f: F) {
        let mut sids = &self.0[self.pattern_offset_end()..];
        let mut prev = 0i32;
        while !sids.is_empty() {
            let (delta, nr) = read_vari32(sids);
            sids = &sids[nr..];
            let sid = prev + delta;
            prev = sid;
            f(StateID::new_unchecked(sid.as_usize()))
        }
    }
    fn pattern_offset_end(&self) -> usize {
        let encoded = self.encoded_pattern_len();
        if encoded == 0 {
            return 9;
        }
        encoded.checked_mul(4).unwrap().checked_add(13).unwrap()
    }
    fn encoded_pattern_len(&self) -> usize {
        if !self.has_pattern_ids() {
            return 0;
        }
        usize::try_from(wire::read_u32(&self.0[9..13])).unwrap()
    }
}
impl<'a> core::fmt::Debug for Repr<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut nfa_ids = std::vec![];
        self.iter_nfa_state_ids(|sid| nfa_ids.push(sid));
        f.debug_struct("Repr")
            .field("is_match", &self.is_match())
            .field("is_from_word", &self.is_from_word())
            .field("is_half_crlf", &self.is_half_crlf())
            .field("look_have", &self.look_have())
            .field("look_need", &self.look_need())
            .field("match_pattern_ids", &self.match_pattern_ids())
            .field("nfa_state_ids", &nfa_ids)
            .finish()
    }
}
struct ReprVec<'a>(&'a mut Vec<u8>);
impl<'a> ReprVec<'a> {
    fn set_is_match(&mut self) {
        self.0[0] |= 1 << 0;
    }
    fn set_has_pattern_ids(&mut self) {
        self.0[0] |= 1 << 1;
    }
    fn set_is_from_word(&mut self) {
        self.0[0] |= 1 << 2;
    }
    fn set_is_half_crlf(&mut self) {
        self.0[0] |= 1 << 3;
    }
    fn look_have(&self) -> LookSet {
        self.repr().look_have()
    }
    fn look_need(&self) -> LookSet {
        self.repr().look_need()
    }
    fn set_look_have(&mut self, mut set: impl FnMut(LookSet) -> LookSet) {
        set(self.look_have()).write_repr(&mut self.0[1..]);
    }
    fn set_look_need(&mut self, mut set: impl FnMut(LookSet) -> LookSet) {
        set(self.look_need()).write_repr(&mut self.0[5..]);
    }
    fn add_match_pattern_id(&mut self, pid: PatternID) {
        if !self.repr().has_pattern_ids() {
            if pid == PatternID::ZERO {
                self.set_is_match();
                return;
            }
            self.0.extend(core::iter::repeat(0).take(PatternID::SIZE));
            self.set_has_pattern_ids();
            if self.repr().is_match() {
                write_u32(self.0, 0)
            } else {
                self.set_is_match();
            }
        }
        write_u32(self.0, pid.as_u32());
    }
    fn close_match_pattern_ids(&mut self) {
        if !self.repr().has_pattern_ids() {
            return;
        }
        let patsize = PatternID::SIZE;
        let pattern_bytes = self.0.len() - 13;
        assert_eq!(pattern_bytes % patsize, 0);
        let count32 = u32::try_from(pattern_bytes / patsize).unwrap();
        wire::NE::write_u32(count32, &mut self.0[9..13]);
    }
    fn add_nfa_state_id(&mut self, prev: &mut StateID, sid: StateID) {
        let delta = sid.as_i32() - prev.as_i32();
        write_vari32(self.0, delta);
        *prev = sid;
    }
    fn repr(&self) -> Repr<'_> {
        Repr(self.0.as_slice())
    }
}
fn write_vari32(data: &mut Vec<u8>, n: i32) {
    let mut un = n.to_bits() << 1;
    if n < 0 {
        un = !un;
    }
    write_varu32(data, un)
}
fn read_vari32(data: &[u8]) -> (i32, usize) {
    let (un, i) = read_varu32(data);
    let mut n = i32::from_bits(un >> 1);
    if un & 1 != 0 {
        n = !n;
    }
    (n, i)
}
fn write_varu32(data: &mut Vec<u8>, mut n: u32) {
    while n >= 0b1000_0000 {
        data.push(n.low_u8() | 0b1000_0000);
        n >>= 7;
    }
    data.push(n.low_u8());
}
fn read_varu32(data: &[u8]) -> (u32, usize) {
    let mut n: u32 = 0;
    let mut shift: u32 = 0;
    for (i, &b) in data.iter().enumerate() {
        if b < 0b1000_0000 {
            return (n | (u32::from(b) << shift), i + 1);
        }
        n |= (u32::from(b) & 0b0111_1111) << shift;
        shift += 7;
    }
    (0, 0)
}
fn write_u32(dst: &mut Vec<u8>, n: u32) {
    use crate::regex_engine::automata::util::wire::NE;
    let start = dst.len();
    dst.extend(core::iter::repeat(0).take(mem::size_of::<u32>()));
    NE::write_u32(n, &mut dst[start..]);
}
