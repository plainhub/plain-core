macro_rules! defraw { ($ty : ident , $find : ident , $start : ident , $end : ident , $($needles : ident) ,+) => { { #[cfg (target_feature = "neon")] { use crate::memchr::arch::aarch64::neon::memchr::$ty ; debug ! ("chose neon for {}" , stringify ! ($ty)) ; debug_assert ! ($ty::is_available ()) ; $ty::new_unchecked ($($needles) ,+) .$find ($start , $end) } #[cfg (not (target_feature = "neon"))] { use crate::memchr::arch::all::memchr::$ty ; debug ! ("no neon feature available, using fallback for {}" , stringify ! ($ty) ,) ; $ty::new ($($needles) ,+) .$find ($start , $end) } } } }
#[inline(always)]
pub(crate) unsafe fn memchr_raw(n1: u8, start: *const u8, end: *const u8) -> Option<*const u8> {
    unsafe { defraw!(One, find_raw, start, end, n1) }
}
#[inline(always)]
pub(crate) unsafe fn memrchr_raw(n1: u8, start: *const u8, end: *const u8) -> Option<*const u8> {
    unsafe { defraw!(One, rfind_raw, start, end, n1) }
}
#[inline(always)]
pub(crate) unsafe fn memchr2_raw(
    n1: u8,
    n2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe { defraw!(Two, find_raw, start, end, n1, n2) }
}
#[inline(always)]
pub(crate) unsafe fn memrchr2_raw(
    n1: u8,
    n2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe { defraw!(Two, rfind_raw, start, end, n1, n2) }
}
#[inline(always)]
pub(crate) unsafe fn memchr3_raw(
    n1: u8,
    n2: u8,
    n3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe { defraw!(Three, find_raw, start, end, n1, n2, n3) }
}
#[inline(always)]
pub(crate) unsafe fn memrchr3_raw(
    n1: u8,
    n2: u8,
    n3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe { defraw!(Three, rfind_raw, start, end, n1, n2, n3) }
}
#[inline(always)]
pub(crate) unsafe fn count_raw(n1: u8, start: *const u8, end: *const u8) -> usize {
    unsafe { defraw!(One, count_raw, start, end, n1) }
}
