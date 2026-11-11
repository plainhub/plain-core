use crate::regex_engine::aho::{
    automaton::{self, Automaton, OverlappingState},
    dfa,
    nfa::{contiguous, noncontiguous},
    util::{
        error::{BuildError, MatchError},
        prefilter::Prefilter,
        primitives::{PatternID, StateID},
        search::{Anchored, Input, Match, MatchKind, StartKind},
    },
};
use core::{
    fmt::Debug,
    panic::{RefUnwindSafe, UnwindSafe},
};
use std::{string::String, sync::Arc, vec::Vec};
#[derive(Clone)]
pub struct AhoCorasick {
    aut: Arc<dyn AcAutomaton>,
    kind: AhoCorasickKind,
    start_kind: StartKind,
}
impl AhoCorasick {
    pub fn new<I, P>(patterns: I) -> Result<AhoCorasick, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        AhoCorasickBuilder::new().build(patterns)
    }
    pub fn builder() -> AhoCorasickBuilder {
        AhoCorasickBuilder::new()
    }
}
impl AhoCorasick {
    pub fn is_match<'h, I: Into<Input<'h>>>(&self, input: I) -> bool {
        self.aut
            .try_find(&input.into().earliest(true))
            .expect("AhoCorasick::try_find is not expected to fail")
            .is_some()
    }
    pub fn find<'h, I: Into<Input<'h>>>(&self, input: I) -> Option<Match> {
        self.try_find(input)
            .expect("AhoCorasick::try_find is not expected to fail")
    }

