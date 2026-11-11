use crate::regex_engine::automata::{
    meta::{
        error::{BuildError, RetryError, RetryFailError, RetryQuadraticError},
        regex::{Cache, RegexInfo},
        reverse_inner, reverse_suffix, wrappers,
    },
    nfa::thompson::{self, NFA, WhichCaptures},
    util::{
        captures::{Captures, GroupInfo},
        look::LookMatcher,
        prefilter::{self, Prefilter, PrefilterI},
        primitives::{NonMaxUsize, PatternID},
        search::{Anchored, HalfMatch, Input, Match, MatchKind, PatternSet},
    },
};
use crate::regex_engine::syntax::hir::{Hir, literal};
use core::{
    fmt::Debug,
    panic::{RefUnwindSafe, UnwindSafe},
};
use std::{borrow::Cow, format, sync::Arc};
pub(super) trait Strategy:
    Debug + Send + Sync + RefUnwindSafe + UnwindSafe + 'static
{
    fn name(&self) -> Cow<'static, str>;
    fn group_info(&self) -> &GroupInfo;
    fn create_cache(&self) -> Cache;
    fn reset_cache(&self, cache: &mut Cache);
    fn is_accelerated(&self) -> bool;
    fn memory_usage(&self) -> usize;
    fn search(&self, cache: &mut Cache, input: &Input<'_>) -> Option<Match>;
    fn search_half(&self, cache: &mut Cache, input: &Input<'_>) -> Option<HalfMatch>;
    fn is_match(&self, cache: &mut Cache, input: &Input<'_>) -> bool;
    fn search_slots(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID>;
    fn which_overlapping_matches(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    );
}
pub(super) fn new(info: &RegexInfo, hirs: &[&Hir]) -> Result<Arc<dyn Strategy>, BuildError> {
    let pre = if info.is_always_anchored_start() {
        debug!("skipping literal extraction since regex is anchored");
        None
    } else if let Some(pre) = info.config().get_prefilter() {
        debug!("skipping literal extraction since the caller provided a prefilter");
        Some(pre.clone())
    } else if info.config().get_auto_prefilter() {
        let kind = info.config().get_match_kind();
        let prefixes = crate::regex_engine::automata::util::prefilter::prefixes(kind, hirs);
        if let Some(pre) = Pre::from_prefixes(info, &prefixes) {
            debug!(
                "found that the regex can be broken down to a literal \
                 search, avoiding the regex engine entirely",
            );
            debug!("using {} strategy", pre.name());
            return Ok(pre);
        }
        if let Some(pre) = Pre::from_alternation_literals(info, hirs) {
            debug!(
                "found plain alternation of literals, \
                 avoiding regex engine entirely and using Aho-Corasick"
            );
            debug!("using {} strategy", pre.name());
            return Ok(pre);
        }
        prefixes.literals().and_then(|strings| {
            debug!(
                "creating prefilter from {} literals: {:?}",
                strings.len(),
                strings,
            );
            Prefilter::new(kind, strings)
        })
    } else {
        debug!("skipping literal extraction since prefilters were disabled");
        None
    };
    let mut core = Core::new(info.clone(), pre.clone(), hirs)?;
    core = match ReverseAnchored::new(core) {
        Err(core) => core,
        Ok(ra) => {
            debug!("using {} strategy", ra.name());
            return Ok(Arc::new(ra));
        }
    };
    core = match ReverseSuffix::new(core, hirs) {
        Err(core) => core,
        Ok(rs) => {
            debug!("using {} strategy", rs.name());
            return Ok(Arc::new(rs));
        }
    };
    core = match ReverseInner::new(core, hirs) {
        Err(core) => core,
        Ok(ri) => {
            debug!("using {} strategy", ri.name());
            return Ok(Arc::new(ri));
        }
    };
    debug!("using {} strategy", core.name());
    Ok(Arc::new(core))
}
#[derive(Clone, Debug)]
struct Pre<P> {
    pre: P,
    group_info: GroupInfo,
}
impl<P: PrefilterI> Pre<P> {
    fn new(pre: P) -> Arc<dyn Strategy> {
        let group_info = GroupInfo::new([[None::<&str>]]).unwrap();
        Arc::new(Pre { pre, group_info })
    }
}
impl Pre<()> {
    fn from_prefixes(info: &RegexInfo, prefixes: &literal::Seq) -> Option<Arc<dyn Strategy>> {
        let kind = info.config().get_match_kind();
        if !prefixes.is_exact() {
            return None;
        }
        if info.pattern_len() != 1 {
            return None;
        }
        if info.props()[0].explicit_captures_len() != 0 {
            return None;
        }
        if !info.props()[0].look_set().is_empty() {
            return None;
        }
        if kind != MatchKind::LeftmostFirst {
            return None;
        }
        let prefixes = prefixes.literals().unwrap();
        debug!(
            "trying to bypass regex engine by creating \
             prefilter from {} literals: {:?}",
            prefixes.len(),
            prefixes,
        );
        let choice = match prefilter::Choice::new(kind, prefixes) {
            Some(choice) => choice,
            None => {
                debug!("regex bypass failed because no prefilter could be built");
                return None;
            }
        };
        let strat: Arc<dyn Strategy> = match choice {
            prefilter::Choice::Memchr(pre) => Pre::new(pre),
            prefilter::Choice::Memchr2(pre) => Pre::new(pre),
            prefilter::Choice::Memchr3(pre) => Pre::new(pre),
            prefilter::Choice::Memmem(pre) => Pre::new(pre),
            prefilter::Choice::Teddy(pre) => Pre::new(pre),
            prefilter::Choice::ByteSet(pre) => Pre::new(pre),
            prefilter::Choice::AhoCorasick(pre) => Pre::new(pre),
        };
        Some(strat)
    }
    fn from_alternation_literals(info: &RegexInfo, hirs: &[&Hir]) -> Option<Arc<dyn Strategy>> {
        use crate::regex_engine::automata::util::prefilter::AhoCorasick;
        let lits = crate::regex_engine::automata::meta::literal::alternation_literals(info, hirs)?;
        let ac = AhoCorasick::new(MatchKind::LeftmostFirst, &lits)?;
        Some(Pre::new(ac))
    }
}
impl<P: PrefilterI> Strategy for Pre<P> {
    fn name(&self) -> Cow<'static, str> {
        Cow::Owned(format!("prefilter {}", self.pre.name()))
    }
    #[cfg_attr(all(), inline(always))]
    fn group_info(&self) -> &GroupInfo {
        &self.group_info
    }
    fn create_cache(&self) -> Cache {
        Cache {
            capmatches: Captures::all(self.group_info().clone()),
            pikevm: wrappers::PikeVMCache::none(),
            backtrack: wrappers::BoundedBacktrackerCache::none(),
            onepass: wrappers::OnePassCache::none(),
            hybrid: wrappers::HybridCache::none(),
            revhybrid: wrappers::ReverseHybridCache::none(),
        }
    }
    fn reset_cache(&self, _cache: &mut Cache) {}
    fn is_accelerated(&self) -> bool {
        self.pre.is_fast()
    }
    fn memory_usage(&self) -> usize {
        self.pre.memory_usage()
    }
    #[cfg_attr(all(), inline(always))]
    fn search(&self, _cache: &mut Cache, input: &Input<'_>) -> Option<Match> {
        if input.is_done() {
            return None;
        }
        if input.get_anchored().is_anchored() {
            return self
                .pre
                .prefix(input.haystack(), input.get_span())
                .map(|sp| Match::new(PatternID::ZERO, sp));
        }
        self.pre
            .find(input.haystack(), input.get_span())
            .map(|sp| Match::new(PatternID::ZERO, sp))
    }
    #[cfg_attr(all(), inline(always))]
    fn search_half(&self, cache: &mut Cache, input: &Input<'_>) -> Option<HalfMatch> {
        self.search(cache, input)
            .map(|m| HalfMatch::new(m.pattern(), m.end()))
    }
    #[cfg_attr(all(), inline(always))]
    fn is_match(&self, cache: &mut Cache, input: &Input<'_>) -> bool {
        self.search(cache, input).is_some()
    }
    #[cfg_attr(all(), inline(always))]
    fn search_slots(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        let m = self.search(cache, input)?;
        if let Some(slot) = slots.get_mut(0) {
            *slot = NonMaxUsize::new(m.start());
        }
        if let Some(slot) = slots.get_mut(1) {
            *slot = NonMaxUsize::new(m.end());
        }
        Some(m.pattern())
    }
    #[cfg_attr(all(), inline(always))]
    fn which_overlapping_matches(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) {
        if self.search(cache, input).is_some() {
            patset.insert(PatternID::ZERO);
        }
    }
}
#[derive(Debug)]
struct Core {
    info: RegexInfo,
    pre: Option<Prefilter>,
    nfa: NFA,
    nfarev: Option<NFA>,
    pikevm: wrappers::PikeVM,
    backtrack: wrappers::BoundedBacktracker,
    onepass: wrappers::OnePass,
    hybrid: wrappers::Hybrid,
    dfa: wrappers::DFA,
}
impl Core {
    fn new(info: RegexInfo, pre: Option<Prefilter>, hirs: &[&Hir]) -> Result<Core, BuildError> {
        let mut lookm = LookMatcher::new();
        lookm.set_line_terminator(info.config().get_line_terminator());
        let thompson_config = info.config().to_thompson_config();
        let nfa = thompson::Compiler::new()
            .configure(thompson_config.clone())
            .build_many_from_hir(hirs)
            .map_err(BuildError::nfa)?;
        let pikevm = wrappers::PikeVM::new(&info, pre.clone(), &nfa)?;
        let backtrack = wrappers::BoundedBacktracker::new(&info, pre.clone(), &nfa)?;
        let onepass = wrappers::OnePass::new(&info, &nfa);
        let (nfarev, hybrid, dfa) = if !info.config().get_hybrid() && !info.config().get_dfa() {
            (None, wrappers::Hybrid::none(), wrappers::DFA::none())
        } else {
            let nfarev = thompson::Compiler::new()
                .configure(
                    thompson_config
                        .which_captures(WhichCaptures::None)
                        .reverse(true),
                )
                .build_many_from_hir(hirs)
                .map_err(BuildError::nfa)?;
            let dfa = if !info.config().get_dfa() {
                wrappers::DFA::none()
            } else {
                wrappers::DFA::new(&info, pre.clone(), &nfa, &nfarev)
            };
            let hybrid = if !info.config().get_hybrid() {
                wrappers::Hybrid::none()
            } else if dfa.is_some() {
                debug!("skipping lazy DFA because we have a full DFA");
                wrappers::Hybrid::none()
            } else {
                wrappers::Hybrid::new(&info, pre.clone(), &nfa, &nfarev)
            };
            (Some(nfarev), hybrid, dfa)
        };
        Ok(Core {
            info,
            pre,
            nfa,
            nfarev,
            pikevm,
            backtrack,
            onepass,
            hybrid,
            dfa,
        })
    }
    #[cfg_attr(all(), inline(always))]
    fn try_search_mayfail(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Option<Result<Option<Match>, RetryFailError>> {
        if let Some(e) = self.dfa.get(input) {
            trace!("using full DFA for search at {:?}", input.get_span());
            Some(e.try_search(input))
        } else if let Some(e) = self.hybrid.get(input) {
            trace!("using lazy DFA for search at {:?}", input.get_span());
            Some(e.try_search(&mut cache.hybrid, input))
        } else {
            None
        }
    }
    fn search_nofail(&self, cache: &mut Cache, input: &Input<'_>) -> Option<Match> {
        let caps = &mut cache.capmatches;
        caps.set_pattern(None);
        let pid = if let Some(ref e) = self.onepass.get(input) {
            trace!("using OnePass for search at {:?}", input.get_span());
            e.search_slots(&mut cache.onepass, input, caps.slots_mut())
        } else if let Some(ref e) = self.backtrack.get(input) {
            trace!(
                "using BoundedBacktracker for search at {:?}",
                input.get_span()
            );
            e.search_slots(&mut cache.backtrack, input, caps.slots_mut())
        } else {
            trace!("using PikeVM for search at {:?}", input.get_span());
            let e = self.pikevm.get();
            e.search_slots(&mut cache.pikevm, input, caps.slots_mut())
        };
        caps.set_pattern(pid);
        caps.get_match()
    }
    fn search_half_nofail(&self, cache: &mut Cache, input: &Input<'_>) -> Option<HalfMatch> {
        let m = self.search_nofail(cache, input)?;
        Some(HalfMatch::new(m.pattern(), m.end()))
    }
    fn search_slots_nofail(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        if let Some(ref e) = self.onepass.get(input) {
            trace!("using OnePass for capture search at {:?}", input.get_span());
            e.search_slots(&mut cache.onepass, input, slots)
        } else if let Some(ref e) = self.backtrack.get(input) {
            trace!(
                "using BoundedBacktracker for capture search at {:?}",
                input.get_span()
            );
            e.search_slots(&mut cache.backtrack, input, slots)
        } else {
            trace!("using PikeVM for capture search at {:?}", input.get_span());
            let e = self.pikevm.get();
            e.search_slots(&mut cache.pikevm, input, slots)
        }
    }
    fn is_match_nofail(&self, cache: &mut Cache, input: &Input<'_>) -> bool {
        if let Some(ref e) = self.onepass.get(input) {
            trace!(
                "using OnePass for is-match search at {:?}",
                input.get_span()
            );
            e.search_slots(&mut cache.onepass, input, &mut []).is_some()
        } else if let Some(ref e) = self.backtrack.get(input) {
            trace!(
                "using BoundedBacktracker for is-match search at {:?}",
                input.get_span()
            );
            e.is_match(&mut cache.backtrack, input)
        } else {
            trace!("using PikeVM for is-match search at {:?}", input.get_span());
            let e = self.pikevm.get();
            e.is_match(&mut cache.pikevm, input)
        }
    }
    fn is_capture_search_needed(&self, slots_len: usize) -> bool {
        slots_len > self.nfa.group_info().implicit_slot_len()
    }
}
impl Strategy for Core {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("core")
    }
    #[cfg_attr(all(), inline(always))]
    fn group_info(&self) -> &GroupInfo {
        self.nfa.group_info()
    }
    #[cfg_attr(all(), inline(always))]
    fn create_cache(&self) -> Cache {
        Cache {
            capmatches: Captures::all(self.group_info().clone()),
            pikevm: self.pikevm.create_cache(),
            backtrack: self.backtrack.create_cache(),
            onepass: self.onepass.create_cache(),
            hybrid: self.hybrid.create_cache(),
            revhybrid: wrappers::ReverseHybridCache::none(),
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn reset_cache(&self, cache: &mut Cache) {
        cache.pikevm.reset(&self.pikevm);
        cache.backtrack.reset(&self.backtrack);
        cache.onepass.reset(&self.onepass);
        cache.hybrid.reset(&self.hybrid);
    }
    fn is_accelerated(&self) -> bool {
        self.pre.as_ref().map_or(false, |pre| pre.is_fast())
    }
    fn memory_usage(&self) -> usize {
        self.info.memory_usage()
            + self.pre.as_ref().map_or(0, |pre| pre.memory_usage())
            + self.nfa.memory_usage()
            + self.nfarev.as_ref().map_or(0, |nfa| nfa.memory_usage())
            + self.onepass.memory_usage()
            + self.dfa.memory_usage()
    }
    #[cfg_attr(all(), inline(always))]
    fn search(&self, cache: &mut Cache, input: &Input<'_>) -> Option<Match> {
        return if let Some(e) = self.dfa.get(input) {
            trace!("using full DFA for full search at {:?}", input.get_span());
            match e.try_search(input) {
                Ok(x) => x,
                Err(_err) => {
                    trace!("full DFA search failed: {_err}");
                    self.search_nofail(cache, input)
                }
            }
        } else if let Some(e) = self.hybrid.get(input) {
            trace!("using lazy DFA for full search at {:?}", input.get_span());
            match e.try_search(&mut cache.hybrid, input) {
                Ok(x) => x,
                Err(_err) => {
                    trace!("lazy DFA search failed: {_err}");
                    self.search_nofail(cache, input)
                }
            }
        } else {
            self.search_nofail(cache, input)
        };
    }
    #[cfg_attr(all(), inline(always))]
    fn search_half(&self, cache: &mut Cache, input: &Input<'_>) -> Option<HalfMatch> {
        if let Some(e) = self.dfa.get(input) {
            trace!("using full DFA for half search at {:?}", input.get_span());
            match e.try_search_half_fwd(input) {
                Ok(x) => x,
                Err(_err) => {
                    trace!("full DFA half search failed: {_err}");
                    self.search_half_nofail(cache, input)
                }
            }
        } else if let Some(e) = self.hybrid.get(input) {
            trace!("using lazy DFA for half search at {:?}", input.get_span());
            match e.try_search_half_fwd(&mut cache.hybrid, input) {
                Ok(x) => x,
                Err(_err) => {
                    trace!("lazy DFA half search failed: {_err}");
                    self.search_half_nofail(cache, input)
                }
            }
        } else {
            self.search_half_nofail(cache, input)
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn is_match(&self, cache: &mut Cache, input: &Input<'_>) -> bool {
        if let Some(e) = self.dfa.get(input) {
            trace!(
                "using full DFA for is-match search at {:?}",
                input.get_span()
            );
            match e.try_search_half_fwd(input) {
                Ok(x) => x.is_some(),
                Err(_err) => {
                    trace!("full DFA half search failed: {_err}");
                    self.is_match_nofail(cache, input)
                }
            }
        } else if let Some(e) = self.hybrid.get(input) {
            trace!(
                "using lazy DFA for is-match search at {:?}",
                input.get_span()
            );
            match e.try_search_half_fwd(&mut cache.hybrid, input) {
                Ok(x) => x.is_some(),
                Err(_err) => {
                    trace!("lazy DFA half search failed: {_err}");
                    self.is_match_nofail(cache, input)
                }
            }
        } else {
            self.is_match_nofail(cache, input)
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn search_slots(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        if !self.is_capture_search_needed(slots.len()) {
            trace!("asked for slots unnecessarily, trying fast path");
            let m = self.search(cache, input)?;
            copy_match_to_slots(m, slots);
            return Some(m.pattern());
        }
        if self.onepass.get(&input).is_some() {
            return self.search_slots_nofail(cache, &input, slots);
        }
        let m = match self.try_search_mayfail(cache, input) {
            Some(Ok(Some(m))) => m,
            Some(Ok(None)) => return None,
            Some(Err(_err)) => {
                trace!("fast capture search failed: {_err}");
                return self.search_slots_nofail(cache, input, slots);
            }
            None => {
                return self.search_slots_nofail(cache, input, slots);
            }
        };
        trace!(
            "match found at {}..{} in capture search, \
             using another engine to find captures",
            m.start(),
            m.end(),
        );
        let input = input
            .clone()
            .span(m.start()..m.end())
            .anchored(Anchored::Pattern(m.pattern()));
        Some(
            self.search_slots_nofail(cache, &input, slots)
                .expect("should find a match"),
        )
    }
    #[cfg_attr(all(), inline(always))]
    fn which_overlapping_matches(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) {
        if let Some(e) = self.dfa.get(input) {
            trace!(
                "using full DFA for overlapping search at {:?}",
                input.get_span()
            );
            let _err = match e.try_which_overlapping_matches(input, patset) {
                Ok(()) => return,
                Err(err) => err,
            };
            trace!("fast overlapping search failed: {_err}");
        } else if let Some(e) = self.hybrid.get(input) {
            trace!(
                "using lazy DFA for overlapping search at {:?}",
                input.get_span()
            );
            let _err = match e.try_which_overlapping_matches(&mut cache.hybrid, input, patset) {
                Ok(()) => {
                    return;
                }
                Err(err) => err,
            };
            trace!("fast overlapping search failed: {_err}");
        }
        trace!(
            "using PikeVM for overlapping search at {:?}",
            input.get_span()
        );
        let e = self.pikevm.get();
        e.which_overlapping_matches(&mut cache.pikevm, input, patset)
    }
}
#[derive(Debug)]
struct ReverseAnchored {
    core: Core,
}
impl ReverseAnchored {
    fn new(core: Core) -> Result<ReverseAnchored, Core> {
        if !core.info.is_always_anchored_end() {
            debug!(
                "skipping reverse anchored optimization because \
                 the regex is not always anchored at the end"
            );
            return Err(core);
        }
        if core.info.is_always_anchored_start() {
            debug!(
                "skipping reverse anchored optimization because \
                 the regex is also anchored at the start"
            );
            return Err(core);
        }
        if !core.hybrid.is_some() && !core.dfa.is_some() {
            debug!(
                "skipping reverse anchored optimization because \
                 we don't have a lazy DFA or a full DFA"
            );
            return Err(core);
        }
        Ok(ReverseAnchored { core })
    }
    #[cfg_attr(all(), inline(always))]
    fn try_search_half_anchored_rev(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, RetryFailError> {
        let input = input.clone().anchored(Anchored::Yes);
        if let Some(e) = self.core.dfa.get(&input) {
            trace!(
                "using full DFA for reverse anchored search at {:?}",
                input.get_span()
            );
            e.try_search_half_rev(&input)
        } else if let Some(e) = self.core.hybrid.get(&input) {
            trace!(
                "using lazy DFA for reverse anchored search at {:?}",
                input.get_span()
            );
            e.try_search_half_rev(&mut cache.hybrid, &input)
        } else {
            unreachable!("ReverseAnchored always has a DFA")
        }
    }
}
impl Strategy for ReverseAnchored {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("reverse anchored")
    }
    #[cfg_attr(all(), inline(always))]
    fn group_info(&self) -> &GroupInfo {
        self.core.group_info()
    }
    #[cfg_attr(all(), inline(always))]
    fn create_cache(&self) -> Cache {
        self.core.create_cache()
    }
    #[cfg_attr(all(), inline(always))]
    fn reset_cache(&self, cache: &mut Cache) {
        self.core.reset_cache(cache);
    }
    fn is_accelerated(&self) -> bool {
        true
    }
    fn memory_usage(&self) -> usize {
        self.core.memory_usage()
    }
    #[cfg_attr(all(), inline(always))]
    fn search(&self, cache: &mut Cache, input: &Input<'_>) -> Option<Match> {
        if input.get_anchored().is_anchored() {
            return self.core.search(cache, input);
        }
        match self.try_search_half_anchored_rev(cache, input) {
            Err(_err) => {
                trace!("fast reverse anchored search failed: {_err}");
                self.core.search_nofail(cache, input)
            }
            Ok(None) => None,
            Ok(Some(hm)) => Some(Match::new(hm.pattern(), hm.offset()..input.end())),
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn search_half(&self, cache: &mut Cache, input: &Input<'_>) -> Option<HalfMatch> {
        if input.get_anchored().is_anchored() {
            return self.core.search_half(cache, input);
        }
        match self.try_search_half_anchored_rev(cache, input) {
            Err(_err) => {
                trace!("fast reverse anchored search failed: {_err}");
                self.core.search_half_nofail(cache, input)
            }
            Ok(None) => None,
            Ok(Some(hm)) => Some(HalfMatch::new(hm.pattern(), input.end())),
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn is_match(&self, cache: &mut Cache, input: &Input<'_>) -> bool {
        if input.get_anchored().is_anchored() {
            return self.core.is_match(cache, input);
        }
        match self.try_search_half_anchored_rev(cache, input) {
            Err(_err) => {
                trace!("fast reverse anchored search failed: {_err}");
                self.core.is_match_nofail(cache, input)
            }
            Ok(None) => false,
            Ok(Some(_)) => true,
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn search_slots(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        if input.get_anchored().is_anchored() {
            return self.core.search_slots(cache, input, slots);
        }
        match self.try_search_half_anchored_rev(cache, input) {
            Err(_err) => {
                trace!("fast reverse anchored search failed: {_err}");
                self.core.search_slots_nofail(cache, input, slots)
            }
            Ok(None) => None,
            Ok(Some(hm)) => {
                if !self.core.is_capture_search_needed(slots.len()) {
                    trace!("asked for slots unnecessarily, skipping captures");
                    let m = Match::new(hm.pattern(), hm.offset()..input.end());
                    copy_match_to_slots(m, slots);
                    return Some(m.pattern());
                }
                let start = hm.offset();
                let input = input
                    .clone()
                    .span(start..input.end())
                    .anchored(Anchored::Pattern(hm.pattern()));
                self.core.search_slots_nofail(cache, &input, slots)
            }
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn which_overlapping_matches(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) {
        self.core.which_overlapping_matches(cache, input, patset)
    }
}
#[derive(Debug)]
struct ReverseSuffix {
    core: Core,
    pre: Prefilter,
}
impl ReverseSuffix {
    fn new(core: Core, hirs: &[&Hir]) -> Result<ReverseSuffix, Core> {
        if !core.info.config().get_auto_prefilter() {
            debug!(
                "skipping reverse suffix optimization because \
                 automatic prefilters are disabled"
            );
            return Err(core);
        }
        if core.info.config().get_match_kind() != MatchKind::LeftmostFirst {
            debug!(
                "skipping reverse suffix optimization because \
                 match kind is {:?} but this only supports leftmost-first",
                core.info.config().get_match_kind(),
            );
            return Err(core);
        }
        if core.info.is_always_anchored_start() {
            debug!(
                "skipping reverse suffix optimization because \
                 the regex is always anchored at the start",
            );
            return Err(core);
        }
        if !core.hybrid.is_some() && !core.dfa.is_some() {
            debug!(
                "skipping reverse suffix optimization because \
                 we don't have a lazy DFA or a full DFA"
            );
            return Err(core);
        }
        if core.pre.as_ref().map_or(false, |p| p.is_fast()) {
            debug!(
                "skipping reverse suffix optimization because \
                 we already have a prefilter that we think is fast"
            );
            return Err(core);
        }
        let kind = core.info.config().get_match_kind();
        let suffixes = crate::regex_engine::automata::util::prefilter::suffixes(kind, hirs);
        let lcs = match suffixes.longest_common_suffix() {
            None => {
                debug!(
                    "skipping reverse suffix optimization because \
                     a longest common suffix could not be found",
                );
                return Err(core);
            }
            Some(lcs) if lcs.is_empty() => {
                debug!(
                    "skipping reverse suffix optimization because \
                     the longest common suffix is the empty string",
                );
                return Err(core);
            }
            Some(lcs) => lcs,
        };
        let pre = match Prefilter::new(kind, &[lcs]) {
            Some(pre) => pre,
            None => {
                debug!(
                    "skipping reverse suffix optimization because \
                     a prefilter could not be constructed from the \
                     longest common suffix",
                );
                return Err(core);
            }
        };
        if !pre.is_fast() {
            debug!(
                "skipping reverse suffix optimization because \
                 while we have a suffix prefilter, it is not \
                 believed to be 'fast'"
            );
            return Err(core);
        }
        if !reverse_suffix::has_no_earlier_match(hirs, &lcs) {
            debug!(
                "skipping reverse suffix optimization because \
                 an earlier suffix match could be a complete match \
                 inside of a larger match"
            );
            return Err(core);
        }
        Ok(ReverseSuffix { core, pre })
    }
    #[cfg_attr(all(), inline(always))]
    fn try_search_half_start(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, RetryError> {
        let mut span = input.get_span();
        let mut min_start = 0;
        loop {
            let litmatch = match self.pre.find(input.haystack(), span) {
                None => break,
                Some(span) => span,
            };
            trace!("reverse suffix scan found suffix match at {litmatch:?}");
            let revinput = input
                .clone()
                .anchored(Anchored::Yes)
                .span(input.start()..litmatch.end);
            if let Some(hm) = self.try_search_half_rev_limited(cache, &revinput, min_start)? {
                return Ok(Some(hm));
            }
            if span.start >= span.end {
                break;
            }
            span.start = litmatch.start.checked_add(1).unwrap();
            min_start = litmatch.end;
        }
        Ok(None)
    }
    #[cfg_attr(all(), inline(always))]
    fn try_search_half_fwd(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, RetryFailError> {
        if let Some(e) = self.core.dfa.get(&input) {
            trace!(
                "using full DFA for forward reverse suffix search at {:?}",
                input.get_span()
            );
            e.try_search_half_fwd(&input)
        } else if let Some(e) = self.core.hybrid.get(&input) {
            trace!(
                "using lazy DFA for forward reverse suffix search at {:?}",
                input.get_span()
            );
            e.try_search_half_fwd(&mut cache.hybrid, &input)
        } else {
            unreachable!("ReverseSuffix always has a DFA")
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn try_search_half_rev_limited(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        min_start: usize,
    ) -> Result<Option<HalfMatch>, RetryError> {
        if let Some(e) = self.core.dfa.get(&input) {
            trace!(
                "using full DFA for reverse suffix search at {:?}, \
                 but will be stopped at {} to avoid quadratic behavior",
                input.get_span(),
                min_start,
            );
            e.try_search_half_rev_limited(&input, min_start)
        } else if let Some(e) = self.core.hybrid.get(&input) {
            trace!(
                "using lazy DFA for reverse suffix search at {:?}, \
                 but will be stopped at {} to avoid quadratic behavior",
                input.get_span(),
                min_start,
            );
            e.try_search_half_rev_limited(&mut cache.hybrid, &input, min_start)
        } else {
            unreachable!("ReverseSuffix always has a DFA")
        }
    }
}
impl Strategy for ReverseSuffix {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("reverse suffix")
    }
    #[cfg_attr(all(), inline(always))]
    fn group_info(&self) -> &GroupInfo {
        self.core.group_info()
    }
    #[cfg_attr(all(), inline(always))]
    fn create_cache(&self) -> Cache {
        self.core.create_cache()
    }
    #[cfg_attr(all(), inline(always))]
    fn reset_cache(&self, cache: &mut Cache) {
        self.core.reset_cache(cache);
    }
    fn is_accelerated(&self) -> bool {
        self.pre.is_fast()
    }
    fn memory_usage(&self) -> usize {
        self.core.memory_usage() + self.pre.memory_usage()
    }
    #[cfg_attr(all(), inline(always))]
    fn search(&self, cache: &mut Cache, input: &Input<'_>) -> Option<Match> {
        if input.get_anchored().is_anchored() {
            return self.core.search(cache, input);
        }
        match self.try_search_half_start(cache, input) {
            Err(RetryError::Quadratic(_err)) => {
                trace!("reverse suffix optimization failed: {_err}");
                self.core.search(cache, input)
            }
            Err(RetryError::Fail(_err)) => {
                trace!("reverse suffix reverse fast search failed: {_err}");
                self.core.search_nofail(cache, input)
            }
            Ok(None) => None,
            Ok(Some(hm_start)) => {
                let fwdinput = input
                    .clone()
                    .anchored(Anchored::Pattern(hm_start.pattern()))
                    .span(hm_start.offset()..input.end());
                match self.try_search_half_fwd(cache, &fwdinput) {
                    Err(_err) => {
                        trace!("reverse suffix forward fast search failed: {_err}");
                        self.core.search_nofail(cache, input)
                    }
                    Ok(None) => {
                        unreachable!(
                            "suffix match plus reverse match implies \
                             there must be a match",
                        )
                    }
                    Ok(Some(hm_end)) => Some(Match::new(
                        hm_start.pattern(),
                        hm_start.offset()..hm_end.offset(),
                    )),
                }
            }
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn search_half(&self, cache: &mut Cache, input: &Input<'_>) -> Option<HalfMatch> {
        if input.get_anchored().is_anchored() {
            return self.core.search_half(cache, input);
        }
        match self.try_search_half_start(cache, input) {
            Err(RetryError::Quadratic(_err)) => {
                trace!("reverse suffix half optimization failed: {_err}");
                self.core.search_half(cache, input)
            }
            Err(RetryError::Fail(_err)) => {
                trace!("reverse suffix reverse fast half search failed: {_err}");
                self.core.search_half_nofail(cache, input)
            }
            Ok(None) => None,
            Ok(Some(hm_start)) => {
                let fwdinput = input
                    .clone()
                    .anchored(Anchored::Pattern(hm_start.pattern()))
                    .span(hm_start.offset()..input.end());
                match self.try_search_half_fwd(cache, &fwdinput) {
                    Err(_err) => {
                        trace!("reverse suffix forward fast search failed: {_err}");
                        self.core.search_half_nofail(cache, input)
                    }
                    Ok(None) => {
                        unreachable!(
                            "suffix match plus reverse match implies \
                             there must be a match",
                        )
                    }
                    Ok(Some(hm_end)) => Some(hm_end),
                }
            }
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn is_match(&self, cache: &mut Cache, input: &Input<'_>) -> bool {
        if input.get_anchored().is_anchored() {
            return self.core.is_match(cache, input);
        }
        match self.try_search_half_start(cache, input) {
            Err(RetryError::Quadratic(_err)) => {
                trace!("reverse suffix half optimization failed: {_err}");
                self.core.is_match_nofail(cache, input)
            }
            Err(RetryError::Fail(_err)) => {
                trace!("reverse suffix reverse fast half search failed: {_err}");
                self.core.is_match_nofail(cache, input)
            }
            Ok(None) => false,
            Ok(Some(_)) => true,
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn search_slots(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        if input.get_anchored().is_anchored() {
            return self.core.search_slots(cache, input, slots);
        }
        if !self.core.is_capture_search_needed(slots.len()) {
            trace!("asked for slots unnecessarily, trying fast path");
            let m = self.search(cache, input)?;
            copy_match_to_slots(m, slots);
            return Some(m.pattern());
        }
        let hm_start = match self.try_search_half_start(cache, input) {
            Err(RetryError::Quadratic(_err)) => {
                trace!("reverse suffix captures optimization failed: {_err}");
                return self.core.search_slots(cache, input, slots);
            }
            Err(RetryError::Fail(_err)) => {
                trace!(
                    "reverse suffix reverse fast captures search failed: \
                     {_err}"
                );
                return self.core.search_slots_nofail(cache, input, slots);
            }
            Ok(None) => return None,
            Ok(Some(hm_start)) => hm_start,
        };
        trace!(
            "match found at {}..{} in capture search, \
             using another engine to find captures",
            hm_start.offset(),
            input.end(),
        );
        let start = hm_start.offset();
        let input = input
            .clone()
            .span(start..input.end())
            .anchored(Anchored::Pattern(hm_start.pattern()));
        self.core.search_slots_nofail(cache, &input, slots)
    }
    #[cfg_attr(all(), inline(always))]
    fn which_overlapping_matches(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) {
        self.core.which_overlapping_matches(cache, input, patset)
    }
}
#[derive(Debug)]
struct ReverseInner {
    core: Core,
    preinner: Prefilter,
    nfarev: NFA,
    hybrid: wrappers::ReverseHybrid,
    dfa: wrappers::ReverseDFA,
}
impl ReverseInner {
    fn new(core: Core, hirs: &[&Hir]) -> Result<ReverseInner, Core> {
        if !core.info.config().get_auto_prefilter() {
            debug!(
                "skipping reverse inner optimization because \
                 automatic prefilters are disabled"
            );
            return Err(core);
        }
        if core.info.config().get_match_kind() != MatchKind::LeftmostFirst {
            debug!(
                "skipping reverse inner optimization because \
                 match kind is {:?} but this only supports leftmost-first",
                core.info.config().get_match_kind(),
            );
            return Err(core);
        }
        if core.info.is_always_anchored_start() {
            debug!(
                "skipping reverse inner optimization because \
                 the regex is always anchored at the start",
            );
            return Err(core);
        }
        if !core.hybrid.is_some() && !core.dfa.is_some() {
            debug!(
                "skipping reverse inner optimization because \
                 we don't have a lazy DFA or a full DFA"
            );
            return Err(core);
        }
        if core.pre.as_ref().map_or(false, |p| p.is_fast()) {
            debug!(
                "skipping reverse inner optimization because \
                 we already have a prefilter that we think is fast"
            );
            return Err(core);
        } else if core.pre.is_some() {
            debug!(
                "core engine has a prefix prefilter, but it is \
                 probably not fast, so continuing with attempt to \
                 use reverse inner prefilter"
            );
        }
        let prefilter = match reverse_inner::InnerPrefilter::new(hirs) {
            Some(prefilter) => prefilter,
            None => return Err(core),
        };
        if !reverse_inner::has_no_earlier_match(&prefilter.prefix, &prefilter.literals) {
            debug!(
                "skipping reverse inner optimization because an inner \
                 literal match could be confirmed before an earlier match"
            );
            return Err(core);
        }
        debug!("building reverse NFA for prefix before inner literal");
        let thompson_config = core
            .info
            .config()
            .to_thompson_config()
            .reverse(true)
            .which_captures(WhichCaptures::None);
        let result = thompson::Compiler::new()
            .configure(thompson_config)
            .build_from_hir(&prefilter.prefix);
        let nfarev = match result {
            Ok(nfarev) => nfarev,
            Err(_err) => {
                debug!(
                    "skipping reverse inner optimization because the \
                     reverse NFA failed to build: {}",
                    _err,
                );
                return Err(core);
            }
        };
        debug!("building reverse DFA for prefix before inner literal");
        let dfa = if !core.info.config().get_dfa() {
            wrappers::ReverseDFA::none()
        } else {
            wrappers::ReverseDFA::new(&core.info, &nfarev)
        };
        let hybrid = if !core.info.config().get_hybrid() {
            wrappers::ReverseHybrid::none()
        } else if dfa.is_some() {
            debug!(
                "skipping lazy DFA for reverse inner optimization \
                 because we have a full DFA"
            );
            wrappers::ReverseHybrid::none()
        } else {
            wrappers::ReverseHybrid::new(&core.info, &nfarev)
        };
        Ok(ReverseInner {
            core,
            preinner: prefilter.pre,
            nfarev,
            hybrid,
            dfa,
        })
    }
    #[cfg_attr(all(), inline(always))]
    fn try_search_full(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<Option<Match>, RetryError> {
        let mut span = input.get_span();
        let mut min_match_start = 0;
        let mut min_pre_start = 0;
        loop {
            let litmatch = match self.preinner.find(input.haystack(), span) {
                None => break,
                Some(span) => span,
            };
            if litmatch.start < min_pre_start {
                trace!(
                    "found inner prefilter match at {litmatch:?}, which starts \
                     before the end of the last forward scan at {min_pre_start}, \
                     quitting to avoid quadratic behavior",
                );
                return Err(RetryError::Quadratic(RetryQuadraticError::new()));
            }
            trace!("reverse inner scan found inner match at {litmatch:?}");
            let revinput = input
                .clone()
                .anchored(Anchored::Yes)
                .span(input.start()..litmatch.start);
            if let Some(hm_start) =
                self.try_search_half_rev_limited(cache, &revinput, min_match_start)?
            {
                let fwdinput = input
                    .clone()
                    .anchored(Anchored::Pattern(hm_start.pattern()))
                    .span(hm_start.offset()..input.end());
                match self.try_search_half_fwd_stopat(cache, &fwdinput)? {
                    Err(stopat) => {
                        min_pre_start = stopat;
                        span.start = litmatch.start.checked_add(1).unwrap();
                    }
                    Ok(hm_end) => {
                        return Ok(Some(Match::new(
                            hm_start.pattern(),
                            hm_start.offset()..hm_end.offset(),
                        )));
                    }
                }
            }
            if span.start >= span.end {
                break;
            }
            span.start = litmatch.start.checked_add(1).unwrap();
            min_match_start = litmatch.end;
        }
        Ok(None)
    }
    #[cfg_attr(all(), inline(always))]
    fn try_search_half_fwd_stopat(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<Result<HalfMatch, usize>, RetryFailError> {
        if let Some(e) = self.core.dfa.get(&input) {
            trace!(
                "using full DFA for forward reverse inner search at {:?}",
                input.get_span()
            );
            e.try_search_half_fwd_stopat(&input)
        } else if let Some(e) = self.core.hybrid.get(&input) {
            trace!(
                "using lazy DFA for forward reverse inner search at {:?}",
                input.get_span()
            );
            e.try_search_half_fwd_stopat(&mut cache.hybrid, &input)
        } else {
            unreachable!("ReverseInner always has a DFA")
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn try_search_half_rev_limited(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        min_start: usize,
    ) -> Result<Option<HalfMatch>, RetryError> {
        if let Some(e) = self.dfa.get(&input) {
            trace!(
                "using full DFA for reverse inner search at {:?}, \
                 but will be stopped at {} to avoid quadratic behavior",
                input.get_span(),
                min_start,
            );
            e.try_search_half_rev_limited(&input, min_start)
        } else if let Some(e) = self.hybrid.get(&input) {
            trace!(
                "using lazy DFA for reverse inner search at {:?}, \
                 but will be stopped at {} to avoid quadratic behavior",
                input.get_span(),
                min_start,
            );
            e.try_search_half_rev_limited(&mut cache.revhybrid, &input, min_start)
        } else {
            unreachable!("ReverseInner always has a DFA")
        }
    }
}
impl Strategy for ReverseInner {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("reverse inner")
    }
    #[cfg_attr(all(), inline(always))]
    fn group_info(&self) -> &GroupInfo {
        self.core.group_info()
    }
    #[cfg_attr(all(), inline(always))]
    fn create_cache(&self) -> Cache {
        let mut cache = self.core.create_cache();
        cache.revhybrid = self.hybrid.create_cache();
        cache
    }
    #[cfg_attr(all(), inline(always))]
    fn reset_cache(&self, cache: &mut Cache) {
        self.core.reset_cache(cache);
        cache.revhybrid.reset(&self.hybrid);
    }
    fn is_accelerated(&self) -> bool {
        self.preinner.is_fast()
    }
    fn memory_usage(&self) -> usize {
        self.core.memory_usage()
            + self.preinner.memory_usage()
            + self.nfarev.memory_usage()
            + self.dfa.memory_usage()
    }
    #[cfg_attr(all(), inline(always))]
    fn search(&self, cache: &mut Cache, input: &Input<'_>) -> Option<Match> {
        if input.get_anchored().is_anchored() {
            return self.core.search(cache, input);
        }
        match self.try_search_full(cache, input) {
            Err(RetryError::Quadratic(_err)) => {
                trace!("reverse inner optimization failed: {_err}");
                self.core.search(cache, input)
            }
            Err(RetryError::Fail(_err)) => {
                trace!("reverse inner fast search failed: {_err}");
                self.core.search_nofail(cache, input)
            }
            Ok(matornot) => matornot,
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn search_half(&self, cache: &mut Cache, input: &Input<'_>) -> Option<HalfMatch> {
        if input.get_anchored().is_anchored() {
            return self.core.search_half(cache, input);
        }
        match self.try_search_full(cache, input) {
            Err(RetryError::Quadratic(_err)) => {
                trace!("reverse inner half optimization failed: {_err}");
                self.core.search_half(cache, input)
            }
            Err(RetryError::Fail(_err)) => {
                trace!("reverse inner fast half search failed: {_err}");
                self.core.search_half_nofail(cache, input)
            }
            Ok(None) => None,
            Ok(Some(m)) => Some(HalfMatch::new(m.pattern(), m.end())),
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn is_match(&self, cache: &mut Cache, input: &Input<'_>) -> bool {
        if input.get_anchored().is_anchored() {
            return self.core.is_match(cache, input);
        }
        match self.try_search_full(cache, input) {
            Err(RetryError::Quadratic(_err)) => {
                trace!("reverse inner half optimization failed: {_err}");
                self.core.is_match_nofail(cache, input)
            }
            Err(RetryError::Fail(_err)) => {
                trace!("reverse inner fast half search failed: {_err}");
                self.core.is_match_nofail(cache, input)
            }
            Ok(None) => false,
            Ok(Some(_)) => true,
        }
    }
    #[cfg_attr(all(), inline(always))]
    fn search_slots(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        slots: &mut [Option<NonMaxUsize>],
    ) -> Option<PatternID> {
        if input.get_anchored().is_anchored() {
            return self.core.search_slots(cache, input, slots);
        }
        if !self.core.is_capture_search_needed(slots.len()) {
            trace!("asked for slots unnecessarily, trying fast path");
            let m = self.search(cache, input)?;
            copy_match_to_slots(m, slots);
            return Some(m.pattern());
        }
        let m = match self.try_search_full(cache, input) {
            Err(RetryError::Quadratic(_err)) => {
                trace!("reverse inner captures optimization failed: {_err}");
                return self.core.search_slots(cache, input, slots);
            }
            Err(RetryError::Fail(_err)) => {
                trace!("reverse inner fast captures search failed: {_err}");
                return self.core.search_slots_nofail(cache, input, slots);
            }
            Ok(None) => return None,
            Ok(Some(m)) => m,
        };
        trace!(
            "match found at {}..{} in capture search, \
             using another engine to find captures",
            m.start(),
            m.end(),
        );
        let input = input
            .clone()
            .span(m.start()..m.end())
            .anchored(Anchored::Pattern(m.pattern()));
        self.core.search_slots_nofail(cache, &input, slots)
    }
    #[cfg_attr(all(), inline(always))]
    fn which_overlapping_matches(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) {
        self.core.which_overlapping_matches(cache, input, patset)
    }
}
#[cfg_attr(all(), inline(always))]
fn copy_match_to_slots(m: Match, slots: &mut [Option<NonMaxUsize>]) {
    let slot_start = m.pattern().as_usize() * 2;
    let slot_end = slot_start + 1;
    if let Some(slot) = slots.get_mut(slot_start) {
        *slot = NonMaxUsize::new(m.start());
    }
    if let Some(slot) = slots.get_mut(slot_end) {
        *slot = NonMaxUsize::new(m.end());
    }
}
