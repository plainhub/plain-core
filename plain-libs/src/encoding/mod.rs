#[macro_use]
mod macros;
mod ascii;
mod big5;
mod data;
mod euc_kr;
mod gb18030;
mod gb18030_2022;
mod handles;

mod shift_jis;
mod single_byte;
mod utf_16;
mod utf_8;
mod variant;
use crate::encoding::ascii::ascii_valid_up_to;
use crate::encoding::utf_8::utf8_valid_up_to;
use crate::encoding::variant::*;
use core::hash::Hash;
use core::hash::Hasher;
use core::mem::MaybeUninit;
use std::borrow::Cow;
use std::string::String;
pub static BIG5_INIT: Encoding = Encoding {
    name: "Big5",
    variant: VariantEncoding::Big5,
};
pub static BIG5: &'static Encoding = &BIG5_INIT;
pub static EUC_KR_INIT: Encoding = Encoding {
    name: "EUC-KR",
    variant: VariantEncoding::EucKr,
};
pub static EUC_KR: &'static Encoding = &EUC_KR_INIT;
pub static GBK_INIT: Encoding = Encoding {
    name: "GBK",
    variant: VariantEncoding::Gbk,
};
pub static GBK: &'static Encoding = &GBK_INIT;
pub static SHIFT_JIS_INIT: Encoding = Encoding {
    name: "Shift_JIS",
    variant: VariantEncoding::ShiftJis,
};
pub static SHIFT_JIS: &'static Encoding = &SHIFT_JIS_INIT;
pub static UTF_16BE_INIT: Encoding = Encoding {
    name: "UTF-16BE",
    variant: VariantEncoding::Utf16Be,
};
pub static UTF_16BE: &'static Encoding = &UTF_16BE_INIT;
pub static UTF_16LE_INIT: Encoding = Encoding {
    name: "UTF-16LE",
    variant: VariantEncoding::Utf16Le,
};
pub static UTF_16LE: &'static Encoding = &UTF_16LE_INIT;
pub static UTF_8_INIT: Encoding = Encoding {
    name: "UTF-8",
    variant: VariantEncoding::Utf8,
};
pub static UTF_8: &'static Encoding = &UTF_8_INIT;
pub static MACINTOSH_INIT: Encoding = Encoding {
    name: "macintosh",
    variant: VariantEncoding::SingleByte(&data::SINGLE_BYTE_DATA.macintosh),
};
pub static MACINTOSH: &'static Encoding = &MACINTOSH_INIT;
pub static WINDOWS_1250_INIT: Encoding = Encoding {
    name: "windows-1250",
    variant: VariantEncoding::SingleByte(&data::SINGLE_BYTE_DATA.windows_1250),
};
pub static WINDOWS_1250: &'static Encoding = &WINDOWS_1250_INIT;
pub static WINDOWS_1251_INIT: Encoding = Encoding {
    name: "windows-1251",
    variant: VariantEncoding::SingleByte(&data::SINGLE_BYTE_DATA.windows_1251),
};
pub static WINDOWS_1251: &'static Encoding = &WINDOWS_1251_INIT;
pub static WINDOWS_1253_INIT: Encoding = Encoding {
    name: "windows-1253",
    variant: VariantEncoding::SingleByte(&data::SINGLE_BYTE_DATA.windows_1253),
};
pub static WINDOWS_1253: &'static Encoding = &WINDOWS_1253_INIT;
pub static WINDOWS_1254_INIT: Encoding = Encoding {
    name: "windows-1254",
    variant: VariantEncoding::SingleByte(&data::SINGLE_BYTE_DATA.windows_1254),
};
pub static WINDOWS_1254: &'static Encoding = &WINDOWS_1254_INIT;
pub static WINDOWS_1255_INIT: Encoding = Encoding {
    name: "windows-1255",
    variant: VariantEncoding::SingleByte(&data::SINGLE_BYTE_DATA.windows_1255),
};
pub static WINDOWS_1255: &'static Encoding = &WINDOWS_1255_INIT;
pub static WINDOWS_1256_INIT: Encoding = Encoding {
    name: "windows-1256",
    variant: VariantEncoding::SingleByte(&data::SINGLE_BYTE_DATA.windows_1256),
};
pub static WINDOWS_1256: &'static Encoding = &WINDOWS_1256_INIT;
pub static WINDOWS_1257_INIT: Encoding = Encoding {
    name: "windows-1257",
    variant: VariantEncoding::SingleByte(&data::SINGLE_BYTE_DATA.windows_1257),
};
pub static WINDOWS_1257: &'static Encoding = &WINDOWS_1257_INIT;
pub static WINDOWS_1258_INIT: Encoding = Encoding {
    name: "windows-1258",
    variant: VariantEncoding::SingleByte(&data::SINGLE_BYTE_DATA.windows_1258),
};
pub static WINDOWS_1258: &'static Encoding = &WINDOWS_1258_INIT;
pub static WINDOWS_874_INIT: Encoding = Encoding {
    name: "windows-874",
    variant: VariantEncoding::SingleByte(&data::SINGLE_BYTE_DATA.windows_874),
};
pub static WINDOWS_874: &'static Encoding = &WINDOWS_874_INIT;
pub struct Encoding {
    name: &'static str,
    variant: VariantEncoding,
}
impl Encoding {
    #[inline]
    pub fn for_bom(buffer: &[u8]) -> Option<(&'static Encoding, usize)> {
        if buffer.starts_with(b"\xEF\xBB\xBF") {
            Some((UTF_8, 3))
        } else if buffer.starts_with(b"\xFF\xFE") {
            Some((UTF_16LE, 2))
        } else if buffer.starts_with(b"\xFE\xFF") {
            Some((UTF_16BE, 2))
        } else {
            None
        }
    }
    #[inline]
    pub fn name(&'static self) -> &'static str {
        self.name
    }

    #[inline]
    fn is_potentially_borrowable(&'static self) -> bool {
        !(self == UTF_16BE || self == UTF_16LE)
    }
    #[inline]
    pub fn decode<'a>(&'static self, bytes: &'a [u8]) -> (Cow<'a, str>, &'static Encoding, bool) {
        let (encoding, without_bom) = match Encoding::for_bom(bytes) {
            Some((encoding, bom_length)) => (encoding, &bytes[bom_length..]),
            None => (self, bytes),
        };
        let (cow, had_errors) = encoding.decode_without_bom_handling(without_bom);
        (cow, encoding, had_errors)
    }

