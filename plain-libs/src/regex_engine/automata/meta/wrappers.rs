use crate::regex_engine::automata::dfa;
use crate::regex_engine::automata::dfa::onepass;
use crate::regex_engine::automata::hybrid;
use crate::regex_engine::automata::nfa::thompson::backtrack;
use crate::regex_engine::automata::{
    HalfMatch, Input, Match, MatchKind, PatternID, PatternSet,
    meta::{
        error::{BuildError, RetryError, RetryFailError},
        regex::RegexInfo,
    },
    nfa::thompson::{NFA, pikevm},
    util::{prefilter::Prefilter, primitives::NonMaxUsize},
};
use std::vec::Vec;
#[derive(Debug)]
pub(crate) struct PikeVM(PikeVMEngine);
impl PikeVM {
    pub(crate) fn new(
        info: &RegexInfo,
        pre: Option<Prefilter>,
        nfa: &NFA,
    ) -> Result<PikeVM, BuildError> {
        PikeVMEngine::new(info, pre, nfa).map(PikeVM)
    }
    pub(crate) fn create_cache(&self) -> PikeVMCache {
        PikeVMCache::none()
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn get(&self) -> &PikeVMEngine {
        &self.0
    }
}
#[derive(Debug)]
pub(crate) struct PikeVMEngine(pikevm::PikeVM);
impl PikeVMEngine {
    pub(crate) fn new(
        info: &RegexInfo,
        pre: Option<Prefilter>,
        nfa: &NFA,
    ) -> Result<PikeVMEngine, BuildError> {
        let pikevm_config = pikevm::Config::new()
            .match_kind(info.config().get_match_kind())
            .prefilter(pre);
        let engine = pikevm::Builder::new()
            .configure(pikevm_config)
            .build_from_nfa(nfa.clone())
            .map_err(BuildError::nfa)?;
        debug!("PikeVM built");
        Ok(PikeVMEngine(engine))
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn is_match(&self, cache: &mut PikeVMCache, input: &Input<'_>) -> bool {
        self.0.is_match(cache.get(&self.0), input.clone())
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn search_slots(
        &self,
        cache: &mut PikeVMCache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        self.0.search_slots(cache.get(&self.0), input, slots)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn which_overlapping_matches(
        &self,
        cache: &mut PikeVMCache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) {
        self.0
            .which_overlapping_matches(cache.get(&self.0), input, patset)
    }
}
#[derive(Clone, Debug)]
pub(crate) struct PikeVMCache(Option<pikevm::Cache>);
impl PikeVMCache {
    pub(crate) fn none() -> PikeVMCache {
        PikeVMCache(None)
    }
    pub(crate) fn reset(&mut self, builder: &PikeVM) {
        self.get(&builder.get().0).reset(&builder.get().0);
    }
    pub(crate) fn memory_usage(&self) -> usize {
        self.0.as_ref().map_or(0, |c| c.memory_usage())
    }
    fn get(&mut self, vm: &pikevm::PikeVM) -> &mut pikevm::Cache {
        self.0.get_or_insert_with(|| vm.create_cache())
    }
}
#[derive(Debug)]
pub(crate) struct BoundedBacktracker(Option<BoundedBacktrackerEngine>);
impl BoundedBacktracker {
    pub(crate) fn new(
        info: &RegexInfo,
        pre: Option<Prefilter>,
        nfa: &NFA,
    ) -> Result<BoundedBacktracker, BuildError> {
        BoundedBacktrackerEngine::new(info, pre, nfa).map(BoundedBacktracker)
    }
    pub(crate) fn create_cache(&self) -> BoundedBacktrackerCache {
        BoundedBacktrackerCache::none()
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn get(&self, input: &Input<'_>) -> Option<&BoundedBacktrackerEngine> {
        let engine = self.0.as_ref()?;
        if input.get_earliest() && input.haystack().len() > 128 {
            return None;
        }
        if input.get_span().len() > engine.max_haystack_len() {
            return None;
        }
        Some(engine)
    }
}
#[derive(Debug)]
pub(crate) struct BoundedBacktrackerEngine(
    #[cfg(all())] backtrack::BoundedBacktracker,
    #[cfg(not(all()))] (),
);
impl BoundedBacktrackerEngine {
    pub(crate) fn new(
        info: &RegexInfo,
        pre: Option<Prefilter>,
        nfa: &NFA,
    ) -> Result<Option<BoundedBacktrackerEngine>, BuildError> {
        {
            if !info.config().get_backtrack()
                || info.config().get_match_kind() != MatchKind::LeftmostFirst
            {
                return Ok(None);
            }
            let backtrack_config = backtrack::Config::new().prefilter(pre);
            let engine = backtrack::Builder::new()
                .configure(backtrack_config)
                .build_from_nfa(nfa.clone())
                .map_err(BuildError::nfa)?;
            debug!(
                "BoundedBacktracker built (max haystack length: {:?})",
                engine.max_haystack_len()
            );
            Ok(Some(BoundedBacktrackerEngine(engine)))
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn is_match(&self, cache: &mut BoundedBacktrackerCache, input: &Input<'_>) -> bool {
        {
            self.0
                .try_is_match(cache.get(&self.0), input.clone())
                .unwrap()
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn search_slots(
        &self,
        cache: &mut BoundedBacktrackerCache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        {
            self.0
                .try_search_slots(cache.get(&self.0), input, slots)
                .unwrap()
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn max_haystack_len(&self) -> usize {
        { self.0.max_haystack_len() }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct BoundedBacktrackerCache(
    #[cfg(all())] Option<backtrack::Cache>,
    #[cfg(not(all()))] (),
);
impl BoundedBacktrackerCache {
    pub(crate) fn none() -> BoundedBacktrackerCache {
        { BoundedBacktrackerCache(None) }
    }
    pub(crate) fn reset(&mut self, builder: &BoundedBacktracker) {
        if let Some(ref e) = builder.0 {
            self.get(&e.0).reset(&e.0);
        }
    }
    pub(crate) fn memory_usage(&self) -> usize {
        { self.0.as_ref().map_or(0, |c| c.memory_usage()) }
    }
    fn get(&mut self, bb: &backtrack::BoundedBacktracker) -> &mut backtrack::Cache {
        self.0.get_or_insert_with(|| bb.create_cache())
    }
}
#[derive(Debug)]
pub(crate) struct OnePass(Option<OnePassEngine>);
impl OnePass {
    pub(crate) fn new(info: &RegexInfo, nfa: &NFA) -> OnePass {
        OnePass(OnePassEngine::new(info, nfa))
    }
    pub(crate) fn create_cache(&self) -> OnePassCache {
        OnePassCache::new(self)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn get(&self, input: &Input<'_>) -> Option<&OnePassEngine> {
        let engine = self.0.as_ref()?;
        if !input.get_anchored().is_anchored() && !engine.get_nfa().is_always_start_anchored() {
            return None;
        }
        Some(engine)
    }
    pub(crate) fn memory_usage(&self) -> usize {
        self.0.as_ref().map_or(0, |e| e.memory_usage())
    }
}
#[derive(Debug)]
pub(crate) struct OnePassEngine(#[cfg(all())] onepass::DFA, #[cfg(not(all()))] ());
impl OnePassEngine {
    pub(crate) fn new(info: &RegexInfo, nfa: &NFA) -> Option<OnePassEngine> {
        {
            if !info.config().get_onepass() {
                return None;
            }
            if info.props_union().explicit_captures_len() == 0
                && !info.props_union().look_set().contains_word_unicode()
            {
                debug!("not building OnePass because it isn't worth it");
                return None;
            }
            let onepass_config = onepass::Config::new()
                .match_kind(info.config().get_match_kind())
                .starts_for_each_pattern(true)
                .byte_classes(info.config().get_byte_classes())
                .size_limit(info.config().get_onepass_size_limit());
            let result = onepass::Builder::new()
                .configure(onepass_config)
                .build_from_nfa(nfa.clone());
            let engine = match result {
                Ok(engine) => engine,
                Err(_err) => {
                    debug!("OnePass failed to build: {_err}");
                    return None;
                }
            };
            debug!("OnePass built, {} bytes", engine.memory_usage());
            Some(OnePassEngine(engine))
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn search_slots(
        &self,
        cache: &mut OnePassCache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        {
            self.0
                .try_search_slots(cache.0.as_mut().unwrap(), input, slots)
                .unwrap()
        }
    }
    pub(crate) fn memory_usage(&self) -> usize {
        { self.0.memory_usage() }
    }
    #[cfg_attr(all(), inline(always))]
    fn get_nfa(&self) -> &NFA {
        { self.0.get_nfa() }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct OnePassCache(#[cfg(all())] Option<onepass::Cache>, #[cfg(not(all()))] ());
impl OnePassCache {
    pub(crate) fn none() -> OnePassCache {
        { OnePassCache(None) }
    }
    pub(crate) fn new(builder: &OnePass) -> OnePassCache {
        { OnePassCache(builder.0.as_ref().map(|e| e.0.create_cache())) }
    }
    pub(crate) fn reset(&mut self, builder: &OnePass) {
        if let Some(ref e) = builder.0 {
            self.0.as_mut().unwrap().reset(&e.0);
        }
    }
    pub(crate) fn memory_usage(&self) -> usize {
        { self.0.as_ref().map_or(0, |c| c.memory_usage()) }
    }
}
#[derive(Debug)]
pub(crate) struct Hybrid(Option<HybridEngine>);
impl Hybrid {
    pub(crate) fn none() -> Hybrid {
        Hybrid(None)
    }
    pub(crate) fn new(info: &RegexInfo, pre: Option<Prefilter>, nfa: &NFA, nfarev: &NFA) -> Hybrid {
        Hybrid(HybridEngine::new(info, pre, nfa, nfarev))
    }
    pub(crate) fn create_cache(&self) -> HybridCache {
        HybridCache::new(self)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn get(&self, _input: &Input<'_>) -> Option<&HybridEngine> {
        let engine = self.0.as_ref()?;
        Some(engine)
    }
    pub(crate) fn is_some(&self) -> bool {
        self.0.is_some()
    }
}
#[derive(Debug)]
pub(crate) struct HybridEngine(#[cfg(all())] hybrid::regex::Regex, #[cfg(not(all()))] ());
impl HybridEngine {
    pub(crate) fn new(
        info: &RegexInfo,
        pre: Option<Prefilter>,
        nfa: &NFA,
        nfarev: &NFA,
    ) -> Option<HybridEngine> {
        {
            if !info.config().get_hybrid() {
                return None;
            }
            let dfa_config = hybrid::dfa::Config::new()
                .match_kind(info.config().get_match_kind())
                .prefilter(pre.clone())
                .starts_for_each_pattern(true)
                .byte_classes(info.config().get_byte_classes())
                .unicode_word_boundary(true)
                .specialize_start_states(pre.is_some())
                .cache_capacity(info.config().get_hybrid_cache_capacity())
                .skip_cache_capacity_check(false)
                .minimum_cache_clear_count(Some(3))
                .minimum_bytes_per_state(Some(10));
            let result = hybrid::dfa::Builder::new()
                .configure(dfa_config.clone())
                .build_from_nfa(nfa.clone());
            let fwd = match result {
                Ok(fwd) => fwd,
                Err(_err) => {
                    debug!("forward lazy DFA failed to build: {_err}");
                    return None;
                }
            };
            let result = hybrid::dfa::Builder::new()
                .configure(
                    dfa_config
                        .clone()
                        .match_kind(MatchKind::All)
                        .prefilter(None)
                        .specialize_start_states(false),
                )
                .build_from_nfa(nfarev.clone());
            let rev = match result {
                Ok(rev) => rev,
                Err(_err) => {
                    debug!("reverse lazy DFA failed to build: {_err}");
                    return None;
                }
            };
            let engine = hybrid::regex::Builder::new().build_from_dfas(fwd, rev);
            debug!("lazy DFA built");
            Some(HybridEngine(engine))
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search(
        &self,
        cache: &mut HybridCache,
        input: &Input<'_>,
    ) -> Result<Option<Match>, RetryFailError> {
        {
            let cache = cache.0.as_mut().unwrap();
            self.0.try_search(cache, input).map_err(|e| e.into())
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search_half_fwd(
        &self,
        cache: &mut HybridCache,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, RetryFailError> {
        {
            let fwd = self.0.forward();
            let mut fwdcache = cache.0.as_mut().unwrap().as_parts_mut().0;
            fwd.try_search_fwd(&mut fwdcache, input)
                .map_err(|e| e.into())
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search_half_fwd_stopat(
        &self,
        cache: &mut HybridCache,
        input: &Input<'_>,
    ) -> Result<Result<HalfMatch, usize>, RetryFailError> {
        {
            let dfa = self.0.forward();
            let mut cache = cache.0.as_mut().unwrap().as_parts_mut().0;
            crate::regex_engine::automata::meta::stopat::hybrid_try_search_half_fwd(
                dfa, &mut cache, input,
            )
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search_half_rev(
        &self,
        cache: &mut HybridCache,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, RetryFailError> {
        {
            let rev = self.0.reverse();
            let mut revcache = cache.0.as_mut().unwrap().as_parts_mut().1;
            rev.try_search_rev(&mut revcache, input)
                .map_err(|e| e.into())
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search_half_rev_limited(
        &self,
        cache: &mut HybridCache,
        input: &Input<'_>,
        min_start: usize,
    ) -> Result<Option<HalfMatch>, RetryError> {
        {
            let dfa = self.0.reverse();
            let mut cache = cache.0.as_mut().unwrap().as_parts_mut().1;
            crate::regex_engine::automata::meta::limited::hybrid_try_search_half_rev(
                dfa, &mut cache, input, min_start,
            )
        }
    }
    #[inline]
    pub(crate) fn try_which_overlapping_matches(
        &self,
        cache: &mut HybridCache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) -> Result<(), RetryFailError> {
        {
            let fwd = self.0.forward();
            let mut fwdcache = cache.0.as_mut().unwrap().as_parts_mut().0;
            fwd.try_which_overlapping_matches(&mut fwdcache, input, patset)
                .map_err(|e| e.into())
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct HybridCache(
    #[cfg(all())] Option<hybrid::regex::Cache>,
    #[cfg(not(all()))] (),
);
impl HybridCache {
    pub(crate) fn none() -> HybridCache {
        { HybridCache(None) }
    }
    pub(crate) fn new(builder: &Hybrid) -> HybridCache {
        { HybridCache(builder.0.as_ref().map(|e| e.0.create_cache())) }
    }
    pub(crate) fn reset(&mut self, builder: &Hybrid) {
        if let Some(ref e) = builder.0 {
            self.0.as_mut().unwrap().reset(&e.0);
        }
    }
    pub(crate) fn memory_usage(&self) -> usize {
        { self.0.as_ref().map_or(0, |c| c.memory_usage()) }
    }
}
#[derive(Debug)]
pub(crate) struct DFA(Option<DFAEngine>);
impl DFA {
    pub(crate) fn none() -> DFA {
        DFA(None)
    }
    pub(crate) fn new(info: &RegexInfo, pre: Option<Prefilter>, nfa: &NFA, nfarev: &NFA) -> DFA {
        DFA(DFAEngine::new(info, pre, nfa, nfarev))
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn get(&self, _input: &Input<'_>) -> Option<&DFAEngine> {
        let engine = self.0.as_ref()?;
        Some(engine)
    }
    pub(crate) fn is_some(&self) -> bool {
        self.0.is_some()
    }
    pub(crate) fn memory_usage(&self) -> usize {
        self.0.as_ref().map_or(0, |e| e.memory_usage())
    }
}
#[derive(Debug)]
pub(crate) struct DFAEngine(#[cfg(all())] dfa::regex::Regex, #[cfg(not(all()))] ());
impl DFAEngine {
    pub(crate) fn new(
        info: &RegexInfo,
        pre: Option<Prefilter>,
        nfa: &NFA,
        nfarev: &NFA,
    ) -> Option<DFAEngine> {
        {
            if !info.config().get_dfa() {
                return None;
            }
            if let Some(state_limit) = info.config().get_dfa_state_limit() {
                if nfa.states().len() > state_limit {
                    debug!(
                        "skipping full DFA because NFA has {} states, \
                         which exceeds the heuristic limit of {}",
                        nfa.states().len(),
                        state_limit,
                    );
                    return None;
                }
            }
            let size_limit = info.config().get_dfa_size_limit().map(|n| n / 4);
            let dfa_config = dfa::dense::Config::new()
                .match_kind(info.config().get_match_kind())
                .prefilter(pre.clone())
                .starts_for_each_pattern(true)
                .byte_classes(info.config().get_byte_classes())
                .unicode_word_boundary(true)
                .specialize_start_states(pre.is_some())
                .determinize_size_limit(size_limit)
                .dfa_size_limit(size_limit);
            let result = dfa::dense::Builder::new()
                .configure(dfa_config.clone())
                .build_from_nfa(&nfa);
            let fwd = match result {
                Ok(fwd) => fwd,
                Err(_err) => {
                    debug!("forward full DFA failed to build: {_err}");
                    return None;
                }
            };
            let result = dfa::dense::Builder::new()
                .configure(
                    dfa_config
                        .clone()
                        .start_kind(dfa::StartKind::Anchored)
                        .match_kind(MatchKind::All)
                        .prefilter(None)
                        .specialize_start_states(false),
                )
                .build_from_nfa(&nfarev);
            let rev = match result {
                Ok(rev) => rev,
                Err(_err) => {
                    debug!("reverse full DFA failed to build: {_err}");
                    return None;
                }
            };
            let engine = dfa::regex::Builder::new().build_from_dfas(fwd, rev);
            debug!(
                "fully compiled forward and reverse DFAs built, {} bytes",
                engine.forward().memory_usage() + engine.reverse().memory_usage(),
            );
            Some(DFAEngine(engine))
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search(&self, input: &Input<'_>) -> Result<Option<Match>, RetryFailError> {
        { self.0.try_search(input).map_err(|e| e.into()) }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search_half_fwd(
        &self,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, RetryFailError> {
        {
            use crate::regex_engine::automata::dfa::Automaton;
            self.0.forward().try_search_fwd(input).map_err(|e| e.into())
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search_half_fwd_stopat(
        &self,
        input: &Input<'_>,
    ) -> Result<Result<HalfMatch, usize>, RetryFailError> {
        {
            let dfa = self.0.forward();
            crate::regex_engine::automata::meta::stopat::dfa_try_search_half_fwd(dfa, input)
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search_half_rev(
        &self,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, RetryFailError> {
        {
            use crate::regex_engine::automata::dfa::Automaton;
            self.0
                .reverse()
                .try_search_rev(&input)
                .map_err(|e| e.into())
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search_half_rev_limited(
        &self,
        input: &Input<'_>,
        min_start: usize,
    ) -> Result<Option<HalfMatch>, RetryError> {
        {
            let dfa = self.0.reverse();
            crate::regex_engine::automata::meta::limited::dfa_try_search_half_rev(
                dfa, input, min_start,
            )
        }
    }
    #[inline]
    pub(crate) fn try_which_overlapping_matches(
        &self,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) -> Result<(), RetryFailError> {
        {
            use crate::regex_engine::automata::dfa::Automaton;
            self.0
                .forward()
                .try_which_overlapping_matches(input, patset)
                .map_err(|e| e.into())
        }
    }
    pub(crate) fn memory_usage(&self) -> usize {
        { self.0.forward().memory_usage() + self.0.reverse().memory_usage() }
    }
}
#[derive(Debug)]
pub(crate) struct ReverseHybrid(Option<ReverseHybridEngine>);
impl ReverseHybrid {
    pub(crate) fn none() -> ReverseHybrid {
        ReverseHybrid(None)
    }
    pub(crate) fn new(info: &RegexInfo, nfarev: &NFA) -> ReverseHybrid {
        ReverseHybrid(ReverseHybridEngine::new(info, nfarev))
    }
    pub(crate) fn create_cache(&self) -> ReverseHybridCache {
        ReverseHybridCache::new(self)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn get(&self, _input: &Input<'_>) -> Option<&ReverseHybridEngine> {
        let engine = self.0.as_ref()?;
        Some(engine)
    }
}
#[derive(Debug)]
pub(crate) struct ReverseHybridEngine(#[cfg(all())] hybrid::dfa::DFA, #[cfg(not(all()))] ());
impl ReverseHybridEngine {
    pub(crate) fn new(info: &RegexInfo, nfarev: &NFA) -> Option<ReverseHybridEngine> {
        {
            if !info.config().get_hybrid() {
                return None;
            }
            let dfa_config = hybrid::dfa::Config::new()
                .match_kind(MatchKind::All)
                .prefilter(None)
                .starts_for_each_pattern(false)
                .byte_classes(info.config().get_byte_classes())
                .unicode_word_boundary(true)
                .specialize_start_states(false)
                .cache_capacity(info.config().get_hybrid_cache_capacity())
                .skip_cache_capacity_check(false)
                .minimum_cache_clear_count(Some(3))
                .minimum_bytes_per_state(Some(10));
            let result = hybrid::dfa::Builder::new()
                .configure(dfa_config)
                .build_from_nfa(nfarev.clone());
            let rev = match result {
                Ok(rev) => rev,
                Err(_err) => {
                    debug!("lazy reverse DFA failed to build: {_err}");
                    return None;
                }
            };
            debug!("lazy reverse DFA built");
            Some(ReverseHybridEngine(rev))
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search_half_rev_limited(
        &self,
        cache: &mut ReverseHybridCache,
        input: &Input<'_>,
        min_start: usize,
    ) -> Result<Option<HalfMatch>, RetryError> {
        {
            let dfa = &self.0;
            let mut cache = cache.0.as_mut().unwrap();
            crate::regex_engine::automata::meta::limited::hybrid_try_search_half_rev(
                dfa, &mut cache, input, min_start,
            )
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct ReverseHybridCache(
    #[cfg(all())] Option<hybrid::dfa::Cache>,
    #[cfg(not(all()))] (),
);
impl ReverseHybridCache {
    pub(crate) fn none() -> ReverseHybridCache {
        { ReverseHybridCache(None) }
    }
    pub(crate) fn new(builder: &ReverseHybrid) -> ReverseHybridCache {
        { ReverseHybridCache(builder.0.as_ref().map(|e| e.0.create_cache())) }
    }
    pub(crate) fn reset(&mut self, builder: &ReverseHybrid) {
        if let Some(ref e) = builder.0 {
            self.0.as_mut().unwrap().reset(&e.0);
        }
    }
    pub(crate) fn memory_usage(&self) -> usize {
        { self.0.as_ref().map_or(0, |c| c.memory_usage()) }
    }
}
#[derive(Debug)]
pub(crate) struct ReverseDFA(Option<ReverseDFAEngine>);
impl ReverseDFA {
    pub(crate) fn none() -> ReverseDFA {
        ReverseDFA(None)
    }
    pub(crate) fn new(info: &RegexInfo, nfarev: &NFA) -> ReverseDFA {
        ReverseDFA(ReverseDFAEngine::new(info, nfarev))
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn get(&self, _input: &Input<'_>) -> Option<&ReverseDFAEngine> {
        let engine = self.0.as_ref()?;
        Some(engine)
    }
    pub(crate) fn is_some(&self) -> bool {
        self.0.is_some()
    }
    pub(crate) fn memory_usage(&self) -> usize {
        self.0.as_ref().map_or(0, |e| e.memory_usage())
    }
}
#[derive(Debug)]
pub(crate) struct ReverseDFAEngine(
    #[cfg(all())] dfa::dense::DFA<Vec<u32>>,
    #[cfg(not(all()))] (),
);
impl ReverseDFAEngine {
    pub(crate) fn new(info: &RegexInfo, nfarev: &NFA) -> Option<ReverseDFAEngine> {
        {
            if !info.config().get_dfa() {
                return None;
            }
            if let Some(state_limit) = info.config().get_dfa_state_limit() {
                if nfarev.states().len() > state_limit {
                    debug!(
                        "skipping full reverse DFA because NFA has {} states, \
                         which exceeds the heuristic limit of {}",
                        nfarev.states().len(),
                        state_limit,
                    );
                    return None;
                }
            }
            let size_limit = info.config().get_dfa_size_limit().map(|n| n / 2);
            let dfa_config = dfa::dense::Config::new()
                .match_kind(MatchKind::All)
                .prefilter(None)
                .accelerate(false)
                .start_kind(dfa::StartKind::Anchored)
                .starts_for_each_pattern(false)
                .byte_classes(info.config().get_byte_classes())
                .unicode_word_boundary(true)
                .specialize_start_states(false)
                .determinize_size_limit(size_limit)
                .dfa_size_limit(size_limit);
            let result = dfa::dense::Builder::new()
                .configure(dfa_config)
                .build_from_nfa(&nfarev);
            let rev = match result {
                Ok(rev) => rev,
                Err(_err) => {
                    debug!("full reverse DFA failed to build: {_err}");
                    return None;
                }
            };
            debug!(
                "fully compiled reverse DFA built, {} bytes",
                rev.memory_usage()
            );
            Some(ReverseDFAEngine(rev))
        }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn try_search_half_rev_limited(
        &self,
        input: &Input<'_>,
        min_start: usize,
    ) -> Result<Option<HalfMatch>, RetryError> {
        {
            let dfa = &self.0;
            crate::regex_engine::automata::meta::limited::dfa_try_search_half_rev(
                dfa, input, min_start,
            )
        }
    }
    pub(crate) fn memory_usage(&self) -> usize {
        { self.0.memory_usage() }
    }
}
