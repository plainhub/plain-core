use crate::regex_engine::automata::dfa::dense::BuildError;
use crate::regex_engine::automata::{
    Anchored, Match, MatchError,
    dfa::{automaton::Automaton, dense},
    util::{iter, search::Input},
};
use crate::regex_engine::automata::{dfa::StartKind, util::search::MatchKind};
use std::vec::Vec;
macro_rules ! define_regex_type { ($ (# [$ doc : meta]) *) => { $ (# [$ doc]) * pub struct Regex < A = dense :: OwnedDFA > { forward : A , reverse : A , } } ; }
define_regex_type ! (# [derive (Clone , Debug)]);
impl Regex {
    pub fn new(pattern: &str) -> Result<Regex, BuildError> {
        Builder::new().build(pattern)
    }
    pub fn new_many<P: AsRef<str>>(patterns: &[P]) -> Result<Regex, BuildError> {
        Builder::new().build_many(patterns)
    }
}

impl Regex<dense::DFA<&'static [u32]>> {
    pub fn builder() -> Builder {
        Builder::new()
    }
}
impl<A: Automaton> Regex<A> {
    #[inline]
    pub fn is_match<'h, I: Into<Input<'h>>>(&self, input: I) -> bool {
        let input = input.into().earliest(true);
        self.forward()
            .try_search_fwd(&input)
            .map(|x| x.is_some())
            .unwrap()
    }
    #[inline]
    pub fn find<'h, I: Into<Input<'h>>>(&self, input: I) -> Option<Match> {
        self.try_search(&input.into()).unwrap()
    }
    #[inline]
    pub fn find_iter<'r, 'h, I: Into<Input<'h>>>(&'r self, input: I) -> FindMatches<'r, 'h, A> {
        let it = iter::Searcher::new(input.into());
        FindMatches { re: self, it }
    }
}
impl<A: Automaton> Regex<A> {
    #[inline]
    pub fn try_search(&self, input: &Input<'_>) -> Result<Option<Match>, MatchError> {
        let (fwd, rev) = (self.forward(), self.reverse());
        let end = match fwd.try_search_fwd(input)? {
            None => return Ok(None),
            Some(end) => end,
        };
        if input.start() == end.offset() {
            return Ok(Some(Match::new(end.pattern(), end.offset()..end.offset())));
        }
        if self.is_anchored(input) {
            return Ok(Some(Match::new(end.pattern(), input.start()..end.offset())));
        }
        let revsearch = input
            .clone()
            .span(input.start()..end.offset())
            .anchored(Anchored::Yes)
            .earliest(false);
        let start = rev
            .try_search_rev(&revsearch)?
            .expect("reverse search must match if forward search does");
        assert_eq!(
            start.pattern(),
            end.pattern(),
            "forward and reverse search must match same pattern",
        );
        assert!(start.offset() <= end.offset());
        Ok(Some(Match::new(
            end.pattern(),
            start.offset()..end.offset(),
        )))
    }
    fn is_anchored(&self, input: &Input<'_>) -> bool {
        match input.get_anchored() {
            Anchored::No => self.forward().is_always_start_anchored(),
            Anchored::Yes | Anchored::Pattern(_) => true,
        }
    }
}
impl<A: Automaton> Regex<A> {
    pub fn forward(&self) -> &A {
        &self.forward
    }
    pub fn reverse(&self) -> &A {
        &self.reverse
    }
    pub fn pattern_len(&self) -> usize {
        assert_eq!(self.forward().pattern_len(), self.reverse().pattern_len());
        self.forward().pattern_len()
    }
}
#[derive(Debug)]
pub struct FindMatches<'r, 'h, A> {
    re: &'r Regex<A>,
    it: iter::Searcher<'h>,
}
impl<'r, 'h, A: Automaton> Iterator for FindMatches<'r, 'h, A> {
    type Item = Match;
    #[inline]
    fn next(&mut self) -> Option<Match> {
        let FindMatches { re, ref mut it } = *self;
        it.advance(|input| re.try_search(input))
    }
}
#[derive(Clone, Debug)]
pub struct Builder {
    dfa: dense::Builder,
}
impl Builder {
    pub fn new() -> Builder {
        Builder {
            dfa: dense::Builder::new(),
        }
    }
    pub fn build(&self, pattern: &str) -> Result<Regex, BuildError> {
        self.build_many(&[pattern])
    }

    pub fn build_many<P: AsRef<str>>(&self, patterns: &[P]) -> Result<Regex, BuildError> {
        let forward = self.dfa.build_many(patterns)?;
        let reverse = self
            .dfa
            .clone()
            .configure(
                dense::Config::new()
                    .prefilter(None)
                    .specialize_start_states(false)
                    .start_kind(StartKind::Anchored)
                    .match_kind(MatchKind::All),
            )
            .thompson(crate::regex_engine::automata::nfa::thompson::Config::new().reverse(true))
            .build_many(patterns)?;
        Ok(self.build_from_dfas(forward, reverse))
    }

    pub fn build_from_dfas<A: Automaton>(&self, forward: A, reverse: A) -> Regex<A> {
        Regex { forward, reverse }
    }
    pub fn syntax(
        &mut self,
        config: crate::regex_engine::automata::util::syntax::Config,
    ) -> &mut Builder {
        self.dfa.syntax(config);
        self
    }
    pub fn thompson(
        &mut self,
        config: crate::regex_engine::automata::nfa::thompson::Config,
    ) -> &mut Builder {
        self.dfa.thompson(config);
        self
    }
    pub fn dense(&mut self, config: dense::Config) -> &mut Builder {
        self.dfa.configure(config);
        self
    }
}
impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}
