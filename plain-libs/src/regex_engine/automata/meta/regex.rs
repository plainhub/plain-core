use crate::regex_engine::automata::{
    meta::{
        error::BuildError,
        strategy::{self, Strategy},
        wrappers,
    },
    nfa::thompson::{self, WhichCaptures},
    util::{
        captures::{Captures, GroupInfo},
        iter,
        look::LookMatcher,
        pool::{Pool, PoolGuard},
        prefilter::Prefilter,
        primitives::{NonMaxUsize, PatternID},
        search::{HalfMatch, Input, Match, MatchKind, PatternSet, Span},
    },
};
use crate::regex_engine::syntax::{
    ast,
    hir::{self, Hir},
};
use core::{
    borrow::Borrow,
    panic::{RefUnwindSafe, UnwindSafe},
};
use std::{boxed::Box, sync::Arc, vec, vec::Vec};
type CachePool = Pool<Cache, CachePoolFn>;
type CachePoolGuard<'a> = PoolGuard<'a, Cache, CachePoolFn>;
type CachePoolFn = Box<dyn Fn() -> Cache + Send + Sync + UnwindSafe + RefUnwindSafe>;
#[derive(Debug)]
pub struct Regex {
    imp: Arc<RegexI>,
    pool: CachePool,
}
#[derive(Debug)]
struct RegexI {
    strat: Arc<dyn Strategy>,
    info: RegexInfo,
}
impl Regex {
    pub fn new(pattern: &str) -> Result<Regex, BuildError> {
        Regex::builder().build(pattern)
    }
    pub fn new_many<P: AsRef<str>>(patterns: &[P]) -> Result<Regex, BuildError> {
        Regex::builder().build_many(patterns)
    }
    pub fn config() -> Config {
        Config::new()
    }
    pub fn builder() -> Builder {
        Builder::new()
    }
}
impl Regex {
    #[inline]
    pub fn is_match<'h, I: Into<Input<'h>>>(&self, input: I) -> bool {
        let input = input.into().earliest(true);
        if self.imp.info.is_impossible(&input) {
            return false;
        }
        let mut guard = self.pool.get();
        let result = self.imp.strat.is_match(&mut guard, &input);
        PoolGuard::put(guard);
        result
    }
    #[inline]
    pub fn find<'h, I: Into<Input<'h>>>(&self, input: I) -> Option<Match> {
        self.search(&input.into())
    }
    #[inline]
    pub fn captures<'h, I: Into<Input<'h>>>(&self, input: I, caps: &mut Captures) {
        self.search_captures(&input.into(), caps)
    }
    #[inline]
    pub fn find_iter<'r, 'h, I: Into<Input<'h>>>(&'r self, input: I) -> FindMatches<'r, 'h> {
        let cache = self.pool.get();
        let it = iter::Searcher::new(input.into());
        FindMatches {
            re: self,
            cache,
            it,
        }
    }
    #[inline]
    pub fn captures_iter<'r, 'h, I: Into<Input<'h>>>(
        &'r self,
        input: I,
    ) -> CapturesMatches<'r, 'h> {
        let cache = self.pool.get();
        let caps = self.create_captures();
        let it = iter::Searcher::new(input.into());
        CapturesMatches {
            re: self,
            cache,
            caps,
            it,
        }
    }
    #[inline]
    pub fn split<'r, 'h, I: Into<Input<'h>>>(&'r self, input: I) -> Split<'r, 'h> {
        Split {
            finder: self.find_iter(input),
            last: 0,
        }
    }
    pub fn splitn<'r, 'h, I: Into<Input<'h>>>(&'r self, input: I, limit: usize) -> SplitN<'r, 'h> {
        SplitN {
            splits: self.split(input),
            limit,
        }
    }
}
impl Regex {
    #[inline]
    pub fn search(&self, input: &Input<'_>) -> Option<Match> {
        if self.imp.info.captures_disabled() || self.imp.info.is_impossible(input) {
            return None;
        }
        let mut guard = self.pool.get();
        let result = self.imp.strat.search(&mut guard, input);
        PoolGuard::put(guard);
        result
    }
    #[inline]
    pub fn search_half(&self, input: &Input<'_>) -> Option<HalfMatch> {
        if self.imp.info.captures_disabled() || self.imp.info.is_impossible(input) {
            return None;
        }
        let mut guard = self.pool.get();
        let result = self.imp.strat.search_half(&mut guard, input);
        PoolGuard::put(guard);
        result
    }
    #[inline]
    pub fn search_captures(&self, input: &Input<'_>, caps: &mut Captures) {
        caps.set_pattern(None);
        let pid = self.search_slots(input, caps.slots_mut());
        caps.set_pattern(pid);
    }
    #[inline]
    pub fn search_slots(
        &self,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        if self.imp.info.captures_disabled() || self.imp.info.is_impossible(input) {
            return None;
        }
        let mut guard = self.pool.get();
        let result = self.imp.strat.search_slots(&mut guard, input, slots);
        PoolGuard::put(guard);
        result
    }
    #[inline]
    pub fn which_overlapping_matches(&self, input: &Input<'_>, patset: &mut PatternSet) {
        if self.imp.info.is_impossible(input) {
            return;
        }
        let mut guard = self.pool.get();
        let result = self
            .imp
            .strat
            .which_overlapping_matches(&mut guard, input, patset);
        PoolGuard::put(guard);
        result
    }
}
impl Regex {
    #[inline]
    pub fn search_with(&self, cache: &mut Cache, input: &Input<'_>) -> Option<Match> {
        if self.imp.info.captures_disabled() || self.imp.info.is_impossible(input) {
            return None;
        }
        self.imp.strat.search(cache, input)
    }
    #[inline]
    pub fn search_half_with(&self, cache: &mut Cache, input: &Input<'_>) -> Option<HalfMatch> {
        if self.imp.info.captures_disabled() || self.imp.info.is_impossible(input) {
            return None;
        }
        self.imp.strat.search_half(cache, input)
    }
    #[inline]
    pub fn search_captures_with(&self, cache: &mut Cache, input: &Input<'_>, caps: &mut Captures) {
        caps.set_pattern(None);
        let pid = self.search_slots_with(cache, input, caps.slots_mut());
        caps.set_pattern(pid);
    }
    #[inline]
    pub fn search_slots_with(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        if self.imp.info.captures_disabled() || self.imp.info.is_impossible(input) {
            return None;
        }
        self.imp.strat.search_slots(cache, input, slots)
    }
}
impl Regex {
    pub fn create_captures(&self) -> Captures {
        Captures::all(self.group_info().clone())
    }
    pub fn create_cache(&self) -> Cache {
        self.imp.strat.create_cache()
    }
    pub fn pattern_len(&self) -> usize {
        self.imp.info.pattern_len()
    }

