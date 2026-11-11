use crate::regex_engine::aho::{
    packed::{pattern::Patterns, rabinkarp::RabinKarp, teddy},
    util::search::{Match, Span},
};
use std::sync::Arc;
const PATTERN_LIMIT: usize = 128;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MatchKind {
    LeftmostFirst,
    LeftmostLongest,
}
impl Default for MatchKind {
    fn default() -> MatchKind {
        MatchKind::LeftmostFirst
    }
}
#[derive(Clone, Debug)]
pub struct Config {
    kind: MatchKind,
    force: Option<ForceAlgorithm>,
    only_teddy_fat: Option<bool>,
    only_teddy_256bit: Option<bool>,
    heuristic_pattern_limits: bool,
}
#[derive(Clone, Debug)]
enum ForceAlgorithm {
    Teddy,
    RabinKarp,
}
impl Default for Config {
    fn default() -> Config {
        Config::new()
    }
}
impl Config {
    pub fn new() -> Config {
        Config {
            kind: MatchKind::LeftmostFirst,
            force: None,
            only_teddy_fat: None,
            only_teddy_256bit: None,
            heuristic_pattern_limits: true,
        }
    }
    pub fn builder(&self) -> Builder {
        Builder::from_config(self.clone())
    }
    pub fn match_kind(&mut self, kind: MatchKind) -> &mut Config {
        self.kind = kind;
        self
    }

    pub fn only_teddy_fat(&mut self, yes: Option<bool>) -> &mut Config {
        self.only_teddy_fat = yes;
        self
    }
    pub fn only_teddy_256bit(&mut self, yes: Option<bool>) -> &mut Config {
        self.only_teddy_256bit = yes;
        self
    }

    pub fn heuristic_pattern_limits(&mut self, yes: bool) -> &mut Config {
        self.heuristic_pattern_limits = yes;
        self
    }
}
#[derive(Clone, Debug)]
pub struct Builder {
    config: Config,
    inert: bool,
    patterns: Patterns,
}
impl Builder {
    pub fn new() -> Builder {
        Builder::from_config(Config::new())
    }
    fn from_config(config: Config) -> Builder {
        Builder {
            config,
            inert: false,
            patterns: Patterns::new(),
        }
    }
    pub fn build(&self) -> Option<Searcher> {
        if self.inert || self.patterns.is_empty() {
            return None;
        }
        let mut patterns = self.patterns.clone();
        patterns.set_match_kind(self.config.kind);
        let patterns = Arc::new(patterns);
        let rabinkarp = RabinKarp::new(&patterns);
        let (search_kind, minimum_len) = match self.config.force {
            None | Some(ForceAlgorithm::Teddy) => {
                debug!("trying to build Teddy packed matcher");
                let teddy = match self.build_teddy(Arc::clone(&patterns)) {
                    None => return None,
                    Some(teddy) => teddy,
                };
                let minimum_len = teddy.minimum_len();
                (SearchKind::Teddy(teddy), minimum_len)
            }
            Some(ForceAlgorithm::RabinKarp) => {
                debug!("using Rabin-Karp packed matcher");
                (SearchKind::RabinKarp, 0)
            }
        };
        Some(Searcher {
            patterns,
            rabinkarp,
            search_kind,
            minimum_len,
        })
    }
    fn build_teddy(&self, patterns: Arc<Patterns>) -> Option<teddy::Searcher> {
        teddy::Builder::new()
            .only_256bit(self.config.only_teddy_256bit)
            .only_fat(self.config.only_teddy_fat)
            .heuristic_pattern_limits(self.config.heuristic_pattern_limits)
            .build(patterns)
    }
    pub fn add<P: AsRef<[u8]>>(&mut self, pattern: P) -> &mut Builder {
        if self.inert {
            return self;
        } else if self.patterns.len() >= PATTERN_LIMIT {
            self.inert = true;
            self.patterns.reset();
            return self;
        }
        assert!(self.patterns.len() <= core::u16::MAX as usize);
        let pattern = pattern.as_ref();
        if pattern.is_empty() {
            self.inert = true;
            self.patterns.reset();
            return self;
        }
        self.patterns.add(pattern);
        self
    }
    pub fn extend<I, P>(&mut self, patterns: I) -> &mut Builder
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        for p in patterns {
            self.add(p);
        }
        self
    }
    pub fn len(&self) -> usize {
        self.patterns.len()
    }
    pub fn minimum_len(&self) -> usize {
        self.patterns.minimum_len()
    }
}
impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}
#[derive(Clone, Debug)]
pub struct Searcher {
    patterns: Arc<Patterns>,
    rabinkarp: RabinKarp,
    search_kind: SearchKind,
    minimum_len: usize,
}
#[derive(Clone, Debug)]
enum SearchKind {
    Teddy(teddy::Searcher),
    RabinKarp,
}
impl Searcher {
    pub fn new<I, P>(patterns: I) -> Option<Searcher>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        Builder::new().extend(patterns).build()
    }
    pub fn config() -> Config {
        Config::new()
    }
    pub fn builder() -> Builder {
        Builder::new()
    }
    #[inline]
    pub fn find<B: AsRef<[u8]>>(&self, haystack: B) -> Option<Match> {
        let haystack = haystack.as_ref();
        self.find_in(haystack, Span::from(0..haystack.len()))
    }
    #[inline]
    pub fn find_in<B: AsRef<[u8]>>(&self, haystack: B, span: Span) -> Option<Match> {
        let haystack = haystack.as_ref();
        match self.search_kind {
            SearchKind::Teddy(ref teddy) => {
                if haystack[span].len() < teddy.minimum_len() {
                    return self.find_in_slow(haystack, span);
                }
                teddy.find(&haystack[..span.end], span.start)
            }
            SearchKind::RabinKarp => self.rabinkarp.find_at(&haystack[..span.end], span.start),
        }
    }
    #[inline]
    pub fn find_iter<'a, 'b, B: ?Sized + AsRef<[u8]>>(
        &'a self,
        haystack: &'b B,
    ) -> FindIter<'a, 'b> {
        let haystack = haystack.as_ref();
        let span = Span::from(0..haystack.len());
        FindIter {
            searcher: self,
            haystack,
            span,
        }
    }
    #[inline]
    pub fn match_kind(&self) -> &MatchKind {
        self.patterns.match_kind()
    }
    #[inline]
    pub fn minimum_len(&self) -> usize {
        self.minimum_len
    }
    #[inline]
    pub fn memory_usage(&self) -> usize {
        self.patterns.memory_usage()
            + self.rabinkarp.memory_usage()
            + self.search_kind.memory_usage()
    }
    fn find_in_slow(&self, haystack: &[u8], span: Span) -> Option<Match> {
        self.rabinkarp.find_at(&haystack[..span.end], span.start)
    }
}
impl SearchKind {
    fn memory_usage(&self) -> usize {
        match *self {
            SearchKind::Teddy(ref ted) => ted.memory_usage(),
            SearchKind::RabinKarp => 0,
        }
    }
}
#[derive(Debug)]
pub struct FindIter<'s, 'h> {
    searcher: &'s Searcher,
    haystack: &'h [u8],
    span: Span,
}
impl<'s, 'h> Iterator for FindIter<'s, 'h> {
    type Item = Match;
    fn next(&mut self) -> Option<Match> {
        if self.span.start > self.span.end {
            return None;
        }
        match self.searcher.find_in(self.haystack, self.span) {
            None => None,
            Some(m) => {
                self.span.start = m.end();
                Some(m)
            }
        }
    }
}
