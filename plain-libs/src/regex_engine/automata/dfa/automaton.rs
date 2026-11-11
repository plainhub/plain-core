use crate::regex_engine::automata::util::search::PatternSet;
use crate::regex_engine::automata::{
    dfa::search,
    util::{
        empty,
        prefilter::Prefilter,
        primitives::{PatternID, StateID},
        search::{Anchored, HalfMatch, Input, MatchError},
        start,
    },
};
pub unsafe trait Automaton {
    fn next_state(&self, current: StateID, input: u8) -> StateID;
    unsafe fn next_state_unchecked(&self, current: StateID, input: u8) -> StateID;
    fn next_eoi_state(&self, current: StateID) -> StateID;
    fn start_state(&self, config: &start::Config) -> Result<StateID, StartError>;
    fn start_state_forward(&self, input: &Input<'_>) -> Result<StateID, MatchError> {
        let config = start::Config::from_input_forward(input);
        self.start_state(&config).map_err(|err| match err {
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
    fn start_state_reverse(&self, input: &Input<'_>) -> Result<StateID, MatchError> {
        let config = start::Config::from_input_reverse(input);
        self.start_state(&config).map_err(|err| match err {
            StartError::Quit { byte } => {
                let offset = input.end();
                MatchError::quit(byte, offset)
            }
            StartError::UnsupportedAnchored { mode } => MatchError::unsupported_anchored(mode),
        })
    }
    #[inline]
    fn universal_start_state(&self, _mode: Anchored) -> Option<StateID> {
        None
    }
    fn is_special_state(&self, id: StateID) -> bool;
    fn is_dead_state(&self, id: StateID) -> bool;
    fn is_quit_state(&self, id: StateID) -> bool;
    fn is_match_state(&self, id: StateID) -> bool;
    fn is_start_state(&self, id: StateID) -> bool;
    fn is_accel_state(&self, id: StateID) -> bool;
    fn pattern_len(&self) -> usize;
    fn match_len(&self, id: StateID) -> usize;
    fn match_pattern(&self, id: StateID, index: usize) -> PatternID;
    fn has_empty(&self) -> bool;
    fn is_utf8(&self) -> bool;
    fn is_always_start_anchored(&self) -> bool;
    #[inline]
    fn accelerator(&self, _id: StateID) -> &[u8] {
        &[]
    }
    #[inline]
    fn get_prefilter(&self) -> Option<&Prefilter> {
        None
    }
    #[inline]
    fn try_search_fwd(&self, input: &Input<'_>) -> Result<Option<HalfMatch>, MatchError> {
        let utf8empty = self.has_empty() && self.is_utf8();
        let hm = match search::find_fwd(&self, input)? {
            None => return Ok(None),
            Some(hm) if !utf8empty => return Ok(Some(hm)),
            Some(hm) => hm,
        };
        empty::skip_splits_fwd(input, hm, hm.offset(), |input| {
            let got = search::find_fwd(&self, input)?;
            Ok(got.map(|hm| (hm, hm.offset())))
        })
    }
    #[inline]
    fn try_search_rev(&self, input: &Input<'_>) -> Result<Option<HalfMatch>, MatchError> {
        let utf8empty = self.has_empty() && self.is_utf8();
        let hm = match search::find_rev(self, input)? {
            None => return Ok(None),
            Some(hm) if !utf8empty => return Ok(Some(hm)),
            Some(hm) => hm,
        };
        empty::skip_splits_rev(input, hm, hm.offset(), |input| {
            let got = search::find_rev(self, input)?;
            Ok(got.map(|hm| (hm, hm.offset())))
        })
    }
    #[inline]
    fn try_search_overlapping_fwd(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        let utf8empty = self.has_empty() && self.is_utf8();
        search::find_overlapping_fwd(self, input, state)?;
        match state.get_match() {
            None => Ok(()),
            Some(_) if !utf8empty => Ok(()),
            Some(_) => skip_empty_utf8_splits_overlapping(input, state, |input, state| {
                search::find_overlapping_fwd(self, input, state)
            }),
        }
    }
    #[inline]
    fn try_search_overlapping_rev(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        let utf8empty = self.has_empty() && self.is_utf8();
        search::find_overlapping_rev(self, input, state)?;
        match state.get_match() {
            None => Ok(()),
            Some(_) if !utf8empty => Ok(()),
            Some(_) => skip_empty_utf8_splits_overlapping(input, state, |input, state| {
                search::find_overlapping_rev(self, input, state)
            }),
        }
    }
    #[inline]
    fn try_which_overlapping_matches(
        &self,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) -> Result<(), MatchError> {
        let mut state = OverlappingState::start();
        while let Some(m) = {
            self.try_search_overlapping_fwd(input, &mut state)?;
            state.get_match()
        } {
            let _ = patset.insert(m.pattern());
            if patset.is_full() || input.get_earliest() {
                break;
            }
        }
        Ok(())
    }
}
unsafe impl<'a, A: Automaton + ?Sized> Automaton for &'a A {
    #[inline]
    fn next_state(&self, current: StateID, input: u8) -> StateID {
        (**self).next_state(current, input)
    }
    #[inline]
    unsafe fn next_state_unchecked(&self, current: StateID, input: u8) -> StateID {
        unsafe { (**self).next_state_unchecked(current, input) }
    }
    #[inline]
    fn next_eoi_state(&self, current: StateID) -> StateID {
        (**self).next_eoi_state(current)
    }
    #[inline]
    fn start_state(&self, config: &start::Config) -> Result<StateID, StartError> {
        (**self).start_state(config)
    }
    #[inline]
    fn start_state_forward(&self, input: &Input<'_>) -> Result<StateID, MatchError> {
        (**self).start_state_forward(input)
    }
    #[inline]
    fn start_state_reverse(&self, input: &Input<'_>) -> Result<StateID, MatchError> {
        (**self).start_state_reverse(input)
    }
    #[inline]
    fn universal_start_state(&self, mode: Anchored) -> Option<StateID> {
        (**self).universal_start_state(mode)
    }
    #[inline]
    fn is_special_state(&self, id: StateID) -> bool {
        (**self).is_special_state(id)
    }
    #[inline]
    fn is_dead_state(&self, id: StateID) -> bool {
        (**self).is_dead_state(id)
    }
    #[inline]
    fn is_quit_state(&self, id: StateID) -> bool {
        (**self).is_quit_state(id)
    }
    #[inline]
    fn is_match_state(&self, id: StateID) -> bool {
        (**self).is_match_state(id)
    }
    #[inline]
    fn is_start_state(&self, id: StateID) -> bool {
        (**self).is_start_state(id)
    }
    #[inline]
    fn is_accel_state(&self, id: StateID) -> bool {
        (**self).is_accel_state(id)
    }
    #[inline]
    fn pattern_len(&self) -> usize {
        (**self).pattern_len()
    }
    #[inline]
    fn match_len(&self, id: StateID) -> usize {
        (**self).match_len(id)
    }
    #[inline]
    fn match_pattern(&self, id: StateID, index: usize) -> PatternID {
        (**self).match_pattern(id, index)
    }
    #[inline]
    fn has_empty(&self) -> bool {
        (**self).has_empty()
    }
    #[inline]
    fn is_utf8(&self) -> bool {
        (**self).is_utf8()
    }
    #[inline]
    fn is_always_start_anchored(&self) -> bool {
        (**self).is_always_start_anchored()
    }
    #[inline]
    fn accelerator(&self, id: StateID) -> &[u8] {
        (**self).accelerator(id)
    }
    #[inline]
    fn get_prefilter(&self) -> Option<&Prefilter> {
        (**self).get_prefilter()
    }
    #[inline]
    fn try_search_fwd(&self, input: &Input<'_>) -> Result<Option<HalfMatch>, MatchError> {
        (**self).try_search_fwd(input)
    }
    #[inline]
    fn try_search_rev(&self, input: &Input<'_>) -> Result<Option<HalfMatch>, MatchError> {
        (**self).try_search_rev(input)
    }
    #[inline]
    fn try_search_overlapping_fwd(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        (**self).try_search_overlapping_fwd(input, state)
    }
    #[inline]
    fn try_search_overlapping_rev(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        (**self).try_search_overlapping_rev(input, state)
    }
    #[inline]
    fn try_which_overlapping_matches(
        &self,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) -> Result<(), MatchError> {
        (**self).try_which_overlapping_matches(input, patset)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlappingState {
    pub(crate) mat: Option<HalfMatch>,
    pub(crate) id: Option<StateID>,
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
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum StartError {
    Quit { byte: u8 },
    UnsupportedAnchored { mode: Anchored },
}
impl StartError {
    pub(crate) fn quit(byte: u8) -> StartError {
        StartError::Quit { byte }
    }
    pub(crate) fn unsupported_anchored(mode: Anchored) -> StartError {
        StartError::UnsupportedAnchored { mode }
    }
}
impl std::error::Error for StartError {}
impl core::fmt::Display for StartError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            StartError::Quit { byte } => write!(
                f,
                "error computing start state because the look-behind byte \
                 {:?} triggered a quit state",
                crate::regex_engine::automata::util::escape::DebugByte(byte),
            ),
            StartError::UnsupportedAnchored {
                mode: Anchored::Yes,
            } => {
                write!(
                    f,
                    "error computing start state because \
                     anchored searches are not supported or enabled"
                )
            }
            StartError::UnsupportedAnchored { mode: Anchored::No } => {
                write!(
                    f,
                    "error computing start state because \
                     unanchored searches are not supported or enabled"
                )
            }
            StartError::UnsupportedAnchored {
                mode: Anchored::Pattern(pid),
            } => {
                write!(
                    f,
                    "error computing start state because \
                     anchored searches for a specific pattern ({}) \
                     are not supported or enabled",
                    pid.as_usize(),
                )
            }
        }
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
pub(crate) fn fmt_state_indicator<A: Automaton>(
    f: &mut core::fmt::Formatter<'_>,
    dfa: A,
    id: StateID,
) -> core::fmt::Result {
    if dfa.is_dead_state(id) {
        write!(f, "D")?;
        if dfa.is_start_state(id) {
            write!(f, ">")?;
        } else {
            write!(f, " ")?;
        }
    } else if dfa.is_quit_state(id) {
        write!(f, "Q ")?;
    } else if dfa.is_start_state(id) {
        if dfa.is_accel_state(id) {
            write!(f, "A>")?;
        } else {
            write!(f, " >")?;
        }
    } else if dfa.is_match_state(id) {
        if dfa.is_accel_state(id) {
            write!(f, "A*")?;
        } else {
            write!(f, " *")?;
        }
    } else if dfa.is_accel_state(id) {
        write!(f, "A ")?;
    } else {
        write!(f, "  ")?;
    }
    Ok(())
}
