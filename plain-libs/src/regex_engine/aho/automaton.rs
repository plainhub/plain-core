use crate::regex_engine::aho::util::{
    error::MatchError,
    primitives::PatternID,
    search::{Anchored, Input, Match, MatchKind, Span},
};
pub use crate::regex_engine::aho::util::{
    prefilter::{Candidate, Prefilter},
    primitives::{StateID, StateIDError},
};
use std::{string::String, vec::Vec};
pub(crate) mod private {
    pub trait Sealed {}
}
impl private::Sealed for crate::regex_engine::aho::nfa::noncontiguous::NFA {}
impl private::Sealed for crate::regex_engine::aho::nfa::contiguous::NFA {}
impl private::Sealed for crate::regex_engine::aho::dfa::DFA {}
impl<'a, T: private::Sealed + ?Sized> private::Sealed for &'a T {}
pub unsafe trait Automaton: private::Sealed {
    fn start_state(&self, anchored: Anchored) -> Result<StateID, MatchError>;
    fn next_state(&self, anchored: Anchored, sid: StateID, byte: u8) -> StateID;
    fn is_special(&self, sid: StateID) -> bool;
    fn is_dead(&self, sid: StateID) -> bool;
    fn is_match(&self, sid: StateID) -> bool;
    fn is_start(&self, sid: StateID) -> bool;
    fn match_kind(&self) -> MatchKind;
    fn match_len(&self, sid: StateID) -> usize;
    fn match_pattern(&self, sid: StateID, index: usize) -> PatternID;
    fn patterns_len(&self) -> usize;
    fn pattern_len(&self, pid: PatternID) -> usize;
    fn min_pattern_len(&self) -> usize;
    fn max_pattern_len(&self) -> usize;
    fn memory_usage(&self) -> usize;
    fn prefilter(&self) -> Option<&Prefilter>;
    fn try_find(&self, input: &Input<'_>) -> Result<Option<Match>, MatchError> {
        try_find_fwd(&self, input)
    }
    fn try_find_overlapping(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        try_find_overlapping_fwd(&self, input, state)
    }
    fn try_find_iter<'a, 'h>(
        &'a self,
        input: Input<'h>,
    ) -> Result<FindIter<'a, 'h, Self>, MatchError>
    where
        Self: Sized,
    {
        FindIter::new(self, input)
    }
    fn try_find_overlapping_iter<'a, 'h>(
        &'a self,
        input: Input<'h>,
    ) -> Result<FindOverlappingIter<'a, 'h, Self>, MatchError>
    where
        Self: Sized,
    {
        if !self.match_kind().is_standard() {
            return Err(MatchError::unsupported_overlapping(self.match_kind()));
        }
        if input.get_anchored().is_anchored() {
            return Err(MatchError::invalid_input_anchored());
        }
        let _ = self.start_state(input.get_anchored())?;
        let state = OverlappingState::start();
        Ok(FindOverlappingIter {
            aut: self,
            input,
            state,
        })
    }
    fn try_replace_all<B>(&self, haystack: &str, replace_with: &[B]) -> Result<String, MatchError>
    where
        Self: Sized,
        B: AsRef<str>,
    {
        assert_eq!(
            replace_with.len(),
            self.patterns_len(),
            "replace_all requires a replacement for every pattern \
             in the automaton"
        );
        let mut dst = String::with_capacity(haystack.len());
        self.try_replace_all_with(haystack, &mut dst, |mat, _, dst| {
            dst.push_str(replace_with[mat.pattern()].as_ref());
            true
        })?;
        Ok(dst)
    }
    fn try_replace_all_bytes<B>(
        &self,
        haystack: &[u8],
        replace_with: &[B],
    ) -> Result<Vec<u8>, MatchError>
    where
        Self: Sized,
        B: AsRef<[u8]>,
    {
        assert_eq!(
            replace_with.len(),
            self.patterns_len(),
            "replace_all requires a replacement for every pattern \
             in the automaton"
        );
        let mut dst = Vec::with_capacity(haystack.len());
        self.try_replace_all_with_bytes(haystack, &mut dst, |mat, _, dst| {
            dst.extend(replace_with[mat.pattern()].as_ref());
            true
        })?;
        Ok(dst)
    }
    fn try_replace_all_with<F>(
        &self,
        haystack: &str,
        dst: &mut String,
        mut replace_with: F,
    ) -> Result<(), MatchError>
    where
        Self: Sized,
        F: FnMut(&Match, &str, &mut String) -> bool,
    {
        let mut last_match = 0;
        for m in self.try_find_iter(Input::new(haystack))? {
            if !haystack.is_char_boundary(m.start()) || !haystack.is_char_boundary(m.end()) {
                continue;
            }
            dst.push_str(&haystack[last_match..m.start()]);
            last_match = m.end();
            if !replace_with(&m, &haystack[m.start()..m.end()], dst) {
                break;
            };
        }
        dst.push_str(&haystack[last_match..]);
        Ok(())
    }
    fn try_replace_all_with_bytes<F>(
        &self,
        haystack: &[u8],
        dst: &mut Vec<u8>,
        mut replace_with: F,
    ) -> Result<(), MatchError>
    where
        Self: Sized,
        F: FnMut(&Match, &[u8], &mut Vec<u8>) -> bool,
    {
        let mut last_match = 0;
        for m in self.try_find_iter(Input::new(haystack))? {
            dst.extend(&haystack[last_match..m.start()]);
            last_match = m.end();
            if !replace_with(&m, &haystack[m.start()..m.end()], dst) {
                break;
            };
        }
        dst.extend(&haystack[last_match..]);
        Ok(())
    }
    fn try_stream_find_iter<'a, R: std::io::Read>(
        &'a self,
        rdr: R,
    ) -> Result<StreamFindIter<'a, Self, R>, MatchError>
    where
        Self: Sized,
    {
        Ok(StreamFindIter {
            it: StreamChunkIter::new(self, rdr)?,
        })
    }
    fn try_stream_replace_all<R, W, B>(
        &self,
        rdr: R,
        wtr: W,
        replace_with: &[B],
    ) -> std::io::Result<()>
    where
        Self: Sized,
        R: std::io::Read,
        W: std::io::Write,
        B: AsRef<[u8]>,
    {
        assert_eq!(
            replace_with.len(),
            self.patterns_len(),
            "streaming replace_all requires a replacement for every pattern \
             in the automaton",
        );
        self.try_stream_replace_all_with(rdr, wtr, |mat, _, wtr| {
            wtr.write_all(replace_with[mat.pattern()].as_ref())
        })
    }
    fn try_stream_replace_all_with<R, W, F>(
        &self,
        rdr: R,
        mut wtr: W,
        mut replace_with: F,
    ) -> std::io::Result<()>
    where
        Self: Sized,
        R: std::io::Read,
        W: std::io::Write,
        F: FnMut(&Match, &[u8], &mut W) -> std::io::Result<()>,
    {
        let mut it = StreamChunkIter::new(self, rdr).map_err(|e| {
            let kind = std::io::ErrorKind::Other;
            std::io::Error::new(kind, e)
        })?;
        while let Some(result) = it.next() {
            let chunk = result?;
            match chunk {
                StreamChunk::NonMatch { bytes, .. } => {
                    wtr.write_all(bytes)?;
                }
                StreamChunk::Match { bytes, mat } => {
                    replace_with(&mat, bytes, &mut wtr)?;
                }
            }
        }
        Ok(())
    }
}
unsafe impl<'a, A: Automaton + ?Sized> Automaton for &'a A {
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
}
#[derive(Clone, Debug)]
pub struct OverlappingState {
    mat: Option<Match>,
    id: Option<StateID>,
    at: usize,
    next_match_index: Option<usize>,
}
impl OverlappingState {
    pub fn start() -> OverlappingState {
        OverlappingState {
            mat: None,
            id: None,
            at: 0,
            next_match_index: None,
        }
    }
    pub fn get_match(&self) -> Option<Match> {
        self.mat
    }
}
#[derive(Debug)]
pub struct FindIter<'a, 'h, A> {
    aut: &'a A,
    input: Input<'h>,
    last_match_end: Option<usize>,
}
impl<'a, 'h, A: Automaton> FindIter<'a, 'h, A> {
    fn new(aut: &'a A, input: Input<'h>) -> Result<FindIter<'a, 'h, A>, MatchError> {
        let _ = aut.start_state(input.get_anchored())?;
        Ok(FindIter {
            aut,
            input,
            last_match_end: None,
        })
    }
    fn search(&self) -> Option<Match> {
        self.aut
            .try_find(&self.input)
            .expect("already checked that no match error can occur")
    }
    #[cold]
    #[inline(never)]
    fn handle_overlapping_empty_match(&mut self, mut m: Match) -> Option<Match> {
        assert!(m.is_empty());
        if Some(m.end()) == self.last_match_end {
            self.input
                .set_start(self.input.start().checked_add(1).unwrap());
            m = self.search()?;
        }
        Some(m)
    }
}
impl<'a, 'h, A: Automaton> Iterator for FindIter<'a, 'h, A> {
    type Item = Match;
    #[inline(always)]
    fn next(&mut self) -> Option<Match> {
        let mut m = self.search()?;
        if m.is_empty() {
            m = self.handle_overlapping_empty_match(m)?;
        }
        self.input.set_start(m.end());
        self.last_match_end = Some(m.end());
        Some(m)
    }
}
#[derive(Debug)]
pub struct FindOverlappingIter<'a, 'h, A> {
    aut: &'a A,
    input: Input<'h>,
    state: OverlappingState,
}
impl<'a, 'h, A: Automaton> Iterator for FindOverlappingIter<'a, 'h, A> {
    type Item = Match;
    #[inline(always)]
    fn next(&mut self) -> Option<Match> {
        self.aut
            .try_find_overlapping(&self.input, &mut self.state)
            .expect("already checked that no match error can occur here");
        self.state.get_match()
    }
}
#[derive(Debug)]
pub struct StreamFindIter<'a, A, R> {
    it: StreamChunkIter<'a, A, R>,
}
impl<'a, A: Automaton, R: std::io::Read> Iterator for StreamFindIter<'a, A, R> {
    type Item = std::io::Result<Match>;
    fn next(&mut self) -> Option<std::io::Result<Match>> {
        loop {
            match self.it.next() {
                None => return None,
                Some(Err(err)) => return Some(Err(err)),
                Some(Ok(StreamChunk::NonMatch { .. })) => {}
                Some(Ok(StreamChunk::Match { mat, .. })) => {
                    return Some(Ok(mat));
                }
            }
        }
    }
}
#[derive(Debug)]
struct StreamChunkIter<'a, A, R> {
    aut: &'a A,
    rdr: R,
    buf: crate::regex_engine::aho::util::buffer::Buffer,
    start: StateID,
    sid: StateID,
    absolute_pos: usize,
    buffer_pos: usize,
    buffer_reported_pos: usize,
}
impl<'a, A: Automaton, R: std::io::Read> StreamChunkIter<'a, A, R> {
    fn new(aut: &'a A, rdr: R) -> Result<StreamChunkIter<'a, A, R>, MatchError> {
        if !aut.match_kind().is_standard() {
            return Err(MatchError::unsupported_stream(aut.match_kind()));
        }
        if aut.min_pattern_len() == 0 {
            return Err(MatchError::unsupported_empty());
        }
        let start = aut.start_state(Anchored::No)?;
        Ok(StreamChunkIter {
            aut,
            rdr,
            buf: crate::regex_engine::aho::util::buffer::Buffer::new(aut.max_pattern_len()),
            start,
            sid: start,
            absolute_pos: 0,
            buffer_pos: 0,
            buffer_reported_pos: 0,
        })
    }
    fn next(&mut self) -> Option<std::io::Result<StreamChunk>> {
        loop {
            if self.aut.is_match(self.sid) {
                let mat = self.get_match();
                if let Some(r) = self.get_non_match_chunk(mat) {
                    self.buffer_reported_pos += r.len();
                    let bytes = &self.buf.buffer()[r];
                    return Some(Ok(StreamChunk::NonMatch { bytes }));
                }
                self.sid = self.start;
                let r = self.get_match_chunk(mat);
                self.buffer_reported_pos += r.len();
                let bytes = &self.buf.buffer()[r];
                return Some(Ok(StreamChunk::Match { bytes, mat }));
            }
            if self.buffer_pos >= self.buf.buffer().len() {
                if let Some(r) = self.get_pre_roll_non_match_chunk() {
                    self.buffer_reported_pos += r.len();
                    let bytes = &self.buf.buffer()[r];
                    return Some(Ok(StreamChunk::NonMatch { bytes }));
                }
                if self.buf.buffer().len() >= self.buf.min_buffer_len() {
                    self.buffer_pos = self.buf.min_buffer_len();
                    self.buffer_reported_pos -= self.buf.buffer().len() - self.buf.min_buffer_len();
                    self.buf.roll();
                }
                match self.buf.fill(&mut self.rdr) {
                    Err(err) => return Some(Err(err)),
                    Ok(true) => {}
                    Ok(false) => {
                        if let Some(r) = self.get_eof_non_match_chunk() {
                            self.buffer_reported_pos += r.len();
                            let bytes = &self.buf.buffer()[r];
                            return Some(Ok(StreamChunk::NonMatch { bytes }));
                        }
                        return None;
                    }
                }
            }
            let start = self.absolute_pos;
            for &byte in self.buf.buffer()[self.buffer_pos..].iter() {
                self.sid = self.aut.next_state(Anchored::No, self.sid, byte);
                self.absolute_pos += 1;
                if self.aut.is_match(self.sid) {
                    break;
                }
            }
            self.buffer_pos += self.absolute_pos - start;
        }
    }
    fn get_match_chunk(&self, mat: Match) -> core::ops::Range<usize> {
        let start = self.buffer_pos - mat.len();
        let end = self.buffer_pos;
        start..end
    }
    fn get_non_match_chunk(&self, mat: Match) -> Option<core::ops::Range<usize>> {
        let buffer_mat_start = self.buffer_pos - mat.len();
        if buffer_mat_start > self.buffer_reported_pos {
            let start = self.buffer_reported_pos;
            let end = buffer_mat_start;
            return Some(start..end);
        }
        None
    }
    fn get_pre_roll_non_match_chunk(&self) -> Option<core::ops::Range<usize>> {
        let end = self
            .buf
            .buffer()
            .len()
            .saturating_sub(self.buf.min_buffer_len());
        if self.buffer_reported_pos < end {
            return Some(self.buffer_reported_pos..end);
        }
        None
    }
    fn get_eof_non_match_chunk(&self) -> Option<core::ops::Range<usize>> {
        if self.buffer_reported_pos < self.buf.buffer().len() {
            return Some(self.buffer_reported_pos..self.buf.buffer().len());
        }
        None
    }
    fn get_match(&self) -> Match {
        get_match(self.aut, self.sid, 0, self.absolute_pos)
    }
}
#[derive(Debug)]
enum StreamChunk<'r> {
    NonMatch { bytes: &'r [u8] },
    Match { bytes: &'r [u8], mat: Match },
}
#[inline(never)]
pub(crate) fn try_find_fwd<A: Automaton + ?Sized>(
    aut: &A,
    input: &Input<'_>,
) -> Result<Option<Match>, MatchError> {
    if input.is_done() {
        return Ok(None);
    }
    let earliest = aut.match_kind().is_standard() || input.get_earliest();
    if input.get_anchored().is_anchored() {
        try_find_fwd_imp(aut, input, None, Anchored::Yes, earliest)
    } else if let Some(pre) = aut.prefilter() {
        if earliest {
            try_find_fwd_imp(aut, input, Some(pre), Anchored::No, true)
        } else {
            try_find_fwd_imp(aut, input, Some(pre), Anchored::No, false)
        }
    } else {
        if earliest {
            try_find_fwd_imp(aut, input, None, Anchored::No, true)
        } else {
            try_find_fwd_imp(aut, input, None, Anchored::No, false)
        }
    }
}
#[inline(always)]
fn try_find_fwd_imp<A: Automaton + ?Sized>(
    aut: &A,
    input: &Input<'_>,
    pre: Option<&Prefilter>,
    anchored: Anchored,
    earliest: bool,
) -> Result<Option<Match>, MatchError> {
    let mut sid = aut.start_state(input.get_anchored())?;
    let mut at = input.start();
    let mut mat = None;
    if aut.is_match(sid) {
        mat = Some(get_match(aut, sid, 0, at));
        if earliest {
            return Ok(mat);
        }
    }
    if let Some(pre) = pre {
        match pre.find_in(input.haystack(), input.get_span()) {
            Candidate::None => return Ok(None),
            Candidate::Match(m) => return Ok(Some(m)),
            Candidate::PossibleStartOfMatch(i) => {
                at = i;
            }
        }
    }
    while at < input.end() {
        sid = aut.next_state(anchored, sid, input.haystack()[at]);
        if aut.is_special(sid) {
            if aut.is_dead(sid) {
                return Ok(mat);
            } else if aut.is_match(sid) {
                let m = get_match(aut, sid, 0, at + 1);
                if !(anchored.is_anchored() && m.start() > input.start()) {
                    mat = Some(m);
                    if earliest {
                        return Ok(mat);
                    }
                }
            } else if let Some(pre) = pre {
                debug_assert!(aut.is_start(sid));
                let span = Span::from(at..input.end());
                match pre.find_in(input.haystack(), span).into_option() {
                    None => return Ok(None),
                    Some(i) => {
                        if i > at {
                            at = i;
                            continue;
                        }
                    }
                }
            } else {
                debug_assert!(false, "unreachable");
            }
        }
        at += 1;
    }
    Ok(mat)
}
#[inline(never)]
fn try_find_overlapping_fwd<A: Automaton + ?Sized>(
    aut: &A,
    input: &Input<'_>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    state.mat = None;
    if input.is_done() {
        return Ok(());
    }
    if aut.prefilter().is_some() && !input.get_anchored().is_anchored() {
        let pre = aut.prefilter().unwrap();
        try_find_overlapping_fwd_imp(aut, input, Some(pre), state)
    } else {
        try_find_overlapping_fwd_imp(aut, input, None, state)
    }
}
#[inline(always)]
fn try_find_overlapping_fwd_imp<A: Automaton + ?Sized>(
    aut: &A,
    input: &Input<'_>,
    pre: Option<&Prefilter>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    let mut sid = match state.id {
        None => {
            let sid = aut.start_state(input.get_anchored())?;
            if aut.is_match(sid) {
                let i = state.next_match_index.unwrap_or(0);
                let len = aut.match_len(sid);
                if i < len {
                    state.next_match_index = Some(i + 1);
                    state.mat = Some(get_match(aut, sid, i, input.start()));
                    return Ok(());
                }
            }
            state.at = input.start();
            state.id = Some(sid);
            state.next_match_index = None;
            state.mat = None;
            sid
        }
        Some(sid) => {
            if let Some(i) = state.next_match_index {
                let len = aut.match_len(sid);
                if i < len {
                    state.next_match_index = Some(i + 1);
                    state.mat = Some(get_match(aut, sid, i, state.at + 1));
                    return Ok(());
                }
                state.at += 1;
                state.next_match_index = None;
                state.mat = None;
            }
            sid
        }
    };
    while state.at < input.end() {
        sid = aut.next_state(input.get_anchored(), sid, input.haystack()[state.at]);
        if aut.is_special(sid) {
            state.id = Some(sid);
            if aut.is_dead(sid) {
                return Ok(());
            } else if aut.is_match(sid) {
                state.next_match_index = Some(1);
                state.mat = Some(get_match(aut, sid, 0, state.at + 1));
                return Ok(());
            } else if let Some(pre) = pre {
                debug_assert!(aut.is_start(sid));
                let span = Span::from(state.at..input.end());
                match pre.find_in(input.haystack(), span).into_option() {
                    None => return Ok(()),
                    Some(i) => {
                        if i > state.at {
                            state.at = i;
                            continue;
                        }
                    }
                }
            } else {
            }
        }
        state.at += 1;
    }
    state.id = Some(sid);
    Ok(())
}
#[inline(always)]
fn get_match<A: Automaton + ?Sized>(aut: &A, sid: StateID, index: usize, at: usize) -> Match {
    let pid = aut.match_pattern(sid, index);
    let len = aut.pattern_len(pid);
    Match::new(pid, (at - len)..at)
}
pub(crate) fn fmt_state_indicator<A: Automaton>(
    f: &mut core::fmt::Formatter<'_>,
    aut: A,
    id: StateID,
) -> core::fmt::Result {
    if aut.is_dead(id) {
        write!(f, "D ")?;
    } else if aut.is_match(id) {
        if aut.is_start(id) {
            write!(f, "*>")?;
        } else {
            write!(f, "* ")?;
        }
    } else if aut.is_start(id) {
        write!(f, " >")?;
    } else {
        write!(f, "  ")?;
    }
    Ok(())
}
pub(crate) fn sparse_transitions<'a>(
    mut it: impl Iterator<Item = (u8, StateID)> + 'a,
) -> impl Iterator<Item = (u8, u8, StateID)> + 'a {
    let mut cur: Option<(u8, u8, StateID)> = None;
    core::iter::from_fn(move || {
        while let Some((class, next)) = it.next() {
            let (prev_start, prev_end, prev_next) = match cur {
                Some(x) => x,
                None => {
                    cur = Some((class, class, next));
                    continue;
                }
            };
            if prev_next == next {
                cur = Some((prev_start, class, prev_next));
            } else {
                cur = Some((class, class, next));
                return Some((prev_start, prev_end, prev_next));
            }
        }
        if let Some((start, end, next)) = cur.take() {
            return Some((start, end, next));
        }
        None
    })
}
