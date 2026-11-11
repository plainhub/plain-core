use super::*;
use crate::encoding::ascii::validate_ascii;
use crate::encoding::handles::*;
use crate::encoding::variant::*;
cfg_if::cfg_if! { if #[cfg(all())] { # [inline (always)] fn unlikely (b : bool) -> bool { b } # [inline (always)] fn likely (b : bool) -> bool { b } } }
#[repr(align(64))]
pub struct Utf8Data {
    pub table: [u8; 384],
}
pub static UTF8_DATA: Utf8Data = Utf8Data {
    table: [
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 148, 148, 148,
        148, 148, 148, 148, 148, 148, 148, 148, 148, 148, 148, 148, 148, 164, 164, 164, 164, 164,
        164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164,
        164, 164, 164, 164, 164, 164, 164, 164, 164, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
        8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 32, 8, 8, 64, 8, 8, 8, 128, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    ],
};
cfg_if::cfg_if! { if # [cfg (all (target_arch = "aarch64" , target_feature = "neon"))] { # [inline (always)] fn fast_utf8_valid_up_to (src : & [u8]) -> Option < usize > { if src . len () >= 64 { Some (match unsafe { crate::encoding::utf8_simd::implementation::aarch64::neon::validate_utf8_compat (src) } { Ok (_) => src . len () , Err (e) => e . valid_up_to () , }) } else { None } } } else if # [cfg (target_feature = "avx2")] { # [inline (always)] fn fast_utf8_valid_up_to (src : & [u8]) -> Option < usize > { if src . len () >= 64 { Some (match unsafe { crate::encoding::utf8_simd::implementation::x86::avx2::validate_utf8_compat (src) } { Ok (_) => src . len () , Err (e) => e . valid_up_to () , }) } else { None } } } else if # [cfg (target_feature = "sse4.2")] { # [inline (always)] fn fast_utf8_valid_up_to (src : & [u8]) -> Option < usize > { if src . len () >= 64 { if crate::cpu_features::is_enabled_avx2() { Some (match unsafe { crate::encoding::utf8_simd::implementation::x86::avx2::validate_utf8_compat (src) } { Ok (_) => src . len () , Err (e) => e . valid_up_to () , }) } else { Some (match unsafe { crate::encoding::utf8_simd::implementation::x86::sse42::validate_utf8_compat (src) } { Ok (_) => src . len () , Err (e) => e . valid_up_to () , }) } } else { None } } } else if # [cfg (target_feature = "sse")] { # [inline (always)] fn fast_utf8_valid_up_to (src : & [u8]) -> Option < usize > { if src . len () >= 64 { if crate::cpu_features::is_enabled_avx2() { Some (match unsafe { crate::encoding::utf8_simd::implementation::x86::avx2::validate_utf8_compat (src) } { Ok (_) => src . len () , Err (e) => e . valid_up_to () , }) } else if crate::cpu_features::is_enabled_sse42() { Some (match unsafe { crate::encoding::utf8_simd::implementation::x86::sse42::validate_utf8_compat (src) } { Ok (_) => src . len () , Err (e) => e . valid_up_to () , }) } else { None } } else { None } } } else if # [cfg (all (target_arch = "wasm32" , target_feature = "simd128"))] { # [inline (always)] fn fast_utf8_valid_up_to (src : & [u8]) -> Option < usize > { if src . len () >= 64 { Some (match unsafe { crate::encoding::utf8_simd::implementation::wasm32::simd128::validate_utf8_compat (src) } { Ok (_) => src . len () , Err (e) => e . valid_up_to () , }) } else { None } } } else { # [inline (always)] fn fast_utf8_valid_up_to (_src : & [u8]) -> Option < usize > { None } } }
pub fn utf8_valid_up_to(src: &[u8]) -> usize {
    if let Some(up_to) = fast_utf8_valid_up_to(src) {
        return up_to;
    }
    let mut read = 0;
    'outer: loop {
        let mut byte = {
            let src_remaining = &src[read..];
            match validate_ascii(src_remaining) {
                None => {
                    return src.len();
                }
                Some((non_ascii, consumed)) => {
                    read += consumed;
                    non_ascii
                }
            }
        };
        if likely(read + 4 <= src.len()) {
            'inner: loop {
                if likely(in_inclusive_range8(byte, 0xC2, 0xDF)) {
                    let second = unsafe { *(src.get_unchecked(read + 1)) };
                    if !in_inclusive_range8(second, 0x80, 0xBF) {
                        break 'outer;
                    }
                    read += 2;
                    if likely(read + 4 <= src.len()) {
                        byte = unsafe { *(src.get_unchecked(read)) };
                        if byte < 0x80 {
                            read += 1;
                            continue 'outer;
                        }
                        continue 'inner;
                    }
                    break 'inner;
                }
                if likely(byte < 0xF0) {
                    'three: loop {
                        let second = unsafe { *(src.get_unchecked(read + 1)) };
                        let third = unsafe { *(src.get_unchecked(read + 2)) };
                        if ((UTF8_DATA.table[usize::from(second)]
                            & unsafe { *(UTF8_DATA.table.get_unchecked(byte as usize + 0x80)) })
                            | (third >> 6))
                            != 2
                        {
                            break 'outer;
                        }
                        read += 3;
                        if likely(read + 4 <= src.len()) {
                            byte = unsafe { *(src.get_unchecked(read)) };
                            if in_inclusive_range8(byte, 0xE0, 0xEF) {
                                continue 'three;
                            }
                            if likely(byte < 0x80) {
                                read += 1;
                                continue 'outer;
                            }
                            continue 'inner;
                        }
                        break 'inner;
                    }
                }
                let second = unsafe { *(src.get_unchecked(read + 1)) };
                let third = unsafe { *(src.get_unchecked(read + 2)) };
                let fourth = unsafe { *(src.get_unchecked(read + 3)) };
                if (u16::from(
                    UTF8_DATA.table[usize::from(second)]
                        & unsafe { *(UTF8_DATA.table.get_unchecked(byte as usize + 0x80)) },
                ) | u16::from(third >> 6)
                    | (u16::from(fourth & 0xC0) << 2))
                    != 0x202
                {
                    break 'outer;
                }
                read += 4;
                if likely(read + 4 <= src.len()) {
                    byte = unsafe { *(src.get_unchecked(read)) };
                    if byte < 0x80 {
                        read += 1;
                        continue 'outer;
                    }
                    continue 'inner;
                }
                break 'inner;
            }
        }
        'tail: loop {
            if read >= src.len() {
                break 'outer;
            }
            byte = src[read];
            if byte < 0x80 {
                read += 1;
                continue 'tail;
            }
            if in_inclusive_range8(byte, 0xC2, 0xDF) {
                let new_read = read + 2;
                if new_read > src.len() {
                    break 'outer;
                }
                let second = src[read + 1];
                if !in_inclusive_range8(second, 0x80, 0xBF) {
                    break 'outer;
                }
                read += 2;
                continue 'tail;
            }
            if byte < 0xF0 {
                let new_read = read + 3;
                if new_read > src.len() {
                    break 'outer;
                }
                let second = src[read + 1];
                let third = src[read + 2];
                if ((UTF8_DATA.table[usize::from(second)]
                    & unsafe { *(UTF8_DATA.table.get_unchecked(byte as usize + 0x80)) })
                    | (third >> 6))
                    != 2
                {
                    break 'outer;
                }
                read += 3;
                break 'outer;
            }
            break 'outer;
        }
    }
    read
}

