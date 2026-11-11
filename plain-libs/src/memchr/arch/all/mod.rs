pub mod memchr;
pub mod packedpair;
pub mod rabinkarp;
pub mod shiftor;
pub mod twoway;
#[inline(always)]
pub fn is_prefix(haystack: &[u8], needle: &[u8]) -> bool {
    needle.len() <= haystack.len() && is_equal(&haystack[..needle.len()], needle)
}
#[inline(always)]
pub fn is_suffix(haystack: &[u8], needle: &[u8]) -> bool {
    needle.len() <= haystack.len() && is_equal(&haystack[haystack.len() - needle.len()..], needle)
}
#[inline(always)]
pub fn is_equal(x: &[u8], y: &[u8]) -> bool {
    if x.len() != y.len() {
        return false;
    }
    unsafe { is_equal_raw(x.as_ptr(), y.as_ptr(), x.len()) }
}
#[inline(always)]
pub unsafe fn is_equal_raw(mut x: *const u8, mut y: *const u8, mut n: usize) -> bool {
    unsafe {
        while n >= 4 {
            let vx = x.cast::<u32>().read_unaligned();
            let vy = y.cast::<u32>().read_unaligned();
            if vx != vy {
                return false;
            }
            x = x.add(4);
            y = y.add(4);
            n -= 4;
        }
        if n >= 2 {
            let vx = x.cast::<u16>().read_unaligned();
            let vy = y.cast::<u16>().read_unaligned();
            if vx != vy {
                return false;
            }
            x = x.add(2);
            y = y.add(2);
            n -= 2;
        }
        if n > 0 {
            if x.read() != y.read() {
                return false;
            }
        }
        true
    }
}
