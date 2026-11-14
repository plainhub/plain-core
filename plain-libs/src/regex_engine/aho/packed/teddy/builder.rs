use crate::memchr::ext::Pointer;
use crate::regex_engine::aho::packed::{pattern::Patterns, teddy::generic::Match};
use core::{
    fmt::Debug,
    panic::{RefUnwindSafe, UnwindSafe},
};
use std::sync::Arc;
#[derive(Clone, Debug)]
pub(crate) struct Builder {
    only_fat: Option<bool>,
    only_256bit: Option<bool>,
    heuristic_pattern_limits: bool,
}
impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}
impl Builder {
    pub(crate) fn new() -> Builder {
        Builder {
            only_fat: None,
            only_256bit: None,
            heuristic_pattern_limits: true,
        }
    }
    pub(crate) fn build(&self, patterns: Arc<Patterns>) -> Option<Searcher> {
        self.build_imp(patterns)
    }
    pub(crate) fn only_fat(&mut self, yes: Option<bool>) -> &mut Builder {
        self.only_fat = yes;
        self
    }
    pub(crate) fn only_256bit(&mut self, yes: Option<bool>) -> &mut Builder {
        self.only_256bit = yes;
        self
    }
    pub(crate) fn heuristic_pattern_limits(&mut self, yes: bool) -> &mut Builder {
        self.heuristic_pattern_limits = yes;
        self
    }
    fn build_imp(&self, patterns: Arc<Patterns>) -> Option<Searcher> {
        let patlimit = self.heuristic_pattern_limits;
        if !cfg!(target_endian = "little") {
            debug!("skipping Teddy because target isn't little endian");
            return None;
        }
        if patlimit && patterns.len() > 64 {
            debug!("skipping Teddy because of too many patterns");
            return None;
        }
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            use self::x86_64::{FatAVX2, SlimAVX2, SlimSSSE3};
            let mask_len = core::cmp::min(4, patterns.minimum_len());
            let beefy = patterns.len() > 32;
            let has_avx2 = self::x86_64::is_available_avx2();
            let has_ssse3 = has_avx2 || self::x86_64::is_available_ssse3();
            let use_avx2 = if self.only_256bit == Some(true) {
                if !has_avx2 {
                    debug!("skipping Teddy because avx2 was demanded but unavailable");
                    return None;
                }
                true
            } else if self.only_256bit == Some(false) {
                if !has_ssse3 {
                    debug!("skipping Teddy because ssse3 was demanded but unavailable");
                    return None;
                }
                false
            } else if !has_ssse3 && !has_avx2 {
                debug!("skipping Teddy because ssse3 and avx2 are unavailable");
                return None;
            } else {
                has_avx2
            };
            let fat = match self.only_fat {
                None => use_avx2 && beefy,
                Some(false) => false,
                Some(true) if !use_avx2 => {
                    debug!(
                        "skipping Teddy because fat was demanded, but fat \
                         Teddy requires avx2 which is unavailable"
                    );
                    return None;
                }
                Some(true) => true,
            };
            if patlimit && mask_len == 1 && patterns.len() > 16 {
                debug!(
                    "skipping Teddy (mask len: 1) because there are \
                             too many patterns",
                );
                return None;
            }
            match (mask_len, use_avx2, fat) {
                (1, false, _) => {
                    debug!("Teddy choice: 128-bit slim, 1 byte");
                    SlimSSSE3::<1>::new(&patterns)
                }
                (1, true, false) => {
                    debug!("Teddy choice: 256-bit slim, 1 byte");
                    SlimAVX2::<1>::new(&patterns)
                }
                (1, true, true) => {
                    debug!("Teddy choice: 256-bit fat, 1 byte");
                    FatAVX2::<1>::new(&patterns)
                }
                (2, false, _) => {
                    debug!("Teddy choice: 128-bit slim, 2 bytes");
                    SlimSSSE3::<2>::new(&patterns)
                }
                (2, true, false) => {
                    debug!("Teddy choice: 256-bit slim, 2 bytes");
                    SlimAVX2::<2>::new(&patterns)
                }
                (2, true, true) => {
                    debug!("Teddy choice: 256-bit fat, 2 bytes");
                    FatAVX2::<2>::new(&patterns)
                }
                (3, false, _) => {
                    debug!("Teddy choice: 128-bit slim, 3 bytes");
                    SlimSSSE3::<3>::new(&patterns)
                }
                (3, true, false) => {
                    debug!("Teddy choice: 256-bit slim, 3 bytes");
                    SlimAVX2::<3>::new(&patterns)
                }
                (3, true, true) => {
                    debug!("Teddy choice: 256-bit fat, 3 bytes");
                    FatAVX2::<3>::new(&patterns)
                }
                (4, false, _) => {
                    debug!("Teddy choice: 128-bit slim, 4 bytes");
                    SlimSSSE3::<4>::new(&patterns)
                }
                (4, true, false) => {
                    debug!("Teddy choice: 256-bit slim, 4 bytes");
                    SlimAVX2::<4>::new(&patterns)
                }
                (4, true, true) => {
                    debug!("Teddy choice: 256-bit fat, 4 bytes");
                    FatAVX2::<4>::new(&patterns)
                }
                _ => {
                    debug!("no supported Teddy configuration found");
                    None
                }
            }
        }
        #[cfg(all(
            target_arch = "aarch64",
            target_feature = "neon",
            target_endian = "little"
        ))]
        {
            use self::aarch64::SlimNeon;
            let mask_len = core::cmp::min(4, patterns.minimum_len());
            if self.only_256bit == Some(true) {
                debug!(
                    "skipping Teddy because 256-bits were demanded \
                     but unavailable"
                );
                return None;
            }
            if self.only_fat == Some(true) {
                debug!("skipping Teddy because fat was demanded but unavailable");
            }
            match mask_len {
                1 => {
                    if patlimit && patterns.len() > 16 {
                        debug!(
                            "skipping Teddy (mask len: 1) because there are \
                             too many patterns",
                        );
                    }
                    debug!("Teddy choice: 128-bit slim, 1 byte");
                    SlimNeon::<1>::new(&patterns)
                }
                2 => {
                    if patlimit && patterns.len() > 32 {
                        debug!(
                            "skipping Teddy (mask len: 2) because there are \
                             too many patterns",
                        );
                    }
                    debug!("Teddy choice: 128-bit slim, 2 bytes");
                    SlimNeon::<2>::new(&patterns)
                }
                3 => {
                    if patlimit && patterns.len() > 48 {
                        debug!(
                            "skipping Teddy (mask len: 3) because there are \
                             too many patterns",
                        );
                    }
                    debug!("Teddy choice: 128-bit slim, 3 bytes");
                    SlimNeon::<3>::new(&patterns)
                }
                4 => {
                    debug!("Teddy choice: 128-bit slim, 4 bytes");
                    SlimNeon::<4>::new(&patterns)
                }
                _ => {
                    debug!("no supported Teddy configuration found");
                    None
                }
            }
        }
        #[cfg(not(any(
            all(target_arch = "x86_64", target_feature = "sse2"),
            all(
                target_arch = "aarch64",
                target_feature = "neon",
                target_endian = "little"
            )
        )))]
        {
            None
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Searcher {
    imp: Arc<dyn SearcherT>,
    memory_usage: usize,
    minimum_len: usize,
}
impl Searcher {
    #[inline(always)]
    pub(crate) fn find(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Option<crate::regex_engine::aho::Match> {
        assert!(haystack[at..].len() >= self.minimum_len);
        let hayptr = haystack.as_ptr();
        let teddym = unsafe { self.imp.find(hayptr.add(at), hayptr.add(haystack.len()))? };
        let start = teddym.start().as_usize().wrapping_sub(hayptr.as_usize());
        let end = teddym.end().as_usize().wrapping_sub(hayptr.as_usize());
        let span = crate::regex_engine::aho::Span { start, end };
        let pid = crate::regex_engine::aho::PatternID::new_unchecked(teddym.pattern().as_usize());
        let m = crate::regex_engine::aho::Match::new(pid, span);
        Some(m)
    }
    #[inline(always)]
    pub(crate) fn memory_usage(&self) -> usize {
        self.memory_usage
    }
    #[inline(always)]
    pub(crate) fn minimum_len(&self) -> usize {
        self.minimum_len
    }
}
trait SearcherT: Debug + Send + Sync + UnwindSafe + RefUnwindSafe + 'static {
    unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match>;
}
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod x86_64 {
    use super::{Searcher, SearcherT};
    use crate::memchr::ext::Pointer;
    use crate::regex_engine::aho::packed::{
        pattern::Patterns,
        teddy::generic::{self, Match},
    };
    use core::arch::x86_64::{__m128i, __m256i};
    use std::sync::Arc;
    #[derive(Clone, Debug)]
    pub(super) struct SlimSSSE3<const BYTES: usize> {
        slim128: generic::Slim<__m128i, BYTES>,
    }
    macro_rules! slim_ssse3 {
        ($ len : expr) => {
            impl SlimSSSE3<$len> {
                pub(super) fn new(patterns: &Arc<Patterns>) -> Option<Searcher> {
                    if !is_available_ssse3() {
                        return None;
                    }
                    Some(unsafe { SlimSSSE3::<$len>::new_unchecked(patterns) })
                }
                #[target_feature(enable = "ssse3")]
                unsafe fn new_unchecked(patterns: &Arc<Patterns>) -> Searcher {
                    let slim128 = generic::Slim::<__m128i, $len>::new(Arc::clone(patterns));
                    let memory_usage = slim128.memory_usage();
                    let minimum_len = slim128.minimum_len();
                    let imp = Arc::new(SlimSSSE3 { slim128 });
                    Searcher {
                        imp,
                        memory_usage,
                        minimum_len,
                    }
                }
            }
            impl SearcherT for SlimSSSE3<$len> {
                #[target_feature(enable = "ssse3")]
                #[inline]
                unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
                    self.slim128.find(start, end)
                }
            }
        };
    }
    slim_ssse3!(1);
    slim_ssse3!(2);
    slim_ssse3!(3);
    slim_ssse3!(4);
    #[derive(Clone, Debug)]
    pub(super) struct SlimAVX2<const BYTES: usize> {
        slim128: generic::Slim<__m128i, BYTES>,
        slim256: generic::Slim<__m256i, BYTES>,
    }
    macro_rules! slim_avx2 {
        ($ len : expr) => {
            impl SlimAVX2<$len> {
                pub(super) fn new(patterns: &Arc<Patterns>) -> Option<Searcher> {
                    if !is_available_avx2() {
                        return None;
                    }
                    Some(unsafe { SlimAVX2::<$len>::new_unchecked(patterns) })
                }
                #[target_feature(enable = "avx2")]
                unsafe fn new_unchecked(patterns: &Arc<Patterns>) -> Searcher {
                    let slim128 = generic::Slim::<__m128i, $len>::new(Arc::clone(&patterns));
                    let slim256 = generic::Slim::<__m256i, $len>::new(Arc::clone(&patterns));
                    let memory_usage = slim128.memory_usage() + slim256.memory_usage();
                    let minimum_len = slim128.minimum_len();
                    let imp = Arc::new(SlimAVX2 { slim128, slim256 });
                    Searcher {
                        imp,
                        memory_usage,
                        minimum_len,
                    }
                }
            }
            impl SearcherT for SlimAVX2<$len> {
                #[target_feature(enable = "avx2")]
                #[inline]
                unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
                    let len = end.distance(start);
                    if len < self.slim256.minimum_len() {
                        self.slim128.find(start, end)
                    } else {
                        self.slim256.find(start, end)
                    }
                }
            }
        };
    }
    slim_avx2!(1);
    slim_avx2!(2);
    slim_avx2!(3);
    slim_avx2!(4);
    #[derive(Clone, Debug)]
    pub(super) struct FatAVX2<const BYTES: usize> {
        fat256: generic::Fat<__m256i, BYTES>,
    }
    macro_rules! fat_avx2 {
        ($ len : expr) => {
            impl FatAVX2<$len> {
                pub(super) fn new(patterns: &Arc<Patterns>) -> Option<Searcher> {
                    if !is_available_avx2() {
                        return None;
                    }
                    Some(unsafe { FatAVX2::<$len>::new_unchecked(patterns) })
                }
                #[target_feature(enable = "avx2")]
                unsafe fn new_unchecked(patterns: &Arc<Patterns>) -> Searcher {
                    let fat256 = generic::Fat::<__m256i, $len>::new(Arc::clone(&patterns));
                    let memory_usage = fat256.memory_usage();
                    let minimum_len = fat256.minimum_len();
                    let imp = Arc::new(FatAVX2 { fat256 });
                    Searcher {
                        imp,
                        memory_usage,
                        minimum_len,
                    }
                }
            }
            impl SearcherT for FatAVX2<$len> {
                #[target_feature(enable = "avx2")]
                #[inline]
                unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
                    self.fat256.find(start, end)
                }
            }
        };
    }
    fat_avx2!(1);
    fat_avx2!(2);
    fat_avx2!(3);
    fat_avx2!(4);
    #[inline]
    pub(super) fn is_available_ssse3() -> bool {
        #[cfg(not(target_feature = "sse2"))]
        {
            false
        }
        #[cfg(target_feature = "sse2")]
        {
            #[cfg(target_feature = "ssse3")]
            {
                true
            }
            #[cfg(not(target_feature = "ssse3"))]
            {
                { std::is_x86_feature_detected!("ssse3") }
            }
        }
    }
    #[inline]
    pub(super) fn is_available_avx2() -> bool {
        #[cfg(not(target_feature = "sse2"))]
        {
            false
        }
        #[cfg(target_feature = "sse2")]
        {
            #[cfg(target_feature = "avx2")]
            {
                true
            }
            #[cfg(not(target_feature = "avx2"))]
            {
                { std::is_x86_feature_detected!("avx2") }
            }
        }
    }
}
#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
mod aarch64 {
    use super::{Searcher, SearcherT};
    use crate::regex_engine::aho::packed::{
        pattern::Patterns,
        teddy::generic::{self, Match},
    };
    use core::arch::aarch64::uint8x16_t;
    use std::sync::Arc;
    #[derive(Clone, Debug)]
    pub(super) struct SlimNeon<const BYTES: usize> {
        slim128: generic::Slim<uint8x16_t, BYTES>,
    }
    macro_rules! slim_neon {
        ($ len : expr) => {
            impl SlimNeon<$len> {
                pub(super) fn new(patterns: &Arc<Patterns>) -> Option<Searcher> {
                    Some(unsafe { SlimNeon::<$len>::new_unchecked(patterns) })
                }
                #[target_feature(enable = "neon")]
                unsafe fn new_unchecked(patterns: &Arc<Patterns>) -> Searcher {
                    unsafe {
                        let slim128 = generic::Slim::<uint8x16_t, $len>::new(Arc::clone(patterns));
                        let memory_usage = slim128.memory_usage();
                        let minimum_len = slim128.minimum_len();
                        let imp = Arc::new(SlimNeon { slim128 });
                        Searcher {
                            imp,
                            memory_usage,
                            minimum_len,
                        }
                    }
                }
            }
            impl SearcherT for SlimNeon<$len> {
                #[target_feature(enable = "neon")]
                #[inline]
                unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
                    unsafe { self.slim128.find(start, end) }
                }
            }
        };
    }
    slim_neon!(1);
    slim_neon!(2);
    slim_neon!(3);
    slim_neon!(4);
}
