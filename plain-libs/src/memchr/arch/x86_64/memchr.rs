macro_rules! unsafe_ifunc { ($memchrty : ident , $memchrfind : ident , $fnty : ty , $retty : ty , $hay_start : ident , $hay_end : ident , $($needle : ident) ,+) => { {  use core::sync::atomic::{ AtomicPtr , Ordering } ; type Fn = * mut () ; type RealFn = $fnty ; static FN : AtomicPtr < () > = AtomicPtr::new (detect as Fn) ; #[cfg (target_feature = "sse2")] #[target_feature (enable = "sse2" , enable = "avx2")] unsafe fn find_avx2 ($($needle : u8) ,+, $hay_start : * const u8 , $hay_end : * const u8 ,) -> $retty { use crate::memchr::arch::x86_64::avx2::memchr::$memchrty ; $memchrty::new_unchecked ($($needle) ,+) .$memchrfind ($hay_start , $hay_end) } #[cfg (target_feature = "sse2")] #[target_feature (enable = "sse2")] unsafe fn find_sse2 ($($needle : u8) ,+, $hay_start : * const u8 , $hay_end : * const u8 ,) -> $retty { use crate::memchr::arch::x86_64::sse2::memchr::$memchrty ; $memchrty::new_unchecked ($($needle) ,+) .$memchrfind ($hay_start , $hay_end) } unsafe fn find_fallback ($($needle : u8) ,+, $hay_start : * const u8 , $hay_end : * const u8 ,) -> $retty { use crate::memchr::arch::all::memchr::$memchrty ; $memchrty::new ($($needle) ,+) .$memchrfind ($hay_start , $hay_end) } unsafe fn detect ($($needle : u8) ,+, $hay_start : * const u8 , $hay_end : * const u8 ,) -> $retty { let fun = { #[cfg (not (target_feature = "sse2"))] { debug ! ("no sse2 feature available, using fallback for {}" , stringify ! ($memchrty) ,) ; find_fallback as RealFn } #[cfg (target_feature = "sse2")] { use crate::memchr::arch::x86_64::{ sse2 , avx2 } ; if avx2::memchr::$memchrty::is_available () { debug ! ("chose AVX2 for {}" , stringify ! ($memchrty)) ; find_avx2 as RealFn } else if sse2::memchr::$memchrty::is_available () { debug ! ("chose SSE2 for {}" , stringify ! ($memchrty)) ; find_sse2 as RealFn } else { debug ! ("chose fallback for {}" , stringify ! ($memchrty)) ; find_fallback as RealFn } } } ; FN . store (fun as Fn , Ordering::Relaxed) ; fun ($($needle) ,+, $hay_start , $hay_end) } unsafe { let fun = FN . load (Ordering::Relaxed) ; core::mem::transmute::< Fn , RealFn > (fun) ($($needle) ,+, $hay_start , $hay_end ,) } } } ; }
#[inline(always)]
pub(crate) unsafe fn memchr_raw(n1: u8, start: *const u8, end: *const u8) -> Option<*const u8> {
    unsafe {
        unsafe_ifunc!(
            One,
            find_raw,
            unsafe fn(u8, *const u8, *const u8) -> Option<*const u8>,
            Option<*const u8>,
            start,
            end,
            n1
        )
    }
}
#[inline(always)]
pub(crate) unsafe fn memrchr_raw(n1: u8, start: *const u8, end: *const u8) -> Option<*const u8> {
    unsafe {
        unsafe_ifunc!(
            One,
            rfind_raw,
            unsafe fn(u8, *const u8, *const u8) -> Option<*const u8>,
            Option<*const u8>,
            start,
            end,
            n1
        )
    }
}
#[inline(always)]
pub(crate) unsafe fn memchr2_raw(
    n1: u8,
    n2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe {
        unsafe_ifunc!(
            Two,
            find_raw,
            unsafe fn(u8, u8, *const u8, *const u8) -> Option<*const u8>,
            Option<*const u8>,
            start,
            end,
            n1,
            n2
        )
    }
}
#[inline(always)]
pub(crate) unsafe fn memrchr2_raw(
    n1: u8,
    n2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe {
        unsafe_ifunc!(
            Two,
            rfind_raw,
            unsafe fn(u8, u8, *const u8, *const u8) -> Option<*const u8>,
            Option<*const u8>,
            start,
            end,
            n1,
            n2
        )
    }
}
#[inline(always)]
pub(crate) unsafe fn memchr3_raw(
    n1: u8,
    n2: u8,
    n3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe {
        unsafe_ifunc!(
            Three,
            find_raw,
            unsafe fn(u8, u8, u8, *const u8, *const u8) -> Option<*const u8>,
            Option<*const u8>,
            start,
            end,
            n1,
            n2,
            n3
        )
    }
}
#[inline(always)]
pub(crate) unsafe fn memrchr3_raw(
    n1: u8,
    n2: u8,
    n3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe {
        unsafe_ifunc!(
            Three,
            rfind_raw,
            unsafe fn(u8, u8, u8, *const u8, *const u8) -> Option<*const u8>,
            Option<*const u8>,
            start,
            end,
            n1,
            n2,
            n3
        )
    }
}
#[inline(always)]
pub(crate) unsafe fn count_raw(n1: u8, start: *const u8, end: *const u8) -> usize {
    unsafe {
        unsafe_ifunc!(
            One,
            count_raw,
            unsafe fn(u8, *const u8, *const u8) -> usize,
            usize,
            start,
            end,
            n1
        )
    }
}