    #[inline]
    pub fn group_info(&self) -> &GroupInfo {
        self.imp.strat.group_info()
    }
    #[inline]
    pub fn get_config(&self) -> &Config {
        self.imp.info.config()
    }
    #[inline]
    pub fn is_accelerated(&self) -> bool {
        self.imp.strat.is_accelerated()
    }
    #[inline]
    pub fn memory_usage(&self) -> usize {
        self.imp.strat.memory_usage()
    }
}
impl Clone for Regex {
    fn clone(&self) -> Regex {
        let imp = Arc::clone(&self.imp);
        let pool = {
            let strat = Arc::clone(&imp.strat);
            let create: CachePoolFn = Box::new(move || strat.create_cache());
            Pool::with_capacity(self.imp.info.config().get_pool_capacity(), create)
        };
        Regex { imp, pool }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct RegexInfo(Arc<RegexInfoI>);
#[derive(Clone, Debug)]
struct RegexInfoI {
    config: Config,
    props: Vec<hir::Properties>,
    props_union: hir::Properties,
}
impl RegexInfo {
    pub(super) fn new(config: Config, hirs: &[&Hir]) -> RegexInfo {
        let mut props = vec![];
        for hir in hirs.iter() {
            props.push(hir.properties().clone());
        }
        let props_union = hir::Properties::union(&props);
        RegexInfo(Arc::new(RegexInfoI {
            config,
            props,
            props_union,
        }))
    }
    pub(crate) fn config(&self) -> &Config {
        &self.0.config
    }
    pub(crate) fn props(&self) -> &[hir::Properties] {
        &self.0.props
    }
    pub(crate) fn props_union(&self) -> &hir::Properties {
        &self.0.props_union
    }
    pub(crate) fn pattern_len(&self) -> usize {
        self.props().len()
    }
    pub(crate) fn memory_usage(&self) -> usize {
        self.props().iter().map(|p| p.memory_usage()).sum::<usize>()
            + self.props_union().memory_usage()
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn is_anchored_start(&self, input: &Input<'_>) -> bool {
        input.get_anchored().is_anchored() || self.is_always_anchored_start()
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn is_always_anchored_start(&self) -> bool {
        use crate::regex_engine::syntax::hir::Look;
        self.props_union().look_set_prefix().contains(Look::Start)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn is_always_anchored_end(&self) -> bool {
        use crate::regex_engine::syntax::hir::Look;
        self.props_union().look_set_suffix().contains(Look::End)
    }
    fn captures_disabled(&self) -> bool {
        matches!(self.config().get_which_captures(), WhichCaptures::None)
    }
    #[cfg_attr(all(), inline(always))]
    fn is_impossible(&self, input: &Input<'_>) -> bool {
        if input.start() > 0 && self.is_always_anchored_start() {
            return true;
        }
        if input.end() < input.haystack().len() && self.is_always_anchored_end() {
            return true;
        }
        let minlen = match self.props_union().minimum_len() {
            None => return false,
            Some(minlen) => minlen,
        };
        if input.get_span().len() < minlen {
            return true;
        }
        if self.is_anchored_start(input) && self.is_always_anchored_end() {
            let maxlen = match self.props_union().maximum_len() {
                None => return false,
                Some(maxlen) => maxlen,
            };
            if input.get_span().len() > maxlen {
                return true;
            }
        }
        false
    }
}
#[derive(Debug)]
pub struct FindMatches<'r, 'h> {
    re: &'r Regex,
    cache: CachePoolGuard<'r>,
    it: iter::Searcher<'h>,
}
impl<'r, 'h> FindMatches<'r, 'h> {
    #[inline]
    pub fn regex(&self) -> &'r Regex {
        self.re
    }
    #[inline]
    pub fn input<'s>(&'s self) -> &'s Input<'h> {
        self.it.input()
    }
}
impl<'r, 'h> Iterator for FindMatches<'r, 'h> {
    type Item = Match;
    #[inline]
    fn next(&mut self) -> Option<Match> {
        let FindMatches {
            re,
            ref mut cache,
            ref mut it,
        } = *self;
        it.advance(|input| Ok(re.search_with(cache, input)))
    }
    #[inline]
    fn count(self) -> usize {
        let FindMatches { re, mut cache, it } = self;
        let cache = &mut *cache;
        it.into_half_matches_iter(|input| Ok(re.search_half_with(cache, input)))
            .count()
    }
}
impl<'r, 'h> core::iter::FusedIterator for FindMatches<'r, 'h> {}
#[derive(Debug)]
pub struct CapturesMatches<'r, 'h> {
    re: &'r Regex,
    cache: CachePoolGuard<'r>,
    caps: Captures,
    it: iter::Searcher<'h>,
}
impl<'r, 'h> CapturesMatches<'r, 'h> {
    #[inline]
    pub fn regex(&self) -> &'r Regex {
        self.re
    }
    #[inline]
    pub fn input<'s>(&'s self) -> &'s Input<'h> {
        self.it.input()
    }
}
impl<'r, 'h> Iterator for CapturesMatches<'r, 'h> {
    type Item = Captures;
    #[inline]
    fn next(&mut self) -> Option<Captures> {
        let CapturesMatches {
            re,
            ref mut cache,
            ref mut caps,
            ref mut it,
        } = *self;
        let _ = it.advance(|input| {
            re.search_captures_with(cache, input, caps);
            Ok(caps.get_match())
        });
        if caps.is_match() {
            Some(caps.clone())
        } else {
            None
        }
    }
    #[inline]
    fn count(self) -> usize {
        let CapturesMatches {
            re, mut cache, it, ..
        } = self;
        let cache = &mut *cache;
        it.into_half_matches_iter(|input| Ok(re.search_half_with(cache, input)))
            .count()
    }
}
impl<'r, 'h> core::iter::FusedIterator for CapturesMatches<'r, 'h> {}
#[derive(Debug)]
pub struct Split<'r, 'h> {
    finder: FindMatches<'r, 'h>,
    last: usize,
}
impl<'r, 'h> Split<'r, 'h> {
    #[inline]
    pub fn input<'s>(&'s self) -> &'s Input<'h> {
        self.finder.input()
    }
}
impl<'r, 'h> Iterator for Split<'r, 'h> {
    type Item = Span;
    fn next(&mut self) -> Option<Span> {
        match self.finder.next() {
            None => {
                let len = self.finder.it.input().haystack().len();
                if self.last > len {
                    None
                } else {
                    let span = Span::from(self.last..len);
                    self.last = len + 1;
                    Some(span)
                }
            }
            Some(m) => {
                let span = Span::from(self.last..m.start());
                self.last = m.end();
                Some(span)
            }
        }
    }
}
impl<'r, 'h> core::iter::FusedIterator for Split<'r, 'h> {}
#[derive(Debug)]
pub struct SplitN<'r, 'h> {
    splits: Split<'r, 'h>,
    limit: usize,
}
impl<'r, 'h> SplitN<'r, 'h> {
    #[inline]
    pub fn input<'s>(&'s self) -> &'s Input<'h> {
        self.splits.input()
    }
}
impl<'r, 'h> Iterator for SplitN<'r, 'h> {
    type Item = Span;
    fn next(&mut self) -> Option<Span> {
        if self.limit == 0 {
            return None;
        }
        self.limit -= 1;
        if self.limit > 0 {
            return self.splits.next();
        }
        let len = self.splits.finder.it.input().haystack().len();
        if self.splits.last > len {
            None
        } else {
            Some(Span::from(self.splits.last..len))
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.limit))
    }
}
impl<'r, 'h> core::iter::FusedIterator for SplitN<'r, 'h> {}
#[derive(Debug, Clone)]
pub struct Cache {
    pub(crate) capmatches: Captures,
    pub(crate) pikevm: wrappers::PikeVMCache,
    pub(crate) backtrack: wrappers::BoundedBacktrackerCache,
    pub(crate) onepass: wrappers::OnePassCache,
    pub(crate) hybrid: wrappers::HybridCache,
    pub(crate) revhybrid: wrappers::ReverseHybridCache,
}
impl Cache {
    pub fn new(re: &Regex) -> Cache {
        re.create_cache()
    }
    pub fn reset(&mut self, re: &Regex) {
        re.imp.strat.reset_cache(self)
    }
    pub fn memory_usage(&self) -> usize {
        let mut bytes = 0;
        bytes += self.pikevm.memory_usage();
        bytes += self.backtrack.memory_usage();
        bytes += self.onepass.memory_usage();
        bytes += self.hybrid.memory_usage();
        bytes += self.revhybrid.memory_usage();
        bytes
    }
}
#[derive(Clone, Debug, Default)]
pub struct Config {
    match_kind: Option<MatchKind>,
    utf8_empty: Option<bool>,
    autopre: Option<bool>,
    pre: Option<Option<Prefilter>>,
    which_captures: Option<WhichCaptures>,
    nfa_size_limit: Option<Option<usize>>,
    onepass_size_limit: Option<Option<usize>>,
    hybrid_cache_capacity: Option<usize>,
    hybrid: Option<bool>,
    dfa: Option<bool>,
    dfa_size_limit: Option<Option<usize>>,
    dfa_state_limit: Option<Option<usize>>,
    onepass: Option<bool>,
    backtrack: Option<bool>,
    byte_classes: Option<bool>,
    line_terminator: Option<u8>,
    pool_capacity: Option<usize>,
}
impl Config {
    pub fn new() -> Config {
        Config::default()
    }
    pub fn match_kind(self, kind: MatchKind) -> Config {
        Config {
            match_kind: Some(kind),
            ..self
        }
    }
    pub fn utf8_empty(self, yes: bool) -> Config {
        Config {
            utf8_empty: Some(yes),
            ..self
        }
    }

