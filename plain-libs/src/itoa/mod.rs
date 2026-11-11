mod u128_ext;
use core::hint;
use core::mem::{self, MaybeUninit};
use core::str;
pub struct Buffer {
    bytes: [MaybeUninit<u8>; i128::MAX_STR_LEN],
}
impl Default for Buffer {
    #[inline]
    fn default() -> Buffer {
        Buffer::new()
    }
}
impl Copy for Buffer {}
impl Clone for Buffer {
    #[inline]
    fn clone(&self) -> Self {
        Buffer::new()
    }
}
impl Buffer {
    #[inline]
    pub fn new() -> Buffer {
        let bytes = [MaybeUninit::<u8>::uninit(); i128::MAX_STR_LEN];
        Buffer { bytes }
    }
    pub fn format<I: Integer>(&mut self, i: I) -> &str {
        let buf_ptr = self.bytes.as_mut_ptr().cast::<I::Buffer>();
        let string = i.write(unsafe { &mut *buf_ptr });
        if string.len() > I::MAX_STR_LEN {
            unsafe { hint::unreachable_unchecked() };
        }
        string
    }
}
pub trait Integer: private::Sealed {
    const MAX_STR_LEN: usize;
}
mod private {
    pub trait Sealed: Copy {
        type Buffer: 'static;
        fn write(self, buf: &mut Self::Buffer) -> &str;
    }
}
macro_rules! impl_Integer {
    ($Signed : ident , $Unsigned : ident) => {
        const _: () = {
            assert!($Signed::MIN < 0, "need signed");
            assert!($Unsigned::MIN == 0, "need unsigned");
            assert!($Signed::BITS == $Unsigned::BITS, "need counterparts");
        };
        impl Integer for $Unsigned {
            const MAX_STR_LEN: usize = $Unsigned::MAX.ilog10() as usize + 1;
        }
        impl private::Sealed for $Unsigned {
            type Buffer = [MaybeUninit<u8>; Self::MAX_STR_LEN];
            #[inline]
            fn write(self, buf: &mut Self::Buffer) -> &str {
                let offset = Unsigned::fmt(self, buf);
                unsafe { slice_buffer_to_str(buf, offset) }
            }
        }
        impl Integer for $Signed {
            const MAX_STR_LEN: usize = $Signed::MAX.ilog10() as usize + 2;
        }
        impl private::Sealed for $Signed {
            type Buffer = [MaybeUninit<u8>; Self::MAX_STR_LEN];
            #[inline]
            fn write(self, buf: &mut Self::Buffer) -> &str {
                let mut offset = Self::MAX_STR_LEN - $Unsigned::MAX_STR_LEN;
                offset += Unsigned::fmt(
                    self.unsigned_abs(),
                    (&mut buf[offset..]).try_into().unwrap(),
                );
                if self < 0 {
                    offset -= 1;
                    buf[offset].write(b'-');
                }
                unsafe { slice_buffer_to_str(buf, offset) }
            }
        }
    };
}
impl_Integer!(i8, u8);
impl_Integer!(i16, u16);
impl_Integer!(i32, u32);
impl_Integer!(i64, u64);
impl_Integer!(i128, u128);
macro_rules! impl_Integer_size {
    ($t : ty as $primitive : ident #[cfg (target_pointer_width = $width : literal)]) => {
        #[cfg (target_pointer_width = $width)]
        impl Integer for $t {
            const MAX_STR_LEN: usize = <$primitive as Integer>::MAX_STR_LEN;
        }
        #[cfg (target_pointer_width = $width)]
        impl private::Sealed for $t {
            type Buffer = <$primitive as private::Sealed>::Buffer;
            #[inline]
            fn write(self, buf: &mut Self::Buffer) -> &str {
                (self as $primitive).write(buf)
            }
        }
    };
}
impl_Integer_size ! (isize as i16 #[cfg (target_pointer_width = "16")]);
impl_Integer_size ! (usize as u16 #[cfg (target_pointer_width = "16")]);
impl_Integer_size ! (isize as i32 #[cfg (target_pointer_width = "32")]);
impl_Integer_size ! (usize as u32 #[cfg (target_pointer_width = "32")]);
impl_Integer_size ! (isize as i64 #[cfg (target_pointer_width = "64")]);
impl_Integer_size ! (usize as u64 #[cfg (target_pointer_width = "64")]);
#[repr(C, align(2))]
struct DecimalPairs([u8; 200]);
static DECIMAL_PAIRS: DecimalPairs = DecimalPairs(
    *b"0001020304050607080910111213141516171819\
       2021222324252627282930313233343536373839\
       4041424344454647484950515253545556575859\
       6061626364656667686970717273747576777879\
       8081828384858687888990919293949596979899",
);
fn divmod100(value: u32) -> (u32, u32) {
    debug_assert!(value < 10_000);
    const EXP: u32 = 19;
    const SIG: u32 = (1 << EXP) / 100 + 1;
    let div = (value * SIG) >> EXP;
    (div, value - div * 100)
}
unsafe fn slice_buffer_to_str(buf: &[MaybeUninit<u8>], offset: usize) -> &str {
    unsafe {
        let written = unsafe { buf.get_unchecked(offset..) };
        unsafe { str::from_utf8_unchecked(&*(written as *const [MaybeUninit<u8>] as *const [u8])) }
    }
}
trait Unsigned: Integer {
    fn fmt(self, buf: &mut Self::Buffer) -> usize;
}
macro_rules! impl_Unsigned {
    ($Unsigned : ident) => {
        impl Unsigned for $Unsigned {
            fn fmt(self, buf: &mut Self::Buffer) -> usize {
                let mut offset = buf.len();
                let mut remain = self;
                while mem::size_of::<Self>() > 1
                    && remain
                        > 999
                            .try_into()
                            .expect("branch is not hit for types that cannot fit 999 (u8)")
                {
                    offset -= 4;
                    let scale: Self = 1_00_00
                        .try_into()
                        .expect("branch is not hit for types that cannot fit 1E4 (u8)");
                    let quad = remain % scale;
                    remain /= scale;
                    let (pair1, pair2) = divmod100(quad as u32);
                    unsafe {
                        buf[offset + 0]
                            .write(*DECIMAL_PAIRS.0.get_unchecked(pair1 as usize * 2 + 0));
                        buf[offset + 1]
                            .write(*DECIMAL_PAIRS.0.get_unchecked(pair1 as usize * 2 + 1));
                        buf[offset + 2]
                            .write(*DECIMAL_PAIRS.0.get_unchecked(pair2 as usize * 2 + 0));
                        buf[offset + 3]
                            .write(*DECIMAL_PAIRS.0.get_unchecked(pair2 as usize * 2 + 1));
                    }
                }
                if remain > 9 {
                    offset -= 2;
                    let (last, pair) = divmod100(remain as u32);
                    remain = last as Self;
                    unsafe {
                        buf[offset + 0]
                            .write(*DECIMAL_PAIRS.0.get_unchecked(pair as usize * 2 + 0));
                        buf[offset + 1]
                            .write(*DECIMAL_PAIRS.0.get_unchecked(pair as usize * 2 + 1));
                    }
                }
                if remain != 0 || self == 0 {
                    offset -= 1;
                    let last = remain as u8 & 15;
                    buf[offset].write(b'0' + last);
                }
                offset
            }
        }
    };
}
impl_Unsigned!(u8);
impl_Unsigned!(u16);
impl_Unsigned!(u32);
impl_Unsigned!(u64);
impl Unsigned for u128 {
    fn fmt(self, buf: &mut Self::Buffer) -> usize {
        if self == 0 {
            let offset = buf.len() - 1;
            buf[offset].write(b'0');
            return offset;
        }
        let (quot_1e16, mod_1e16) = div_rem_1e16(self);
        let (mut remain, mut offset) = if quot_1e16 == 0 {
            (mod_1e16, u128::MAX_STR_LEN)
        } else {
            enc_16lsd::<{ u128::MAX_STR_LEN - 16 }>(buf, mod_1e16);
            let (quot2, mod2) = div_rem_1e16(quot_1e16);
            if quot2 == 0 {
                (mod2, u128::MAX_STR_LEN - 16)
            } else {
                enc_16lsd::<{ u128::MAX_STR_LEN - 32 }>(buf, mod2);
                (quot2 as u64, u128::MAX_STR_LEN - 32)
            }
        };
        while remain > 999 {
            offset -= 4;
            let quad = remain % 1_00_00;
            remain /= 1_00_00;
            let (pair1, pair2) = divmod100(quad as u32);
            unsafe {
                buf[offset + 0].write(*DECIMAL_PAIRS.0.get_unchecked(pair1 as usize * 2 + 0));
                buf[offset + 1].write(*DECIMAL_PAIRS.0.get_unchecked(pair1 as usize * 2 + 1));
                buf[offset + 2].write(*DECIMAL_PAIRS.0.get_unchecked(pair2 as usize * 2 + 0));
                buf[offset + 3].write(*DECIMAL_PAIRS.0.get_unchecked(pair2 as usize * 2 + 1));
            }
        }
        if remain > 9 {
            offset -= 2;
            let (last, pair) = divmod100(remain as u32);
            remain = last as u64;
            unsafe {
                buf[offset + 0].write(*DECIMAL_PAIRS.0.get_unchecked(pair as usize * 2 + 0));
                buf[offset + 1].write(*DECIMAL_PAIRS.0.get_unchecked(pair as usize * 2 + 1));
            }
        }
        if remain != 0 {
            offset -= 1;
            let last = remain as u8 & 15;
            buf[offset].write(b'0' + last);
        }
        offset
    }
}
fn enc_16lsd<const OFFSET: usize>(buf: &mut [MaybeUninit<u8>], n: u64) {
    let mut remain = n;
    for quad_index in (1..4).rev() {
        let quad = remain % 1_00_00;
        remain /= 1_00_00;
        let (pair1, pair2) = divmod100(quad as u32);
        unsafe {
            buf[quad_index * 4 + OFFSET + 0]
                .write(*DECIMAL_PAIRS.0.get_unchecked(pair1 as usize * 2 + 0));
            buf[quad_index * 4 + OFFSET + 1]
                .write(*DECIMAL_PAIRS.0.get_unchecked(pair1 as usize * 2 + 1));
            buf[quad_index * 4 + OFFSET + 2]
                .write(*DECIMAL_PAIRS.0.get_unchecked(pair2 as usize * 2 + 0));
            buf[quad_index * 4 + OFFSET + 3]
                .write(*DECIMAL_PAIRS.0.get_unchecked(pair2 as usize * 2 + 1));
        }
    }
    let (pair1, pair2) = divmod100(remain as u32);
    unsafe {
        buf[OFFSET + 0].write(*DECIMAL_PAIRS.0.get_unchecked(pair1 as usize * 2 + 0));
        buf[OFFSET + 1].write(*DECIMAL_PAIRS.0.get_unchecked(pair1 as usize * 2 + 1));
        buf[OFFSET + 2].write(*DECIMAL_PAIRS.0.get_unchecked(pair2 as usize * 2 + 0));
        buf[OFFSET + 3].write(*DECIMAL_PAIRS.0.get_unchecked(pair2 as usize * 2 + 1));
    }
}
fn div_rem_1e16(n: u128) -> (u128, u64) {
    const D: u128 = 1_0000_0000_0000_0000;
    if n < D {
        return (0, n as u64);
    }
    const M_HIGH: u128 = 76624777043294442917917351357515459181;
    const SH_POST: u8 = 51;
    let quot = u128_ext::mulhi(n, M_HIGH) >> SH_POST;
    let rem = n - quot * D;
    (quot, rem as u64)
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/itoa.rs"]
mod tests;
