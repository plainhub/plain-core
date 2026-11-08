// UUID parsing and formatting adapted from uuid 1.26.1 (https://github.com/uuid-rs/uuid), see LICENSE-MIT.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, str};

mod parse;

pub use parse::ParseError;

#[repr(transparent)]
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Uuid([u8; 16]);

impl Uuid {
    pub fn new_v4() -> Self {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes)
            .unwrap_or_else(|error| panic!("could not retrieve random bytes for uuid: {error}"));
        Self::from_random_bytes(bytes)
    }

    pub const fn from_random_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(bytes)
    }

    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn from_u128(value: u128) -> Self {
        Self(value.to_be_bytes())
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub const fn as_u128(&self) -> u128 {
        u128::from_be_bytes(self.0)
    }

    pub fn parse_str(value: &str) -> Result<Self, ParseError> {
        parse::parse(value).map(Self)
    }

    pub fn into_string(self) -> String {
        self.encode_lower(&mut [0; 36]).to_owned()
    }

    #[inline]
    fn encode_lower<'a>(&self, buffer: &'a mut [u8; 36]) -> &'a str {
        *buffer = format_hyphenated(&self.0);
        // SAFETY: The formatter only writes ASCII hyphens and hexadecimal digits.
        unsafe { str::from_utf8_unchecked(buffer) }
    }
}

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
#[inline]
const fn nibble_to_hex(nibble: u8) -> u8 {
    nibble + b'0' + if nibble > 9 { 0x27 } else { 0 }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline]
fn format_simple(src: &[u8; 16]) -> [u8; 32] {
    use std::arch::aarch64::*;
    let mut output = [0; 32];
    // SAFETY: The inputs each contain 16 bytes, and the output has space for both halves.
    unsafe {
        let input = vld1q_u8(src.as_ptr());
        let digits = vld1q_u8(b"0123456789abcdef".as_ptr());
        let high = vqtbl1q_u8(digits, vshrq_n_u8::<4>(input));
        let low = vqtbl1q_u8(digits, vandq_u8(input, vdupq_n_u8(15)));
        vst1q_u8(output.as_mut_ptr(), vzip1q_u8(high, low));
        vst1q_u8(output.as_mut_ptr().add(16), vzip2q_u8(high, low));
    }
    output
}

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
#[inline]
fn format_simple(src: &[u8; 16]) -> [u8; 32] {
    let mut dst = [0; 32];
    let mut i = 0;
    while i < 16 {
        let x = src[i];
        dst[i * 2] = nibble_to_hex(x >> 4);
        dst[i * 2 + 1] = nibble_to_hex(x & 0x0f);
        i += 1;
    }
    dst
}

#[inline]
fn format_hyphenated(src: &[u8; 16]) -> [u8; 36] {
    let simple = format_simple(src);
    let mut dst = [0; 36];

    let mut i = 0;
    while i < 8 {
        dst[i] = simple[i];
        i += 1;
    }
    dst[8] = b'-';
    while i < 12 {
        dst[i + 1] = simple[i];
        i += 1;
    }
    dst[13] = b'-';
    while i < 16 {
        dst[i + 2] = simple[i];
        i += 1;
    }
    dst[18] = b'-';
    while i < 20 {
        dst[i + 3] = simple[i];
        i += 1;
    }
    dst[23] = b'-';
    while i < 32 {
        dst[i + 4] = simple[i];
        i += 1;
    }
    dst
}

impl str::FromStr for Uuid {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_str(value)
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.encode_lower(&mut [0; 36]))
    }
}

impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Serialize for Uuid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(self.encode_lower(&mut [0; 36]))
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Uuid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(UuidVisitor { readable: true })
        } else {
            deserializer.deserialize_bytes(UuidVisitor { readable: false })
        }
    }
}

struct UuidVisitor {
    readable: bool,
}

impl<'de> de::Visitor<'de> for UuidVisitor {
    type Value = Uuid;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.readable {
            "a formatted UUID string"
        } else {
            "a 16 byte array"
        })
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Uuid, E> {
        if !self.readable {
            return Err(E::invalid_type(de::Unexpected::Str(value), &self));
        }
        Uuid::parse_str(value)
            .map_err(|error| E::custom(format_args!("UUID parsing failed: {error}")))
    }

    fn visit_bytes<E: de::Error>(self, value: &[u8]) -> Result<Uuid, E> {
        let bytes = value.try_into().map_err(|_| {
            E::custom(format_args!(
                "UUID parsing failed: invalid length: expected 16 bytes, found {}",
                value.len()
            ))
        })?;
        Ok(Uuid::from_bytes(bytes))
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Uuid, A::Error> {
        if !self.readable {
            return Err(de::Error::invalid_type(de::Unexpected::Seq, &self));
        }
        let mut bytes = [0; 16];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = sequence
                .next_element()?
                .ok_or_else(|| de::Error::invalid_length(index, &self))?;
        }
        Ok(Uuid::from_bytes(bytes))
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/utils/uuid.rs"]
mod tests;
