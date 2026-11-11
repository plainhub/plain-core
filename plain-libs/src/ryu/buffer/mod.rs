use crate::ryu::raw;
use core::mem::MaybeUninit;
use core::{slice, str};
const NAN: &str = "NaN";
const INFINITY: &str = "inf";
const NEG_INFINITY: &str = "-inf";
pub struct Buffer {
    bytes: [MaybeUninit<u8>; 24],
}
impl Buffer {
    #[inline]
    pub fn new() -> Self {
        let bytes = [MaybeUninit::<u8>::uninit(); 24];
        Buffer { bytes }
    }
    pub fn format<F: Float>(&mut self, f: F) -> &str {
        if f.is_nonfinite() {
            f.format_nonfinite()
        } else {
            self.format_finite(f)
        }
    }
    #[inline]
    pub fn format_finite<F: Float>(&mut self, f: F) -> &str {
        unsafe {
            let n = f.write_to_ryu_buffer(self.bytes.as_mut_ptr().cast::<u8>());
            debug_assert!(n <= self.bytes.len());
            let slice = slice::from_raw_parts(self.bytes.as_ptr().cast::<u8>(), n);
            str::from_utf8_unchecked(slice)
        }
    }
}
impl Copy for Buffer {}
impl Clone for Buffer {
    #[inline]
    fn clone(&self) -> Self {
        Buffer::new()
    }
}
impl Default for Buffer {
    #[inline]
    fn default() -> Self {
        Buffer::new()
    }
}
pub trait Float: Sealed {}
impl Float for f32 {}
impl Float for f64 {}
pub trait Sealed: Copy {
    fn is_nonfinite(self) -> bool;
    fn format_nonfinite(self) -> &'static str;
    unsafe fn write_to_ryu_buffer(self, result: *mut u8) -> usize;
}
impl Sealed for f32 {
    #[inline]
    fn is_nonfinite(self) -> bool {
        const EXP_MASK: u32 = 0x7f800000;
        let bits = self.to_bits();
        bits & EXP_MASK == EXP_MASK
    }
    #[cold]
    fn format_nonfinite(self) -> &'static str {
        const MANTISSA_MASK: u32 = 0x007fffff;
        const SIGN_MASK: u32 = 0x80000000;
        let bits = self.to_bits();
        if bits & MANTISSA_MASK != 0 {
            NAN
        } else if bits & SIGN_MASK != 0 {
            NEG_INFINITY
        } else {
            INFINITY
        }
    }
    #[inline]
    unsafe fn write_to_ryu_buffer(self, result: *mut u8) -> usize {
        unsafe { raw::format32(self, result) }
    }
}
impl Sealed for f64 {
    #[inline]
    fn is_nonfinite(self) -> bool {
        const EXP_MASK: u64 = 0x7ff0000000000000;
        let bits = self.to_bits();
        bits & EXP_MASK == EXP_MASK
    }
    #[cold]
    fn format_nonfinite(self) -> &'static str {
        const MANTISSA_MASK: u64 = 0x000fffffffffffff;
        const SIGN_MASK: u64 = 0x8000000000000000;
        let bits = self.to_bits();
        if bits & MANTISSA_MASK != 0 {
            NAN
        } else if bits & SIGN_MASK != 0 {
            NEG_INFINITY
        } else {
            INFINITY
        }
    }
    #[inline]
    unsafe fn write_to_ryu_buffer(self, result: *mut u8) -> usize {
        unsafe { raw::format64(self, result) }
    }
}
