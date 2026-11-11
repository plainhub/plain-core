#[cfg(target_arch = "aarch64")]
use crate::memchr::arch::aarch64::neon::packedpair as neon;
use crate::memchr::arch::all::{
    packedpair::{HeuristicFrequencyRank, Pair},
    rabinkarp, twoway,
};
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use crate::memchr::arch::wasm32::simd128::packedpair as simd128;
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
use crate::memchr::arch::x86_64::{avx2::packedpair as avx2, sse2::packedpair as sse2};
#[derive(Clone)]
pub(crate) struct Searcher {
    call: SearcherKindFn,
    kind: SearcherKind,
    rabinkarp: rabinkarp::Finder,
}
impl Searcher {
    #[inline]
    pub(crate) fn new<R: HeuristicFrequencyRank>(
        prefilter: PrefilterConfig,
        ranker: R,
        needle: &[u8],
    ) -> Searcher {
        let rabinkarp = rabinkarp::Finder::new(needle);
        if needle.len() <= 1 {
            return if needle.is_empty() {
                trace!("building empty substring searcher");
                Searcher {
                    call: searcher_kind_empty,
                    kind: SearcherKind { empty: () },
                    rabinkarp,
                }
            } else {
                trace!("building one-byte substring searcher");
                debug_assert_eq!(1, needle.len());
                Searcher {
                    call: searcher_kind_one_byte,
                    kind: SearcherKind {
                        one_byte: needle[0],
                    },
                    rabinkarp,
                }
            };
        }
        let pair = match Pair::with_ranker(needle, &ranker) {
            Some(pair) => pair,
            None => return Searcher::twoway(needle, rabinkarp, None),
        };
        debug_assert_ne!(
            pair.index1(),
            pair.index2(),
            "pair offsets should not be equivalent"
        );
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            if let Some(pp) = avx2::Finder::with_pair(needle, pair) {
                if do_packed_search(needle) {
                    trace!("building x86_64 AVX2 substring searcher");
                    let kind = SearcherKind { avx2: pp };
                    Searcher {
                        call: searcher_kind_avx2,
                        kind,
                        rabinkarp,
                    }
                } else if prefilter.is_none() {
                    Searcher::twoway(needle, rabinkarp, None)
                } else {
                    let prestrat = Prefilter::avx2(pp, needle);
                    Searcher::twoway(needle, rabinkarp, Some(prestrat))
                }
            } else if let Some(pp) = sse2::Finder::with_pair(needle, pair) {
                if do_packed_search(needle) {
                    trace!("building x86_64 SSE2 substring searcher");
                    let kind = SearcherKind { sse2: pp };
                    Searcher {
                        call: searcher_kind_sse2,
                        kind,
                        rabinkarp,
                    }
                } else if prefilter.is_none() {
                    Searcher::twoway(needle, rabinkarp, None)
                } else {
                    let prestrat = Prefilter::sse2(pp, needle);
                    Searcher::twoway(needle, rabinkarp, Some(prestrat))
                }
            } else if prefilter.is_none() {
                Searcher::twoway(needle, rabinkarp, None)
            } else {
                let prestrat = Prefilter::fallback(ranker, pair, needle);
                Searcher::twoway(needle, rabinkarp, prestrat)
            }
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            if let Some(pp) = simd128::Finder::with_pair(needle, pair) {
                if do_packed_search(needle) {
                    trace!("building wasm32 simd128 substring searcher");
                    let kind = SearcherKind { simd128: pp };
                    Searcher {
                        call: searcher_kind_simd128,
                        kind,
                        rabinkarp,
                    }
                } else if prefilter.is_none() {
                    Searcher::twoway(needle, rabinkarp, None)
                } else {
                    let prestrat = Prefilter::simd128(pp, needle);
                    Searcher::twoway(needle, rabinkarp, Some(prestrat))
                }
            } else if prefilter.is_none() {
                Searcher::twoway(needle, rabinkarp, None)
            } else {
                let prestrat = Prefilter::fallback(ranker, pair, needle);
                Searcher::twoway(needle, rabinkarp, prestrat)
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            if let Some(pp) = neon::Finder::with_pair(needle, pair) {
                if do_packed_search(needle) {
                    trace!("building aarch64 neon substring searcher");
                    let kind = SearcherKind { neon: pp };
                    Searcher {
                        call: searcher_kind_neon,
                        kind,
                        rabinkarp,
                    }
                } else if prefilter.is_none() {
                    Searcher::twoway(needle, rabinkarp, None)
                } else {
                    let prestrat = Prefilter::neon(pp, needle);
                    Searcher::twoway(needle, rabinkarp, Some(prestrat))
                }
            } else if prefilter.is_none() {
                Searcher::twoway(needle, rabinkarp, None)
            } else {
                let prestrat = Prefilter::fallback(ranker, pair, needle);
                Searcher::twoway(needle, rabinkarp, prestrat)
            }
        }
        #[cfg(not(any(
            all(target_arch = "x86_64", target_feature = "sse2"),
            all(target_arch = "wasm32", target_feature = "simd128"),
            target_arch = "aarch64"
        )))]
        {
            if prefilter.is_none() {
                Searcher::twoway(needle, rabinkarp, None)
            } else {
                let prestrat = Prefilter::fallback(ranker, pair, needle);
                Searcher::twoway(needle, rabinkarp, prestrat)
            }
        }
    }
    #[inline]
    fn twoway(
        needle: &[u8],
        rabinkarp: rabinkarp::Finder,
        prestrat: Option<Prefilter>,
    ) -> Searcher {
        let finder = twoway::Finder::new(needle);
        match prestrat {
            None => {
                trace!("building scalar two-way substring searcher");
                let kind = SearcherKind { two_way: finder };
                Searcher {
                    call: searcher_kind_two_way,
                    kind,
                    rabinkarp,
                }
            }
            Some(prestrat) => {
                trace!(
                    "building scalar two-way \
                     substring searcher with a prefilter"
                );
                let two_way_with_prefilter = TwoWayWithPrefilter { finder, prestrat };
                let kind = SearcherKind {
                    two_way_with_prefilter,
                };
                Searcher {
                    call: searcher_kind_two_way_with_prefilter,
                    kind,
                    rabinkarp,
                }
            }
        }
    }
    #[inline(always)]
    pub(crate) fn find(
        &self,
        prestate: &mut PrefilterState,
        haystack: &[u8],
        needle: &[u8],
    ) -> Option<usize> {
        if haystack.len() < needle.len() {
            None
        } else {
            unsafe { (self.call)(self, prestate, haystack, needle) }
        }
    }
}
impl core::fmt::Debug for Searcher {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Searcher")
            .field("call", &"<searcher function>")
            .field("kind", &"<searcher kind union>")
            .field("rabinkarp", &self.rabinkarp)
            .finish()
    }
}
#[derive(Clone, Copy)]
union SearcherKind {
    empty: (),
    one_byte: u8,
    two_way: twoway::Finder,
    two_way_with_prefilter: TwoWayWithPrefilter,
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    sse2: crate::memchr::arch::x86_64::sse2::packedpair::Finder,
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    avx2: crate::memchr::arch::x86_64::avx2::packedpair::Finder,
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    simd128: crate::memchr::arch::wasm32::simd128::packedpair::Finder,
    #[cfg(target_arch = "aarch64")]
    neon: crate::memchr::arch::aarch64::neon::packedpair::Finder,
}
#[derive(Copy, Clone, Debug)]
struct TwoWayWithPrefilter {
    finder: twoway::Finder,
    prestrat: Prefilter,
}
type SearcherKindFn = unsafe fn(
    searcher: &Searcher,
    prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize>;
unsafe fn searcher_kind_empty(
    _searcher: &Searcher,
    _prestate: &mut PrefilterState,
    _haystack: &[u8],
    _needle: &[u8],
) -> Option<usize> {
    unsafe { Some(0) }
}
unsafe fn searcher_kind_one_byte(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    _needle: &[u8],
) -> Option<usize> {
    unsafe {
        let needle = searcher.kind.one_byte;
        crate::memchr::memchr(needle, haystack)
    }
}
unsafe fn searcher_kind_two_way(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    unsafe {
        if rabinkarp::is_fast(haystack, needle) {
            searcher.rabinkarp.find(haystack, needle)
        } else {
            searcher.kind.two_way.find(haystack, needle)
        }
    }
}
unsafe fn searcher_kind_two_way_with_prefilter(
    searcher: &Searcher,
    prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    unsafe {
        if rabinkarp::is_fast(haystack, needle) {
            searcher.rabinkarp.find(haystack, needle)
        } else {
            let TwoWayWithPrefilter {
                ref finder,
                ref prestrat,
            } = searcher.kind.two_way_with_prefilter;
            let pre = Pre { prestate, prestrat };
            finder.find_with_prefilter(Some(pre), haystack, needle)
        }
    }
}
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
unsafe fn searcher_kind_sse2(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    unsafe {
        let finder = &searcher.kind.sse2;
        if haystack.len() < finder.min_haystack_len() {
            searcher.rabinkarp.find(haystack, needle)
        } else {
            finder.find(haystack, needle)
        }
    }
}
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
unsafe fn searcher_kind_avx2(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    unsafe {
        let finder = &searcher.kind.avx2;
        if haystack.len() < finder.min_haystack_len() {
            searcher.rabinkarp.find(haystack, needle)
        } else {
            finder.find(haystack, needle)
        }
    }
}
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
unsafe fn searcher_kind_simd128(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    unsafe {
        let finder = &searcher.kind.simd128;
        if haystack.len() < finder.min_haystack_len() {
            searcher.rabinkarp.find(haystack, needle)
        } else {
            finder.find(haystack, needle)
        }
    }
}
#[cfg(target_arch = "aarch64")]
unsafe fn searcher_kind_neon(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    unsafe {
        let finder = &searcher.kind.neon;
        if haystack.len() < finder.min_haystack_len() {
            searcher.rabinkarp.find(haystack, needle)
        } else {
            finder.find(haystack, needle)
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct SearcherRev {
    kind: SearcherRevKind,
    rabinkarp: rabinkarp::FinderRev,
}
#[derive(Clone, Debug)]
enum SearcherRevKind {
    Empty,
    OneByte { needle: u8 },
    TwoWay { finder: twoway::FinderRev },
}
impl SearcherRev {
    #[inline]
    pub(crate) fn new(needle: &[u8]) -> SearcherRev {
        let kind = if needle.len() <= 1 {
            if needle.is_empty() {
                trace!("building empty reverse substring searcher");
                SearcherRevKind::Empty
            } else {
                trace!("building one-byte reverse substring searcher");
                debug_assert_eq!(1, needle.len());
                SearcherRevKind::OneByte { needle: needle[0] }
            }
        } else {
            trace!("building scalar two-way reverse substring searcher");
            let finder = twoway::FinderRev::new(needle);
            SearcherRevKind::TwoWay { finder }
        };
        let rabinkarp = rabinkarp::FinderRev::new(needle);
        SearcherRev { kind, rabinkarp }
    }
    #[inline]
    pub(crate) fn rfind(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        if haystack.len() < needle.len() {
            return None;
        }
        match self.kind {
            SearcherRevKind::Empty => Some(haystack.len()),
            SearcherRevKind::OneByte { needle } => crate::memchr::memrchr(needle, haystack),
            SearcherRevKind::TwoWay { ref finder } => {
                if rabinkarp::is_fast(haystack, needle) {
                    self.rabinkarp.rfind(haystack, needle)
                } else {
                    finder.rfind(haystack, needle)
                }
            }
        }
    }
}
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum PrefilterConfig {
    None,
    Auto,
}
impl Default for PrefilterConfig {
    fn default() -> PrefilterConfig {
        PrefilterConfig::Auto
    }
}
impl PrefilterConfig {
    fn is_none(&self) -> bool {
        matches!(*self, PrefilterConfig::None)
    }
}
#[derive(Clone, Copy)]
struct Prefilter {
    call: PrefilterKindFn,
    kind: PrefilterKind,
    rarest_byte: u8,
    rarest_offset: u8,
}
impl Prefilter {
    #[inline]
    fn fallback<R: HeuristicFrequencyRank>(
        ranker: R,
        pair: Pair,
        needle: &[u8],
    ) -> Option<Prefilter> {
        const MAX_FALLBACK_RANK: u8 = 250;
        trace!("building fallback prefilter");
        let rarest_offset = pair.index1();
        let rarest_byte = needle[usize::from(rarest_offset)];
        let rarest_rank = ranker.rank(rarest_byte);
        if rarest_rank > MAX_FALLBACK_RANK {
            None
        } else {
            let finder = crate::memchr::arch::all::packedpair::Finder::with_pair(needle, pair)?;
            let call = prefilter_kind_fallback;
            let kind = PrefilterKind { fallback: finder };
            Some(Prefilter {
                call,
                kind,
                rarest_byte,
                rarest_offset,
            })
        }
    }
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[inline]
    fn sse2(finder: sse2::Finder, needle: &[u8]) -> Prefilter {
        trace!("building x86_64 SSE2 prefilter");
        let rarest_offset = finder.pair().index1();
        let rarest_byte = needle[usize::from(rarest_offset)];
        Prefilter {
            call: prefilter_kind_sse2,
            kind: PrefilterKind { sse2: finder },
            rarest_byte,
            rarest_offset,
        }
    }
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[inline]
    fn avx2(finder: avx2::Finder, needle: &[u8]) -> Prefilter {
        trace!("building x86_64 AVX2 prefilter");
        let rarest_offset = finder.pair().index1();
        let rarest_byte = needle[usize::from(rarest_offset)];
        Prefilter {
            call: prefilter_kind_avx2,
            kind: PrefilterKind { avx2: finder },
            rarest_byte,
            rarest_offset,
        }
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    #[inline]
    fn simd128(finder: simd128::Finder, needle: &[u8]) -> Prefilter {
        trace!("building wasm32 simd128 prefilter");
        let rarest_offset = finder.pair().index1();
        let rarest_byte = needle[usize::from(rarest_offset)];
        Prefilter {
            call: prefilter_kind_simd128,
            kind: PrefilterKind { simd128: finder },
            rarest_byte,
            rarest_offset,
        }
    }
    #[cfg(target_arch = "aarch64")]
    #[inline]
    fn neon(finder: neon::Finder, needle: &[u8]) -> Prefilter {
        trace!("building aarch64 neon prefilter");
        let rarest_offset = finder.pair().index1();
        let rarest_byte = needle[usize::from(rarest_offset)];
        Prefilter {
            call: prefilter_kind_neon,
            kind: PrefilterKind { neon: finder },
            rarest_byte,
            rarest_offset,
        }
    }
    #[inline]
    fn find(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { (self.call)(self, haystack) }
    }
    #[inline]
    fn find_simple(&self, haystack: &[u8]) -> Option<usize> {
        crate::memchr::arch::all::memchr::One::new(self.rarest_byte)
            .find(haystack)
            .map(|i| i.saturating_sub(usize::from(self.rarest_offset)))
    }
}
impl core::fmt::Debug for Prefilter {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Prefilter")
            .field("call", &"<prefilter function>")
            .field("kind", &"<prefilter kind union>")
            .field("rarest_byte", &self.rarest_byte)
            .field("rarest_offset", &self.rarest_offset)
            .finish()
    }
}
#[derive(Clone, Copy)]
union PrefilterKind {
    fallback: crate::memchr::arch::all::packedpair::Finder,
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    sse2: crate::memchr::arch::x86_64::sse2::packedpair::Finder,
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    avx2: crate::memchr::arch::x86_64::avx2::packedpair::Finder,
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    simd128: crate::memchr::arch::wasm32::simd128::packedpair::Finder,
    #[cfg(target_arch = "aarch64")]
    neon: crate::memchr::arch::aarch64::neon::packedpair::Finder,
}
type PrefilterKindFn = unsafe fn(strat: &Prefilter, haystack: &[u8]) -> Option<usize>;
unsafe fn prefilter_kind_fallback(strat: &Prefilter, haystack: &[u8]) -> Option<usize> {
    unsafe { strat.kind.fallback.find_prefilter(haystack) }
}
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
unsafe fn prefilter_kind_sse2(strat: &Prefilter, haystack: &[u8]) -> Option<usize> {
    unsafe {
        let finder = &strat.kind.sse2;
        if haystack.len() < finder.min_haystack_len() {
            strat.find_simple(haystack)
        } else {
            finder.find_prefilter(haystack)
        }
    }
}
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
unsafe fn prefilter_kind_avx2(strat: &Prefilter, haystack: &[u8]) -> Option<usize> {
    unsafe {
        let finder = &strat.kind.avx2;
        if haystack.len() < finder.min_haystack_len() {
            strat.find_simple(haystack)
        } else {
            finder.find_prefilter(haystack)
        }
    }
}
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
unsafe fn prefilter_kind_simd128(strat: &Prefilter, haystack: &[u8]) -> Option<usize> {
    unsafe {
        let finder = &strat.kind.simd128;
        if haystack.len() < finder.min_haystack_len() {
            strat.find_simple(haystack)
        } else {
            finder.find_prefilter(haystack)
        }
    }
}
#[cfg(target_arch = "aarch64")]
unsafe fn prefilter_kind_neon(strat: &Prefilter, haystack: &[u8]) -> Option<usize> {
    unsafe {
        let finder = &strat.kind.neon;
        if haystack.len() < finder.min_haystack_len() {
            strat.find_simple(haystack)
        } else {
            finder.find_prefilter(haystack)
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct PrefilterState {
    skips: u32,
    skipped: u32,
}
impl PrefilterState {
    const MIN_SKIPS: u32 = 50;
    const MIN_SKIP_BYTES: u32 = 8;
    #[inline]
    pub(crate) fn new() -> PrefilterState {
        PrefilterState {
            skips: 1,
            skipped: 0,
        }
    }
    #[inline]
    fn update(&mut self, skipped: usize) {
        self.skips = self.skips.saturating_add(1);
        self.skipped = match u32::try_from(skipped) {
            Err(_) => u32::MAX,
            Ok(skipped) => self.skipped.saturating_add(skipped),
        };
    }
    #[inline]
    fn is_effective(&mut self) -> bool {
        if self.is_inert() {
            return false;
        }
        if self.skips() < PrefilterState::MIN_SKIPS {
            return true;
        }
        if self.skipped >= PrefilterState::MIN_SKIP_BYTES * self.skips() {
            return true;
        }
        self.skips = 0;
        false
    }
    #[inline]
    fn is_inert(&self) -> bool {
        self.skips == 0
    }
    #[inline]
    fn skips(&self) -> u32 {
        self.skips.saturating_sub(1)
    }
}
#[derive(Debug)]
pub(crate) struct Pre<'a> {
    prestate: &'a mut PrefilterState,
    prestrat: &'a Prefilter,
}
impl<'a> Pre<'a> {
    #[inline]
    pub(crate) fn find(&mut self, haystack: &[u8]) -> Option<usize> {
        let result = self.prestrat.find(haystack);
        self.prestate.update(result.unwrap_or(haystack.len()));
        result
    }
    #[inline]
    pub(crate) fn is_effective(&mut self) -> bool {
        self.prestate.is_effective()
    }
}
#[inline]
fn do_packed_search(needle: &[u8]) -> bool {
    const MIN_LEN: usize = 2;
    const MAX_LEN: usize = 32;
    MIN_LEN <= needle.len() && needle.len() <= MAX_LEN
}
