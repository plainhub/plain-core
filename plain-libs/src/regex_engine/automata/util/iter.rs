use crate::regex_engine::automata::util::captures::Captures;
use crate::regex_engine::automata::util::search::{HalfMatch, Input, Match, MatchError};
#[derive(Clone, Debug)]
pub struct Searcher<'h> {
    input: Input<'h>,
    last_match_end: Option<usize>,
}
impl<'h> Searcher<'h> {
    pub fn new(input: Input<'h>) -> Searcher<'h> {
        Searcher {
            input,
            last_match_end: None,
        }
    }
    pub fn input<'s>(&'s self) -> &'s Input<'h> {
        &self.input
    }

    #[inline]
    pub fn advance<F>(&mut self, finder: F) -> Option<Match>
    where
        F: FnMut(&Input<'_>) -> Result<Option<Match>, MatchError>,
    {
        match self.try_advance(finder) {
            Ok(m) => m,
            Err(err) => panic!(
                "unexpected regex find error: {err}\n\
                 to handle find errors, use 'try' or 'search' methods",
            ),
        }
    }
    #[inline]
    pub fn try_advance_half<F>(&mut self, mut finder: F) -> Result<Option<HalfMatch>, MatchError>
    where
        F: FnMut(&Input<'_>) -> Result<Option<HalfMatch>, MatchError>,
    {
        let mut m = match finder(&self.input)? {
            None => return Ok(None),
            Some(m) => m,
        };
        if Some(m.offset()) == self.last_match_end {
            m = match self.handle_overlapping_empty_half_match(m, finder)? {
                None => return Ok(None),
                Some(m) => m,
            };
        }
        self.input.set_start(m.offset());
        self.last_match_end = Some(m.offset());
        Ok(Some(m))
    }
    #[inline]
    pub fn try_advance<F>(&mut self, mut finder: F) -> Result<Option<Match>, MatchError>
    where
        F: FnMut(&Input<'_>) -> Result<Option<Match>, MatchError>,
    {
        let mut m = match finder(&self.input)? {
            None => return Ok(None),
            Some(m) => m,
        };
        if m.is_empty() && Some(m.end()) == self.last_match_end {
            m = match self.handle_overlapping_empty_match(m, finder)? {
                None => return Ok(None),
                Some(m) => m,
            };
        }
        self.input.set_start(m.end());
        self.last_match_end = Some(m.end());
        Ok(Some(m))
    }
    #[inline]
    pub fn into_half_matches_iter<F>(self, finder: F) -> TryHalfMatchesIter<'h, F>
    where
        F: FnMut(&Input<'_>) -> Result<Option<HalfMatch>, MatchError>,
    {
        TryHalfMatchesIter { it: self, finder }
    }

    #[cold]
    #[inline(never)]
    fn handle_overlapping_empty_half_match<F>(
        &mut self,
        _: HalfMatch,
        mut finder: F,
    ) -> Result<Option<HalfMatch>, MatchError>
    where
        F: FnMut(&Input<'_>) -> Result<Option<HalfMatch>, MatchError>,
    {
        self.input
            .set_start(self.input.start().checked_add(1).unwrap());
        finder(&self.input)
    }
    #[cold]
    #[inline(never)]
    fn handle_overlapping_empty_match<F>(
        &mut self,
        m: Match,
        mut finder: F,
    ) -> Result<Option<Match>, MatchError>
    where
        F: FnMut(&Input<'_>) -> Result<Option<Match>, MatchError>,
    {
        assert!(m.is_empty());
        self.input
            .set_start(self.input.start().checked_add(1).unwrap());
        finder(&self.input)
    }
}
pub struct TryHalfMatchesIter<'h, F> {
    it: Searcher<'h>,
    finder: F,
}
impl<'h, F> TryHalfMatchesIter<'h, F> {
    pub fn infallible(self) -> HalfMatchesIter<'h, F> {
        HalfMatchesIter(self)
    }
    pub fn input<'i>(&'i self) -> &'i Input<'h> {
        self.it.input()
    }
}
impl<'h, F> Iterator for TryHalfMatchesIter<'h, F>
where
    F: FnMut(&Input<'_>) -> Result<Option<HalfMatch>, MatchError>,
{
    type Item = Result<HalfMatch, MatchError>;
    #[inline]
    fn next(&mut self) -> Option<Result<HalfMatch, MatchError>> {
        self.it.try_advance_half(&mut self.finder).transpose()
    }
}
impl<'h, F> core::fmt::Debug for TryHalfMatchesIter<'h, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TryHalfMatchesIter")
            .field("it", &self.it)
            .field("finder", &"<closure>")
            .finish()
    }
}
#[derive(Debug)]
pub struct HalfMatchesIter<'h, F>(TryHalfMatchesIter<'h, F>);
impl<'h, F> HalfMatchesIter<'h, F> {
    pub fn input<'i>(&'i self) -> &'i Input<'h> {
        self.0.it.input()
    }
}
impl<'h, F> Iterator for HalfMatchesIter<'h, F>
where
    F: FnMut(&Input<'_>) -> Result<Option<HalfMatch>, MatchError>,
{
    type Item = HalfMatch;
    #[inline]
    fn next(&mut self) -> Option<HalfMatch> {
        match self.0.next()? {
            Ok(m) => Some(m),
            Err(err) => panic!(
                "unexpected regex half find error: {err}\n\
                 to handle find errors, use 'try' or 'search' methods",
            ),
        }
    }
}
pub struct TryMatchesIter<'h, F> {
    it: Searcher<'h>,
    finder: F,
}
impl<'h, F> TryMatchesIter<'h, F> {
    pub fn infallible(self) -> MatchesIter<'h, F> {
        MatchesIter(self)
    }
    pub fn input<'i>(&'i self) -> &'i Input<'h> {
        self.it.input()
    }
}
impl<'h, F> Iterator for TryMatchesIter<'h, F>
where
    F: FnMut(&Input<'_>) -> Result<Option<Match>, MatchError>,
{
    type Item = Result<Match, MatchError>;
    #[inline]
    fn next(&mut self) -> Option<Result<Match, MatchError>> {
        self.it.try_advance(&mut self.finder).transpose()
    }
}
impl<'h, F> core::fmt::Debug for TryMatchesIter<'h, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TryMatchesIter")
            .field("it", &self.it)
            .field("finder", &"<closure>")
            .finish()
    }
}
#[derive(Debug)]
pub struct MatchesIter<'h, F>(TryMatchesIter<'h, F>);
impl<'h, F> MatchesIter<'h, F> {
    pub fn input<'i>(&'i self) -> &'i Input<'h> {
        self.0.it.input()
    }
}
impl<'h, F> Iterator for MatchesIter<'h, F>
where
    F: FnMut(&Input<'_>) -> Result<Option<Match>, MatchError>,
{
    type Item = Match;
    #[inline]
    fn next(&mut self) -> Option<Match> {
        match self.0.next()? {
            Ok(m) => Some(m),
            Err(err) => panic!(
                "unexpected regex find error: {err}\n\
                 to handle find errors, use 'try' or 'search' methods",
            ),
        }
    }
}
pub struct TryCapturesIter<'h, F> {
    it: Searcher<'h>,
    caps: Captures,
    finder: F,
}
impl<'h, F> TryCapturesIter<'h, F> {
    pub fn infallible(self) -> CapturesIter<'h, F> {
        CapturesIter(self)
    }
}
impl<'h, F> Iterator for TryCapturesIter<'h, F>
where
    F: FnMut(&Input<'_>, &mut Captures) -> Result<(), MatchError>,
{
    type Item = Result<Captures, MatchError>;
    #[inline]
    fn next(&mut self) -> Option<Result<Captures, MatchError>> {
        let TryCapturesIter {
            ref mut it,
            ref mut caps,
            ref mut finder,
        } = *self;
        let result = it
            .try_advance(|input| {
                (finder)(input, caps)?;
                Ok(caps.get_match())
            })
            .transpose()?;
        match result {
            Ok(_) => Some(Ok(caps.clone())),
            Err(err) => Some(Err(err)),
        }
    }
}
impl<'h, F> core::fmt::Debug for TryCapturesIter<'h, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TryCapturesIter")
            .field("it", &self.it)
            .field("caps", &self.caps)
            .field("finder", &"<closure>")
            .finish()
    }
}
#[derive(Debug)]
pub struct CapturesIter<'h, F>(TryCapturesIter<'h, F>);
impl<'h, F> Iterator for CapturesIter<'h, F>
where
    F: FnMut(&Input<'_>, &mut Captures) -> Result<(), MatchError>,
{
    type Item = Captures;
    #[inline]
    fn next(&mut self) -> Option<Captures> {
        match self.0.next()? {
            Ok(m) => Some(m),
            Err(err) => panic!(
                "unexpected regex captures error: {err}\n\
                 to handle find errors, use 'try' or 'search' methods",
            ),
        }
    }
}
