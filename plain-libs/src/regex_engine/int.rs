// Shared conversions from aho-corasick and regex-automata; see aho/SOURCE.json and automata/SOURCE.json.
pub(crate) trait U8 {
    fn as_usize(self) -> usize;
}
impl U8 for u8 {
    fn as_usize(self) -> usize {
        usize::from(self)
    }
}
pub(crate) trait U16 {
    fn as_usize(self) -> usize;
    fn low_u8(self) -> u8;
    fn high_u8(self) -> u8;
}
impl U16 for u16 {
    fn as_usize(self) -> usize {
        usize::from(self)
    }
    fn low_u8(self) -> u8 {
        self as u8
    }
    fn high_u8(self) -> u8 {
        (self >> 8) as u8
    }
}
pub(crate) trait U32 {
    fn as_usize(self) -> usize;
    fn low_u8(self) -> u8;
    fn low_u16(self) -> u16;
    fn high_u16(self) -> u16;
}
impl U32 for u32 {
    #[inline]
    fn as_usize(self) -> usize {
        #[cfg(debug_assertions)]
        {
            usize::try_from(self).expect("u32 overflowed usize")
        }
        #[cfg(not(debug_assertions))]
        {
            self as usize
        }
    }
    fn low_u8(self) -> u8 {
        self as u8
    }
    fn low_u16(self) -> u16 {
        self as u16
    }
    fn high_u16(self) -> u16 {
        (self >> 16) as u16
    }
}
pub(crate) trait U64 {
    fn as_usize(self) -> usize;
    fn low_u8(self) -> u8;
    fn low_u16(self) -> u16;
    fn low_u32(self) -> u32;
    fn high_u32(self) -> u32;
}
impl U64 for u64 {
    fn as_usize(self) -> usize {
        #[cfg(debug_assertions)]
        {
            usize::try_from(self).expect("u64 overflowed usize")
        }
        #[cfg(not(debug_assertions))]
        {
            self as usize
        }
    }
    fn low_u8(self) -> u8 {
        self as u8
    }
    fn low_u16(self) -> u16 {
        self as u16
    }
    fn low_u32(self) -> u32 {
        self as u32
    }
    fn high_u32(self) -> u32 {
        (self >> 32) as u32
    }
}
pub(crate) trait I8 {
    fn as_usize(self) -> usize;
    fn to_bits(self) -> u8;
    fn from_bits(n: u8) -> i8;
}
impl I8 for i8 {
    fn as_usize(self) -> usize {
        #[cfg(debug_assertions)]
        {
            usize::try_from(self).expect("i8 overflowed usize")
        }
        #[cfg(not(debug_assertions))]
        {
            self as usize
        }
    }
    fn to_bits(self) -> u8 {
        self as u8
    }
    fn from_bits(n: u8) -> i8 {
        n as i8
    }
}
pub(crate) trait I32 {
    fn as_usize(self) -> usize;
    fn to_bits(self) -> u32;
    fn from_bits(n: u32) -> i32;
}
impl I32 for i32 {
    fn as_usize(self) -> usize {
        #[cfg(debug_assertions)]
        {
            usize::try_from(self).expect("i32 overflowed usize")
        }
        #[cfg(not(debug_assertions))]
        {
            self as usize
        }
    }
    fn to_bits(self) -> u32 {
        self as u32
    }
    fn from_bits(n: u32) -> i32 {
        n as i32
    }
}
pub(crate) trait I64 {
    fn as_usize(self) -> usize;
    fn to_bits(self) -> u64;
    fn from_bits(n: u64) -> i64;
}
impl I64 for i64 {
    fn as_usize(self) -> usize {
        #[cfg(debug_assertions)]
        {
            usize::try_from(self).expect("i64 overflowed usize")
        }
        #[cfg(not(debug_assertions))]
        {
            self as usize
        }
    }
    fn to_bits(self) -> u64 {
        self as u64
    }
    fn from_bits(n: u64) -> i64 {
        n as i64
    }
}
pub(crate) trait Usize {
    fn as_u8(self) -> u8;
    fn as_u16(self) -> u16;
    fn as_u32(self) -> u32;
    fn as_u64(self) -> u64;
}
impl Usize for usize {
    fn as_u8(self) -> u8 {
        #[cfg(debug_assertions)]
        {
            u8::try_from(self).expect("usize overflowed u8")
        }
        #[cfg(not(debug_assertions))]
        {
            self as u8
        }
    }
    fn as_u16(self) -> u16 {
        #[cfg(debug_assertions)]
        {
            u16::try_from(self).expect("usize overflowed u16")
        }
        #[cfg(not(debug_assertions))]
        {
            self as u16
        }
    }
    fn as_u32(self) -> u32 {
        #[cfg(debug_assertions)]
        {
            u32::try_from(self).expect("usize overflowed u32")
        }
        #[cfg(not(debug_assertions))]
        {
            self as u32
        }
    }
    fn as_u64(self) -> u64 {
        #[cfg(debug_assertions)]
        {
            u64::try_from(self).expect("usize overflowed u64")
        }
        #[cfg(not(debug_assertions))]
        {
            self as u64
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/regex_engine/int.rs"]
mod tests;