pub struct Utf8Decoder {
    code_point: u32,
    bytes_seen: usize,
    bytes_needed: usize,
    lower_boundary: u8,
    upper_boundary: u8,
}
impl Utf8Decoder {
    pub fn new_inner() -> Utf8Decoder {
        Utf8Decoder {
            code_point: 0,
            bytes_seen: 0,
            bytes_needed: 0,
            lower_boundary: 0x80u8,
            upper_boundary: 0xBFu8,
        }
    }
    pub fn new() -> VariantDecoder {
        VariantDecoder::Utf8(Utf8Decoder::new_inner())
    }

    fn extra_from_state(&self) -> usize {
        if self.bytes_needed == 0 {
            0
        } else {
            self.bytes_seen + 1
        }
    }
    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_add(3 + self.extra_from_state())
    }
    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        checked_add(
            3,
            checked_mul(3, byte_length.checked_add(self.extra_from_state())),
        )
    }
    decoder_functions!(
        preamble = {},
        loop_preamble = {
            if self.bytes_needed == 0 {
                dest.copy_utf8_up_to_invalid_from(&mut source);
            }
        },
        eof = {
            if self.bytes_needed != 0 {
                let bad_bytes = (self.bytes_seen + 1) as u8;
                self.code_point = 0;
                self.bytes_needed = 0;
                self.bytes_seen = 0;
                return (
                    DecoderResult::Malformed(bad_bytes, 0),
                    src_consumed,
                    dest.written(),
                );
            }
        },
        body = {
            if self.bytes_needed == 0 {
                if b < 0x80u8 {
                    destination_handle.write_ascii(b);
                    continue;
                }
                if b < 0xC2u8 {
                    return (
                        DecoderResult::Malformed(1, 0),
                        unread_handle.consumed(),
                        destination_handle.written(),
                    );
                }
                if b < 0xE0u8 {
                    self.bytes_needed = 1;
                    self.code_point = u32::from(b) & 0x1F;
                    continue;
                }
                if b < 0xF0u8 {
                    if b == 0xE0u8 {
                        self.lower_boundary = 0xA0u8;
                    } else if b == 0xEDu8 {
                        self.upper_boundary = 0x9Fu8;
                    }
                    self.bytes_needed = 2;
                    self.code_point = u32::from(b) & 0xF;
                    continue;
                }
                if b < 0xF5u8 {
                    if b == 0xF0u8 {
                        self.lower_boundary = 0x90u8;
                    } else if b == 0xF4u8 {
                        self.upper_boundary = 0x8Fu8;
                    }
                    self.bytes_needed = 3;
                    self.code_point = u32::from(b) & 0x7;
                    continue;
                }
                return (
                    DecoderResult::Malformed(1, 0),
                    unread_handle.consumed(),
                    destination_handle.written(),
                );
            }
            if !(b >= self.lower_boundary && b <= self.upper_boundary) {
                let bad_bytes = (self.bytes_seen + 1) as u8;
                self.code_point = 0;
                self.bytes_needed = 0;
                self.bytes_seen = 0;
                self.lower_boundary = 0x80u8;
                self.upper_boundary = 0xBFu8;
                return (
                    DecoderResult::Malformed(bad_bytes, 0),
                    unread_handle.unread(),
                    destination_handle.written(),
                );
            }
            self.lower_boundary = 0x80u8;
            self.upper_boundary = 0xBFu8;
            self.code_point = (self.code_point << 6) | (u32::from(b) & 0x3F);
            self.bytes_seen += 1;
            if self.bytes_seen != self.bytes_needed {
                continue;
            }
            if self.bytes_needed == 3 {
                destination_handle.write_astral(self.code_point);
            } else {
                destination_handle.write_bmp_excl_ascii(self.code_point as u16);
            }
            self.code_point = 0;
            self.bytes_needed = 0;
            self.bytes_seen = 0;
            continue;
        },
        self = self,
        src_consumed = src_consumed,
        dest = dest,
        source = source,
        byte = b,
        destination_handle = destination_handle,
        unread_handle = unread_handle,
        destination_check = check_space_astral
    );
}