    pub fn decode_without_bom_handling<'a>(&'static self, bytes: &'a [u8]) -> (Cow<'a, str>, bool) {
        let (mut decoder, mut string, mut total_read) = if self.is_potentially_borrowable() {
            let valid_up_to = if self == UTF_8 {
                utf8_valid_up_to(bytes)
            } else {
                ascii_valid_up_to(bytes)
            };
            if valid_up_to == bytes.len() {
                let str: &str = unsafe { core::str::from_utf8_unchecked(bytes) };
                return (Cow::Borrowed(str), false);
            }
            let decoder = self.new_decoder_without_bom_handling();
            let rounded_without_replacement = checked_next_power_of_two(checked_add(
                valid_up_to,
                decoder.max_utf8_buffer_length_without_replacement(bytes.len() - valid_up_to),
            ));
            let with_replacement = checked_add(
                valid_up_to,
                decoder.max_utf8_buffer_length(bytes.len() - valid_up_to),
            );
            let mut string = String::with_capacity(
                checked_min(rounded_without_replacement, with_replacement).unwrap(),
            );
            let vec = unsafe { string.as_mut_vec() };
            vec.extend_from_slice(&bytes[..valid_up_to]);
            (decoder, string, valid_up_to)
        } else {
            let decoder = self.new_decoder_without_bom_handling();
            let rounded_without_replacement = checked_next_power_of_two(
                decoder.max_utf8_buffer_length_without_replacement(bytes.len()),
            );
            let with_replacement = decoder.max_utf8_buffer_length(bytes.len());
            let string = String::with_capacity(
                checked_min(rounded_without_replacement, with_replacement).unwrap(),
            );
            (decoder, string, 0)
        };
        let mut total_had_errors = false;
        loop {
            let (result, read, had_errors) =
                decoder.decode_to_string(&bytes[total_read..], &mut string, true);
            total_read += read;
            total_had_errors |= had_errors;
            match result {
                CoderResult::InputEmpty => {
                    debug_assert_eq!(total_read, bytes.len());
                    return (Cow::Owned(string), total_had_errors);
                }
                CoderResult::OutputFull => {
                    let needed = decoder.max_utf8_buffer_length(bytes.len() - total_read);
                    string.reserve(needed.unwrap());
                }
            }
        }
    }
    pub fn decode_without_bom_handling_and_without_replacement<'a>(
        &'static self,
        bytes: &'a [u8],
    ) -> Option<Cow<'a, str>> {
        if self == UTF_8 {
            let valid_up_to = utf8_valid_up_to(bytes);
            if valid_up_to == bytes.len() {
                let str: &str = unsafe { core::str::from_utf8_unchecked(bytes) };
                return Some(Cow::Borrowed(str));
            }
            return None;
        }
        let (mut decoder, mut string, input) = if self.is_potentially_borrowable() {
            let valid_up_to = { ascii_valid_up_to(bytes) };
            if valid_up_to == bytes.len() {
                let str: &str = unsafe { core::str::from_utf8_unchecked(bytes) };
                return Some(Cow::Borrowed(str));
            }
            let decoder = self.new_decoder_without_bom_handling();
            let mut string = String::with_capacity(
                checked_add(
                    valid_up_to,
                    decoder.max_utf8_buffer_length_without_replacement(bytes.len() - valid_up_to),
                )
                .unwrap(),
            );
            let vec = unsafe { string.as_mut_vec() };
            vec.extend_from_slice(&bytes[..valid_up_to]);
            (decoder, string, &bytes[valid_up_to..])
        } else {
            let decoder = self.new_decoder_without_bom_handling();
            let string = String::with_capacity(
                decoder
                    .max_utf8_buffer_length_without_replacement(bytes.len())
                    .unwrap(),
            );
            (decoder, string, bytes)
        };
        let (result, read) = decoder.decode_to_string_without_replacement(input, &mut string, true);
        match result {
            DecoderResult::InputEmpty => {
                debug_assert_eq!(read, input.len());
                Some(Cow::Owned(string))
            }
            DecoderResult::Malformed(_, _) => None,
            DecoderResult::OutputFull => unreachable!(),
        }
    }
    fn new_variant_decoder(&'static self) -> VariantDecoder {
        self.variant.new_variant_decoder()
    }

    #[inline]
    pub fn new_decoder_without_bom_handling(&'static self) -> Decoder {
        Decoder::new(self.new_variant_decoder())
    }
}
impl PartialEq for Encoding {
    #[inline]
    fn eq(&self, other: &Encoding) -> bool {
        ::core::ptr::eq(self, other)
    }
}
impl Eq for Encoding {}
impl Hash for Encoding {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self as *const Encoding).hash(state);
    }
}
impl core::fmt::Debug for Encoding {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Encoding")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}
#[derive(PartialEq, Debug, Copy, Clone)]
enum DecoderLifeCycle {
    Converting,
    Finished,
}
#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub enum CoderResult {
    InputEmpty,
    OutputFull,
}
#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub enum DecoderResult {
    InputEmpty,
    OutputFull,
    Malformed(u8, u8),
}
pub struct Decoder {
    variant: VariantDecoder,
    life_cycle: DecoderLifeCycle,
}
impl Decoder {
    fn new(variant: VariantDecoder) -> Decoder {
        Decoder {
            variant,
            life_cycle: DecoderLifeCycle::Converting,
        }
    }

