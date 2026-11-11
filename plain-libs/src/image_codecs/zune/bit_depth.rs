use core::cmp::Ordering;
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
#[non_exhaustive]
pub enum BitDepth {
    Eight,
    Sixteen,
    Float32,
    #[default]
    Unknown,
}
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BitType {
    U8,
    U16,
    F32,
}
impl BitType {}
impl core::cmp::PartialOrd for BitType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl core::cmp::Ord for BitType {
    fn cmp(&self, other: &Self) -> Ordering {
        if self == other {
            core::cmp::Ordering::Equal
        } else if *self == BitType::U8 {
            core::cmp::Ordering::Less
        } else if *self == BitType::U16 && *other == BitType::F32 {
            core::cmp::Ordering::Less
        } else if *self == BitType::F32 {
            core::cmp::Ordering::Greater
        } else {
            unreachable!()
        }
    }
}
impl BitDepth {
    # [rustfmt :: skip]    pub const fn max_value (self) -> u16 { match self { Self :: Eight => (1 << 08) - 1 , Self :: Sixteen => u16 :: MAX , Self :: Float32 => 1 , Self :: Unknown => 0 , } }

    pub const fn size_of(self) -> usize {
        match self {
            Self::Eight => core::mem::size_of::<u8>(),
            Self::Sixteen => core::mem::size_of::<u16>(),
            Self::Float32 => core::mem::size_of::<f32>(),
            Self::Unknown => panic!("Unknown bit type"),
        }
    }
}
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ByteEndian {
    LE,
    BE,
}
