use crate::regex_engine::automata::{
    hybrid::{
        dfa::{self, DFA},
        error::BuildError,
    },
    nfa::thompson,
    util::{
        iter,
        search::{Anchored, Input, Match, MatchError, MatchKind},
    },
};
#[derive(Debug)]
pub struct Regex {
    forward: DFA,
    reverse: DFA,
}
impl Regex {
    pub fn new(pattern: &str) -> Result<Regex, BuildError> {
        Regex::builder().build(pattern)
    }
    pub fn new_many<P: AsRef<str>>(patterns: &[P]) -> Result<Regex, BuildError> {
        Regex::builder().build_many(patterns)
    }
    pub fn builder() -> Builder {
        Builder::new()
    }
    pub fn create_cache(&self) -> Cache {
        Cache::new(self)
    }
    pub fn reset_cache(&self, cache: &mut Cache) {
        self.forward().reset_cache(&mut cache.forward);
        self.reverse().reset_cache(&mut cache.reverse);
    }
}
impl Regex {
    #[inline]
    pub fn is_match<'h, I: Into<Input<'h>>>(&self, cache: &mut Cache, input: I) -> bool {
        self.forward()
            .try_search_fwd(&mut cache.forward, &input.into().earliest(true))
            .unwrap()
            .is_some()
    }
    #[inline]
    pub fn find<'h, I: Into<Input<'h>>>(&self, cache: &mut Cache, input: I) -> Option<Match> {
        self.try_search(cache, &input.into()).unwrap()
    }
    #[inline]
    pub fn find_iter<'r, 'c, 'h, I: Into<Input<'h>>>(
        &'r self,
        cache: &'c mut Cache,
        input: I,
    ) -> FindMatches<'r, 'c, 'h> {
        let it = iter::Searcher::new(input.into());
        FindMatches {
            re: self,
            cache,
            it,
        }
    }
}
impl Regex {
    #[inline]
    pub fn try_search(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<Option<Match>, MatchError> {
        let (fcache, rcache) = (&mut cache.forward, &mut cache.reverse);
        let end = match self.forward().try_search_fwd(fcache, input)? {
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
        let start = self
            .reverse()
            .try_search_rev(rcache, &revsearch)?
            .expect("reverse search must match if forward search does");
        debug_assert_eq!(
            start.pattern(),
            end.pattern(),
            "forward and reverse search must match same pattern",
        );
        debug_assert!(start.offset() <= end.offset());
        Ok(Some(Match::new(
            end.pattern(),
            start.offset()..end.offset(),
        )))
    }
    fn is_anchored(&self, input: &Input<'_>) -> bool {
        match input.get_anchored() {
            Anchored::No => self.forward().get_nfa().is_always_start_anchored(),
            Anchored::Yes | Anchored::Pattern(_) => true,
        }
    }
}
impl Regex {
    pub fn forward(&self) -> &DFA {
        &self.forward
    }
    pub fn reverse(&self) -> &DFA {
        &self.reverse
    }
    pub fn pattern_len(&self) -> usize {
        assert_eq!(self.forward().pattern_len(), self.reverse().pattern_len());
        self.forward().pattern_len()
    }
}
#[derive(Debug)]
pub struct FindMatches<'r, 'c, 'h> {
    re: &'r Regex,
    cache: &'c mut Cache,
    it: iter::Searcher<'h>,
}
impl<'r, 'c, 'h> Iterator for FindMatches<'r, 'c, 'h> {
    type Item = Match;
    #[inline]
    fn next(&mut self) -> Option<Match> {
        let FindMatches {
            re,
            ref mut cache,
            ref mut it,
        } = *self;
        it.advance(|input| re.try_search(cache, input))
    }
}
#[derive(Debug, Clone)]
pub struct Cache {
    forward: dfa::Cache,
    reverse: dfa::Cache,
}
impl Cache {
    pub fn new(re: &Regex) -> Cache {
        let forward = dfa::Cache::new(re.forward());
        let reverse = dfa::Cache::new(re.reverse());
        Cache { forward, reverse }
    }
    pub fn reset(&mut self, re: &Regex) {
        self.forward.reset(re.forward());
        self.reverse.reset(re.reverse());
    }
    pub fn forward(&mut self) -> &dfa::Cache {
        &self.forward
    }
    pub fn reverse(&mut self) -> &dfa::Cache {
        &self.reverse
    }

    pub fn as_parts_mut(&mut self) -> (&mut dfa::Cache, &mut dfa::Cache) {
        (&mut self.forward, &mut self.reverse)
    }
    pub fn memory_usage(&self) -> usize {
        self.forward.memory_usage() + self.reverse.memory_usage()
    }
}
#[derive(Clone, Debug)]
pub struct Builder {
    dfa: dfa::Builder,
}
impl Builder {
    pub fn new() -> Builder {
        Builder {
            dfa: DFA::builder(),
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
                DFA::config()
                    .prefilter(None)
                    .specialize_start_states(false)
                    .match_kind(MatchKind::All),
            )
            .thompson(thompson::Config::new().reverse(true))
            .build_many(patterns)?;
        Ok(self.build_from_dfas(forward, reverse))
    }
    pub fn build_from_dfas(&self, forward: DFA, reverse: DFA) -> Regex {
        Regex { forward, reverse }
    }
    pub fn syntax(
        &mut self,
        config: crate::regex_engine::automata::util::syntax::Config,
    ) -> &mut Builder {
        self.dfa.syntax(config);
        self
    }
    pub fn thompson(&mut self, config: thompson::Config) -> &mut Builder {
        self.dfa.thompson(config);
        self
    }
    pub fn dfa(&mut self, config: dfa::Config) -> &mut Builder {
        self.dfa.configure(config);
        self
    }
}
impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}