    pub fn decode_to_utf8(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        last: bool,
    ) -> (CoderResult, usize, usize, bool) {
        let mut had_errors = false;
        let mut total_read = 0usize;
        let mut total_written = 0usize;
        loop {
            let (result, read, written) = self.decode_to_utf8_without_replacement(
                &src[total_read..],
                &mut dst[total_written..],
                last,
            );
            total_read += read;
            total_written += written;
            match result {
                DecoderResult::InputEmpty => {
                    return (
                        CoderResult::InputEmpty,
                        total_read,
                        total_written,
                        had_errors,
                    );
                }
                DecoderResult::OutputFull => {
                    return (
                        CoderResult::OutputFull,
                        total_read,
                        total_written,
                        had_errors,
                    );
                }
                DecoderResult::Malformed(_, _) => {
                    had_errors = true;
                    dst[total_written] = 0xEFu8;
                    total_written += 1;
                    dst[total_written] = 0xBFu8;
                    total_written += 1;
                    dst[total_written] = 0xBDu8;
                    total_written += 1;
                }
            }
        }
    }

    pub fn decode_to_string(
        &mut self,
        src: &[u8],
        dst: &mut String,
        last: bool,
    ) -> (CoderResult, usize, bool) {
        let vec = unsafe { dst.as_mut_vec() };
        let old_len = vec.len();
        let spare_capacity = minimally_init(vec.spare_capacity_mut());
        let (result, read, written, replaced) = self.decode_to_utf8(src, spare_capacity, last);
        debug_assert!(written <= spare_capacity.len());
        let new_len = old_len + written;
        assert!(new_len <= vec.capacity());
        unsafe {
            vec.set_len(new_len);
        }
        (result, read, replaced)
    }
    fn decode_to_utf8_without_replacement(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        last: bool,
    ) -> (DecoderResult, usize, usize) {
        assert_ne!(
            self.life_cycle,
            DecoderLifeCycle::Finished,
            "decoder already finished"
        );
        let (result, read, written) = self.variant.decode_to_utf8_raw(src, dst, last);
        if last && result == DecoderResult::InputEmpty {
            self.life_cycle = DecoderLifeCycle::Finished;
        }
        (result, read, written)
    }