    pub fn prefilter(self, pre: Option<Prefilter>) -> Config {
        Config {
            pre: Some(pre),
            ..self
        }
    }
    pub fn which_captures(mut self, which_captures: WhichCaptures) -> Config {
        self.which_captures = Some(which_captures);
        self
    }
    pub fn nfa_size_limit(self, limit: Option<usize>) -> Config {
        Config {
            nfa_size_limit: Some(limit),
            ..self
        }
    }
    pub fn onepass_size_limit(self, limit: Option<usize>) -> Config {
        Config {
            onepass_size_limit: Some(limit),
            ..self
        }
    }
    pub fn hybrid_cache_capacity(self, limit: usize) -> Config {
        Config {
            hybrid_cache_capacity: Some(limit),
            ..self
        }
    }
    pub fn dfa_size_limit(self, limit: Option<usize>) -> Config {
        Config {
            dfa_size_limit: Some(limit),
            ..self
        }
    }
    pub fn dfa_state_limit(self, limit: Option<usize>) -> Config {
        Config {
            dfa_state_limit: Some(limit),
            ..self
        }
    }
    pub fn byte_classes(self, yes: bool) -> Config {
        Config {
            byte_classes: Some(yes),
            ..self
        }
    }
    pub fn line_terminator(self, byte: u8) -> Config {
        Config {
            line_terminator: Some(byte),
            ..self
        }
    }
    pub fn pool_capacity(self, capacity: usize) -> Config {
        Config {
            pool_capacity: Some(capacity),
            ..self
        }
    }
    pub fn hybrid(self, yes: bool) -> Config {
        Config {
            hybrid: Some(yes),
            ..self
        }
    }
    pub fn dfa(self, yes: bool) -> Config {
        Config {
            dfa: Some(yes),
            ..self
        }
    }
    pub fn onepass(self, yes: bool) -> Config {
        Config {
            onepass: Some(yes),
            ..self
        }
    }
    pub fn backtrack(self, yes: bool) -> Config {
        Config {
            backtrack: Some(yes),
            ..self
        }
    }
    pub fn get_match_kind(&self) -> MatchKind {
        self.match_kind.unwrap_or(MatchKind::LeftmostFirst)
    }
    pub fn get_utf8_empty(&self) -> bool {
        self.utf8_empty.unwrap_or(true)
    }
    pub fn get_auto_prefilter(&self) -> bool {
        self.autopre.unwrap_or(true)
    }
    pub fn get_prefilter(&self) -> Option<&Prefilter> {
        self.pre.as_ref().unwrap_or(&None).as_ref()
    }
    pub fn get_which_captures(&self) -> WhichCaptures {
        self.which_captures.unwrap_or(WhichCaptures::All)
    }
    pub fn get_nfa_size_limit(&self) -> Option<usize> {
        self.nfa_size_limit.unwrap_or(Some(10 * (1 << 20)))
    }
    pub fn get_onepass_size_limit(&self) -> Option<usize> {
        self.onepass_size_limit.unwrap_or(Some(1 * (1 << 20)))
    }
    pub fn get_hybrid_cache_capacity(&self) -> usize {
        self.hybrid_cache_capacity.unwrap_or(2 * (1 << 20))
    }
    pub fn get_dfa_size_limit(&self) -> Option<usize> {
        self.dfa_size_limit.unwrap_or(Some(40 * (1 << 10)))
    }
    pub fn get_dfa_state_limit(&self) -> Option<usize> {
        self.dfa_state_limit.unwrap_or(Some(30))
    }
    pub fn get_byte_classes(&self) -> bool {
        self.byte_classes.unwrap_or(true)
    }
    pub fn get_line_terminator(&self) -> u8 {
        self.line_terminator.unwrap_or(b'\n')
    }
    pub fn get_pool_capacity(&self) -> usize {
        const DEFAULT_POOL_CAPACITY: usize = 8;
        self.pool_capacity.unwrap_or_else(|| {
            use crate::regex_engine::automata::util::lazy::Lazy;
            static AVAILABLE_PARALLELISM: Lazy<usize> = Lazy::new(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(DEFAULT_POOL_CAPACITY)
            });
            *Lazy::get(&AVAILABLE_PARALLELISM)
        })
    }
    pub fn get_hybrid(&self) -> bool {
        { self.hybrid.unwrap_or(true) }
    }
    pub fn get_dfa(&self) -> bool {
        { self.dfa.unwrap_or(true) }
    }
    pub fn get_onepass(&self) -> bool {
        { self.onepass.unwrap_or(true) }
    }
    pub fn get_backtrack(&self) -> bool {
        { self.backtrack.unwrap_or(true) }
    }
    pub(crate) fn to_thompson_config(&self) -> thompson::Config {
        let mut lookm = LookMatcher::new();
        lookm.set_line_terminator(self.get_line_terminator());
        thompson::Config::new()
            .utf8(self.get_utf8_empty())
            .reverse(false)
            .nfa_size_limit(self.get_nfa_size_limit())
            .shrink(false)
            .which_captures(self.get_which_captures())
            .look_matcher(lookm)
    }
    pub(crate) fn overwrite(&self, o: Config) -> Config {
        Config {
            match_kind: o.match_kind.or(self.match_kind),
            utf8_empty: o.utf8_empty.or(self.utf8_empty),
            autopre: o.autopre.or(self.autopre),
            pre: o.pre.or_else(|| self.pre.clone()),
            which_captures: o.which_captures.or(self.which_captures),
            nfa_size_limit: o.nfa_size_limit.or(self.nfa_size_limit),
            onepass_size_limit: o.onepass_size_limit.or(self.onepass_size_limit),
            hybrid_cache_capacity: o.hybrid_cache_capacity.or(self.hybrid_cache_capacity),
            hybrid: o.hybrid.or(self.hybrid),
            dfa: o.dfa.or(self.dfa),
            dfa_size_limit: o.dfa_size_limit.or(self.dfa_size_limit),
            dfa_state_limit: o.dfa_state_limit.or(self.dfa_state_limit),
            onepass: o.onepass.or(self.onepass),
            backtrack: o.backtrack.or(self.backtrack),
            byte_classes: o.byte_classes.or(self.byte_classes),
            line_terminator: o.line_terminator.or(self.line_terminator),
            pool_capacity: o.pool_capacity.or(self.pool_capacity),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Builder {
    config: Config,
    ast: ast::parse::ParserBuilder,
    hir: hir::translate::TranslatorBuilder,
}
impl Builder {
    pub fn new() -> Builder {
        Builder {
            config: Config::default(),
            ast: ast::parse::ParserBuilder::new(),
            hir: hir::translate::TranslatorBuilder::new(),
        }
    }
    pub fn build(&self, pattern: &str) -> Result<Regex, BuildError> {
        self.build_many(&[pattern])
    }
    pub fn build_many<P: AsRef<str>>(&self, patterns: &[P]) -> Result<Regex, BuildError> {
        use crate::regex_engine::automata::util::primitives::IteratorIndexExt;
        log! { debug ! ("building meta regex with {} patterns:" , patterns . len ()) ; for (pid , p) in patterns . iter () . with_pattern_ids () { let p = p . as_ref () ; let maxoff = p . char_indices () . map (| (i , ch) | i + ch . len_utf8 ()) . take (1000) . last () . unwrap_or (0) ; if maxoff < p . len () { debug ! ("{pid:?}: {}[... snip ...]" , & p [.. maxoff]) ; } else { debug ! ("{pid:?}: {p}") ; } } }
        let (mut asts, mut hirs) = (vec![], vec![]);
        for (pid, p) in patterns.iter().with_pattern_ids() {
            let ast = self
                .ast
                .build()
                .parse(p.as_ref())
                .map_err(|err| BuildError::ast(pid, err))?;
            asts.push(ast);
        }
        for ((pid, p), ast) in patterns.iter().with_pattern_ids().zip(asts.iter()) {
            let hir = self
                .hir
                .build()
                .translate(p.as_ref(), ast)
                .map_err(|err| BuildError::hir(pid, err))?;
            hirs.push(hir);
        }
        self.build_many_from_hir(&hirs)
    }
    pub fn build_from_hir(&self, hir: &Hir) -> Result<Regex, BuildError> {
        self.build_many_from_hir(&[hir])
    }
    pub fn build_many_from_hir<H: Borrow<Hir>>(&self, hirs: &[H]) -> Result<Regex, BuildError> {
        let config = self.config.clone();
        let hirs: Vec<&Hir> = hirs.iter().map(|hir| hir.borrow()).collect();
        let info = RegexInfo::new(config, &hirs);
        let strat = strategy::new(&info, &hirs)?;
        let pool = {
            let strat = Arc::clone(&strat);
            let create: CachePoolFn = Box::new(move || strat.create_cache());
            Pool::with_capacity(self.config.get_pool_capacity(), create)
        };
        Ok(Regex {
            imp: Arc::new(RegexI { strat, info }),
            pool,
        })
    }
    pub fn configure(&mut self, config: Config) -> &mut Builder {
        self.config = self.config.overwrite(config);
        self
    }
    pub fn syntax(
        &mut self,
        config: crate::regex_engine::automata::util::syntax::Config,
    ) -> &mut Builder {
        config.apply_ast(&mut self.ast);
        config.apply_hir(&mut self.hir);
        self
    }
}