    pub fn find_iter<'a, 'h, I: Into<Input<'h>>>(&'a self, input: I) -> FindIter<'a, 'h> {
        self.try_find_iter(input)
            .expect("AhoCorasick::try_find_iter is not expected to fail")
    }

    pub fn replace_all<B>(&self, haystack: &str, replace_with: &[B]) -> String
    where
        B: AsRef<str>,
    {
        self.try_replace_all(haystack, replace_with)
            .expect("AhoCorasick::try_replace_all is not expected to fail")
    }
}
impl AhoCorasick {
    pub fn try_find<'h, I: Into<Input<'h>>>(&self, input: I) -> Result<Option<Match>, MatchError> {
        let input = input.into();
        enforce_anchored_consistency(self.start_kind, input.get_anchored())?;
        self.aut.try_find(&input)
    }
    pub fn try_find_overlapping<'h, I: Into<Input<'h>>>(
        &self,
        input: I,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        let input = input.into();
        enforce_anchored_consistency(self.start_kind, input.get_anchored())?;
        self.aut.try_find_overlapping(&input, state)
    }
    pub fn try_find_iter<'a, 'h, I: Into<Input<'h>>>(
        &'a self,
        input: I,
    ) -> Result<FindIter<'a, 'h>, MatchError> {
        let input = input.into();
        enforce_anchored_consistency(self.start_kind, input.get_anchored())?;
        Ok(FindIter(self.aut.try_find_iter(input)?))
    }
    pub fn try_find_overlapping_iter<'a, 'h, I: Into<Input<'h>>>(
        &'a self,
        input: I,
    ) -> Result<FindOverlappingIter<'a, 'h>, MatchError> {
        let input = input.into();
        enforce_anchored_consistency(self.start_kind, input.get_anchored())?;
        Ok(FindOverlappingIter(
            self.aut.try_find_overlapping_iter(input)?,
        ))
    }
    pub fn try_replace_all<B>(
        &self,
        haystack: &str,
        replace_with: &[B],
    ) -> Result<String, MatchError>
    where
        B: AsRef<str>,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)?;
        self.aut.try_replace_all(haystack, replace_with)
    }
    pub fn try_replace_all_bytes<B>(
        &self,
        haystack: &[u8],
        replace_with: &[B],
    ) -> Result<Vec<u8>, MatchError>
    where
        B: AsRef<[u8]>,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)?;
        self.aut.try_replace_all_bytes(haystack, replace_with)
    }
    pub fn try_replace_all_with<F>(
        &self,
        haystack: &str,
        dst: &mut String,
        replace_with: F,
    ) -> Result<(), MatchError>
    where
        F: FnMut(&Match, &str, &mut String) -> bool,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)?;
        self.aut.try_replace_all_with(haystack, dst, replace_with)
    }
    pub fn try_replace_all_with_bytes<F>(
        &self,
        haystack: &[u8],
        dst: &mut Vec<u8>,
        replace_with: F,
    ) -> Result<(), MatchError>
    where
        F: FnMut(&Match, &[u8], &mut Vec<u8>) -> bool,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)?;
        self.aut
            .try_replace_all_with_bytes(haystack, dst, replace_with)
    }
    pub fn try_stream_find_iter<'a, R: std::io::Read>(
        &'a self,
        rdr: R,
    ) -> Result<StreamFindIter<'a, R>, MatchError> {
        enforce_anchored_consistency(self.start_kind, Anchored::No)?;
        self.aut.try_stream_find_iter(rdr).map(StreamFindIter)
    }
    pub fn try_stream_replace_all<R, W, B>(
        &self,
        rdr: R,
        wtr: W,
        replace_with: &[B],
    ) -> Result<(), std::io::Error>
    where
        R: std::io::Read,
        W: std::io::Write,
        B: AsRef<[u8]>,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        self.aut.try_stream_replace_all(rdr, wtr, replace_with)
    }
    pub fn try_stream_replace_all_with<R, W, F>(
        &self,
        rdr: R,
        wtr: W,
        replace_with: F,
    ) -> Result<(), std::io::Error>
    where
        R: std::io::Read,
        W: std::io::Write,
        F: FnMut(&Match, &[u8], &mut W) -> Result<(), std::io::Error>,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        self.aut.try_stream_replace_all_with(rdr, wtr, replace_with)
    }
}
impl AhoCorasick {
    pub fn kind(&self) -> AhoCorasickKind {
        self.kind
    }
    pub fn start_kind(&self) -> StartKind {
        self.start_kind
    }
    pub fn match_kind(&self) -> MatchKind {
        self.aut.match_kind()
    }
    pub fn min_pattern_len(&self) -> usize {
        self.aut.min_pattern_len()
    }
    pub fn max_pattern_len(&self) -> usize {
        self.aut.max_pattern_len()
    }
    pub fn patterns_len(&self) -> usize {
        self.aut.patterns_len()
    }
    pub fn memory_usage(&self) -> usize {
        self.aut.memory_usage()
    }
}
impl core::fmt::Debug for AhoCorasick {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_tuple("AhoCorasick").field(&self.aut).finish()
    }
}
#[derive(Debug)]
pub struct FindIter<'a, 'h>(automaton::FindIter<'a, 'h, Arc<dyn AcAutomaton>>);
impl<'a, 'h> Iterator for FindIter<'a, 'h> {
    type Item = Match;
    #[inline]
    fn next(&mut self) -> Option<Match> {
        self.0.next()
    }
}
#[derive(Debug)]
pub struct FindOverlappingIter<'a, 'h>(
    automaton::FindOverlappingIter<'a, 'h, Arc<dyn AcAutomaton>>,
);
impl<'a, 'h> Iterator for FindOverlappingIter<'a, 'h> {
    type Item = Match;
    #[inline]
    fn next(&mut self) -> Option<Match> {
        self.0.next()
    }
}
#[derive(Debug)]
pub struct StreamFindIter<'a, R>(automaton::StreamFindIter<'a, Arc<dyn AcAutomaton>, R>);
impl<'a, R: std::io::Read> Iterator for StreamFindIter<'a, R> {
    type Item = Result<Match, std::io::Error>;
    fn next(&mut self) -> Option<Result<Match, std::io::Error>> {
        self.0.next()
    }
}
#[derive(Clone, Debug, Default)]
pub struct AhoCorasickBuilder {
    nfa_noncontiguous: noncontiguous::Builder,
    nfa_contiguous: contiguous::Builder,
    dfa: dfa::Builder,
    kind: Option<AhoCorasickKind>,
    start_kind: StartKind,
}
impl AhoCorasickBuilder {
    pub fn new() -> AhoCorasickBuilder {
        AhoCorasickBuilder::default()
    }
    pub fn build<I, P>(&self, patterns: I) -> Result<AhoCorasick, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        let nfa = self.nfa_noncontiguous.build(patterns)?;
        let (aut, kind): (Arc<dyn AcAutomaton>, AhoCorasickKind) = match self.kind {
            None => {
                debug!(
                    "asked for automatic Aho-Corasick implementation, \
                     criteria: <patterns: {:?}, max pattern len: {:?}, \
                     start kind: {:?}>",
                    nfa.patterns_len(),
                    nfa.max_pattern_len(),
                    self.start_kind,
                );
                self.build_auto(nfa)
            }
            Some(AhoCorasickKind::NoncontiguousNFA) => {
                debug!("forcefully chose noncontiguous NFA");
                (Arc::new(nfa), AhoCorasickKind::NoncontiguousNFA)
            }
            Some(AhoCorasickKind::ContiguousNFA) => {
                debug!("forcefully chose contiguous NFA");
                let cnfa = self.nfa_contiguous.build_from_noncontiguous(&nfa)?;
                (Arc::new(cnfa), AhoCorasickKind::ContiguousNFA)
            }
            Some(AhoCorasickKind::DFA) => {
                debug!("forcefully chose DFA");
                let dfa = self.dfa.build_from_noncontiguous(&nfa)?;
                (Arc::new(dfa), AhoCorasickKind::DFA)
            }
        };
        Ok(AhoCorasick {
            aut,
            kind,
            start_kind: self.start_kind,
        })
    }
    fn build_auto(&self, nfa: noncontiguous::NFA) -> (Arc<dyn AcAutomaton>, AhoCorasickKind) {
        let try_dfa = !matches!(self.start_kind, StartKind::Both) && nfa.patterns_len() <= 100;
        if try_dfa {
            match self.dfa.build_from_noncontiguous(&nfa) {
                Ok(dfa) => {
                    debug!("chose a DFA");
                    return (Arc::new(dfa), AhoCorasickKind::DFA);
                }
                Err(_err) => {
                    debug!("failed to build DFA, trying something else: {}", _err);
                }
            }
        }
        match self.nfa_contiguous.build_from_noncontiguous(&nfa) {
            Ok(nfa) => {
                debug!("chose contiguous NFA");
                return (Arc::new(nfa), AhoCorasickKind::ContiguousNFA);
            }
            Err(_err) => {
                debug!(
                    "failed to build contiguous NFA, \
                     trying something else: {}",
                    _err
                );
            }
        }
        debug!("chose non-contiguous NFA");
        (Arc::new(nfa), AhoCorasickKind::NoncontiguousNFA)
    }
    pub fn match_kind(&mut self, kind: MatchKind) -> &mut AhoCorasickBuilder {
        self.nfa_noncontiguous.match_kind(kind);
        self.nfa_contiguous.match_kind(kind);
        self.dfa.match_kind(kind);
        self
    }
    pub fn start_kind(&mut self, kind: StartKind) -> &mut AhoCorasickBuilder {
        self.dfa.start_kind(kind);
        self.start_kind = kind;
        self
    }
    pub fn ascii_case_insensitive(&mut self, yes: bool) -> &mut AhoCorasickBuilder {
        self.nfa_noncontiguous.ascii_case_insensitive(yes);
        self.nfa_contiguous.ascii_case_insensitive(yes);
        self.dfa.ascii_case_insensitive(yes);
        self
    }
    pub fn kind(&mut self, kind: Option<AhoCorasickKind>) -> &mut AhoCorasickBuilder {
        self.kind = kind;
        self
    }
    pub fn prefilter(&mut self, yes: bool) -> &mut AhoCorasickBuilder {
        self.nfa_noncontiguous.prefilter(yes);
        self.nfa_contiguous.prefilter(yes);
        self.dfa.prefilter(yes);
        self
    }
    pub fn dense_depth(&mut self, depth: usize) -> &mut AhoCorasickBuilder {
        self.nfa_noncontiguous.dense_depth(depth);
        self.nfa_contiguous.dense_depth(depth);
        self
    }
    pub fn byte_classes(&mut self, yes: bool) -> &mut AhoCorasickBuilder {
        self.nfa_contiguous.byte_classes(yes);
        self.dfa.byte_classes(yes);
        self
    }
}
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AhoCorasickKind {
    NoncontiguousNFA,
    ContiguousNFA,
    DFA,
}
trait AcAutomaton: Automaton + Debug + Send + Sync + UnwindSafe + RefUnwindSafe + 'static {}
impl<A> AcAutomaton for A where
    A: Automaton + Debug + Send + Sync + UnwindSafe + RefUnwindSafe + 'static
{
}
impl crate::regex_engine::aho::automaton::private::Sealed for Arc<dyn AcAutomaton> {}
unsafe impl Automaton for Arc<dyn AcAutomaton> {
    #[inline(always)]
    fn start_state(&self, anchored: Anchored) -> Result<StateID, MatchError> {
        (**self).start_state(anchored)
    }
    #[inline(always)]
    fn next_state(&self, anchored: Anchored, sid: StateID, byte: u8) -> StateID {
        (**self).next_state(anchored, sid, byte)
    }
    #[inline(always)]
    fn is_special(&self, sid: StateID) -> bool {
        (**self).is_special(sid)
    }
    #[inline(always)]
    fn is_dead(&self, sid: StateID) -> bool {
        (**self).is_dead(sid)
    }
    #[inline(always)]
    fn is_match(&self, sid: StateID) -> bool {
        (**self).is_match(sid)
    }
    #[inline(always)]
    fn is_start(&self, sid: StateID) -> bool {
        (**self).is_start(sid)
    }
    #[inline(always)]
    fn match_kind(&self) -> MatchKind {
        (**self).match_kind()
    }
    #[inline(always)]
    fn match_len(&self, sid: StateID) -> usize {
        (**self).match_len(sid)
    }
    #[inline(always)]
    fn match_pattern(&self, sid: StateID, index: usize) -> PatternID {
        (**self).match_pattern(sid, index)
    }
    #[inline(always)]
    fn patterns_len(&self) -> usize {
        (**self).patterns_len()
    }
    #[inline(always)]
    fn pattern_len(&self, pid: PatternID) -> usize {
        (**self).pattern_len(pid)
    }
    #[inline(always)]
    fn min_pattern_len(&self) -> usize {
        (**self).min_pattern_len()
    }
    #[inline(always)]
    fn max_pattern_len(&self) -> usize {
        (**self).max_pattern_len()
    }
    #[inline(always)]
    fn memory_usage(&self) -> usize {
        (**self).memory_usage()
    }
    #[inline(always)]
    fn prefilter(&self) -> Option<&Prefilter> {
        (**self).prefilter()
    }
    #[inline(always)]
    fn try_find(&self, input: &Input<'_>) -> Result<Option<Match>, MatchError> {
        (**self).try_find(input)
    }
    #[inline(always)]
    fn try_find_overlapping(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        (**self).try_find_overlapping(input, state)
    }
}
fn enforce_anchored_consistency(have: StartKind, want: Anchored) -> Result<(), MatchError> {
    match have {
        StartKind::Both => Ok(()),
        StartKind::Unanchored if !want.is_anchored() => Ok(()),
        StartKind::Unanchored => Err(MatchError::invalid_input_anchored()),
        StartKind::Anchored if want.is_anchored() => Ok(()),
        StartKind::Anchored => Err(MatchError::invalid_input_unanchored()),
    }
}