    pub fn decode_to_string_without_replacement(
        &mut self,
        src: &[u8],
        dst: &mut String,
        last: bool,
    ) -> (DecoderResult, usize) {
        let vec = unsafe { dst.as_mut_vec() };
        let old_len = vec.len();
        let spare_capacity = minimally_init(vec.spare_capacity_mut());
        let (result, read, written) =
            self.decode_to_utf8_without_replacement(src, spare_capacity, last);
        debug_assert!(written <= spare_capacity.len());
        let new_len = old_len + written;
        assert!(new_len <= vec.capacity());
        unsafe {
            vec.set_len(new_len);
        }
        (result, read)
    }
}
impl core::fmt::Debug for Decoder {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Decoder")
            .field("life_cycle", &self.life_cycle)
            .finish_non_exhaustive()
    }
}

#[inline(always)]
fn in_range16(i: u16, start: u16, end: u16) -> bool {
    i.wrapping_sub(start) < (end - start)
}

#[inline(always)]
fn in_inclusive_range8(i: u8, start: u8, end: u8) -> bool {
    i.wrapping_sub(start) <= (end - start)
}

#[inline(always)]
fn in_inclusive_range(i: usize, start: usize, end: usize) -> bool {
    i.wrapping_sub(start) <= (end - start)
}
#[inline(always)]
fn checked_add(num: usize, opt: Option<usize>) -> Option<usize> {
    if let Some(n) = opt {
        n.checked_add(num)
    } else {
        None
    }
}
#[inline(always)]
fn checked_add_opt(one: Option<usize>, other: Option<usize>) -> Option<usize> {
    if let Some(n) = one {
        checked_add(n, other)
    } else {
        None
    }
}
#[inline(always)]
fn checked_mul(num: usize, opt: Option<usize>) -> Option<usize> {
    if let Some(n) = opt {
        n.checked_mul(num)
    } else {
        None
    }
}
#[inline(always)]
fn checked_div(opt: Option<usize>, num: usize) -> Option<usize> {
    if let Some(n) = opt {
        n.checked_div(num)
    } else {
        None
    }
}
#[inline(always)]
fn checked_next_power_of_two(opt: Option<usize>) -> Option<usize> {
    opt.map(|n| n.next_power_of_two())
}
#[inline(always)]
fn checked_min(one: Option<usize>, other: Option<usize>) -> Option<usize> {
    if let Some(a) = one {
        if let Some(b) = other {
            Some(::core::cmp::min(a, b))
        } else {
            Some(a)
        }
    } else {
        other
    }
}
fn minimally_init(buf: &mut [MaybeUninit<u8>]) -> &mut [u8] {
    loop {
        if let Some(b) = buf.first_mut() {
            *b = MaybeUninit::zeroed();
        } else {
            break;
        };
        let mut i = SMALLEST_PAGE_SIZE - (buf.as_mut_ptr().addr() & PAGE_MASK);
        while let Some(b) = buf.get_mut(i) {
            *b = MaybeUninit::zeroed();
            i += SMALLEST_PAGE_SIZE;
        }
        break;
    }
    let ptr = buf.as_mut_ptr();
    unsafe {
        pointer_escapes(ptr);
    }
    unsafe { core::slice::from_raw_parts_mut(buf.as_mut_ptr().cast(), buf.len()) }
}
cfg_if::cfg_if! { if # [cfg (all (not (miri) , all () , any (target_arch = "x86" , target_arch = "x86_64" , target_arch = "arm" , target_arch = "aarch64" , target_arch = "arm64ec" , target_arch = "riscv32" , target_arch = "riscv64" , target_arch = "loongarch64" , target_arch = "s390x")))] { # [inline (always)] unsafe fn pointer_escapes (ptr : * mut MaybeUninit < u8 >) { unsafe { core :: arch :: asm ! ("/* {0} */" , in (reg) ptr) ; } } } else if #[cfg(all())] { # [inline (never)] unsafe fn pointer_escapes (_ptr : * mut MaybeUninit < u8 >) {
unsafe { }
} } else { } }
mod utf8_simd;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/encoding.rs"]
mod tests;

const SMALLEST_PAGE_SIZE: usize = 4096;
const PAGE_MASK: usize = SMALLEST_PAGE_SIZE - 1;

impl Decoder {
    fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        assert_ne!(
            self.life_cycle,
            DecoderLifeCycle::Finished,
            "decoder already finished"
        );
        self.variant.max_utf8_buffer_length(byte_length)
    }
    fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        assert_ne!(
            self.life_cycle,
            DecoderLifeCycle::Finished,
            "decoder already finished"
        );
        self.variant
            .max_utf8_buffer_length_without_replacement(byte_length)
    }
}
