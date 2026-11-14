use crate::regex_engine::int::{U16, U32, U64, Usize};
use std::vec::Vec;
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
pub(crate) struct SmallIndex(u32);
impl SmallIndex {
    #[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
    pub const MAX: SmallIndex = SmallIndex::new_unchecked(core::i32::MAX as usize - 1);
    #[cfg(target_pointer_width = "16")]
    pub const MAX: SmallIndex = SmallIndex::new_unchecked(core::isize::MAX - 1);
    pub const LIMIT: usize = SmallIndex::MAX.as_usize() + 1;
    pub const ZERO: SmallIndex = SmallIndex::new_unchecked(0);
    pub const SIZE: usize = core::mem::size_of::<SmallIndex>();
    #[inline]
    pub fn new(index: usize) -> Result<SmallIndex, SmallIndexError> {
        SmallIndex::try_from(index)
    }
    #[inline]
    pub const fn new_unchecked(index: usize) -> SmallIndex {
        SmallIndex::from_u32_unchecked(index as u32)
    }
    #[inline]
    pub const fn from_u32_unchecked(index: u32) -> SmallIndex {
        SmallIndex(index)
    }
    #[inline]
    pub fn must(index: usize) -> SmallIndex {
        SmallIndex::new(index).expect("invalid small index")
    }
    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }
    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0 as u64
    }
    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.0
    }
    #[inline]
    pub const fn as_i32(&self) -> i32 {
        self.0 as i32
    }
    #[inline]
    pub fn one_more(&self) -> usize {
        self.as_usize() + 1
    }
    #[inline]
    pub fn from_ne_bytes(bytes: [u8; 4]) -> Result<SmallIndex, SmallIndexError> {
        let id = u32::from_ne_bytes(bytes);
        if id > SmallIndex::MAX.as_u32() {
            return Err(SmallIndexError {
                attempted: u64::from(id),
            });
        }
        Ok(SmallIndex::new_unchecked(id.as_usize()))
    }
    #[inline]
    pub fn from_ne_bytes_unchecked(bytes: [u8; 4]) -> SmallIndex {
        SmallIndex::new_unchecked(u32::from_ne_bytes(bytes).as_usize())
    }
    #[inline]
    pub fn to_ne_bytes(&self) -> [u8; 4] {
        self.0.to_ne_bytes()
    }
}
impl<T> core::ops::Index<SmallIndex> for [T] {
    type Output = T;
    #[inline]
    fn index(&self, index: SmallIndex) -> &T {
        &self[index.as_usize()]
    }
}
impl<T> core::ops::IndexMut<SmallIndex> for [T] {
    #[inline]
    fn index_mut(&mut self, index: SmallIndex) -> &mut T {
        &mut self[index.as_usize()]
    }
}
impl<T> core::ops::Index<SmallIndex> for Vec<T> {
    type Output = T;
    #[inline]
    fn index(&self, index: SmallIndex) -> &T {
        &self[index.as_usize()]
    }
}
impl<T> core::ops::IndexMut<SmallIndex> for Vec<T> {
    #[inline]
    fn index_mut(&mut self, index: SmallIndex) -> &mut T {
        &mut self[index.as_usize()]
    }
}
impl From<StateID> for SmallIndex {
    fn from(sid: StateID) -> SmallIndex {
        sid.0
    }
}
impl From<PatternID> for SmallIndex {
    fn from(pid: PatternID) -> SmallIndex {
        pid.0
    }
}
impl From<u8> for SmallIndex {
    fn from(index: u8) -> SmallIndex {
        SmallIndex::new_unchecked(usize::from(index))
    }
}
impl TryFrom<u16> for SmallIndex {
    type Error = SmallIndexError;
    fn try_from(index: u16) -> Result<SmallIndex, SmallIndexError> {
        if u32::from(index) > SmallIndex::MAX.as_u32() {
            return Err(SmallIndexError {
                attempted: u64::from(index),
            });
        }
        Ok(SmallIndex::new_unchecked(index.as_usize()))
    }
}
impl TryFrom<u32> for SmallIndex {
    type Error = SmallIndexError;
    fn try_from(index: u32) -> Result<SmallIndex, SmallIndexError> {
        if index > SmallIndex::MAX.as_u32() {
            return Err(SmallIndexError {
                attempted: u64::from(index),
            });
        }
        Ok(SmallIndex::new_unchecked(index.as_usize()))
    }
}
impl TryFrom<u64> for SmallIndex {
    type Error = SmallIndexError;
    fn try_from(index: u64) -> Result<SmallIndex, SmallIndexError> {
        if index > SmallIndex::MAX.as_u64() {
            return Err(SmallIndexError { attempted: index });
        }
        Ok(SmallIndex::new_unchecked(index.as_usize()))
    }
}
impl TryFrom<usize> for SmallIndex {
    type Error = SmallIndexError;
    fn try_from(index: usize) -> Result<SmallIndex, SmallIndexError> {
        if index > SmallIndex::MAX.as_usize() {
            return Err(SmallIndexError {
                attempted: index.as_u64(),
            });
        }
        Ok(SmallIndex::new_unchecked(index))
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmallIndexError {
    attempted: u64,
}
impl SmallIndexError {
    pub fn attempted(&self) -> u64 {
        self.attempted
    }
}
impl std::error::Error for SmallIndexError {}
impl core::fmt::Display for SmallIndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "failed to create small index from {:?}, which exceeds {:?}",
            self.attempted(),
            SmallIndex::MAX,
        )
    }
}
#[derive(Clone, Debug)]
pub(crate) struct SmallIndexIter {
    rng: core::ops::Range<usize>,
}
impl Iterator for SmallIndexIter {
    type Item = SmallIndex;
    fn next(&mut self) -> Option<SmallIndex> {
        if self.rng.start >= self.rng.end {
            return None;
        }
        let next_id = self.rng.start + 1;
        let id = core::mem::replace(&mut self.rng.start, next_id);
        Some(SmallIndex::new_unchecked(id))
    }
}
macro_rules! index_type_impls {
    ($ name : ident , $ err : ident , $ iter : ident , $ withiter : ident) => {
        impl $name {
            pub const MAX: $name = $name(SmallIndex::MAX);
            pub const LIMIT: usize = SmallIndex::LIMIT;
            pub const ZERO: $name = $name(SmallIndex::ZERO);
            pub const SIZE: usize = SmallIndex::SIZE;
            #[inline]
            pub fn new(value: usize) -> Result<$name, $err> {
                SmallIndex::new(value).map($name).map_err($err)
            }
            #[inline]
            pub const fn new_unchecked(value: usize) -> $name {
                $name(SmallIndex::new_unchecked(value))
            }
            #[inline]
            pub const fn from_u32_unchecked(index: u32) -> $name {
                $name(SmallIndex::from_u32_unchecked(index))
            }
            #[inline]
            pub fn must(value: usize) -> $name {
                $name::new(value).expect(concat!("invalid ", stringify!($name), " value"))
            }
            #[inline]
            pub const fn as_usize(&self) -> usize {
                self.0.as_usize()
            }
            #[inline]
            pub const fn as_u64(&self) -> u64 {
                self.0.as_u64()
            }
            #[inline]
            pub const fn as_u32(&self) -> u32 {
                self.0.as_u32()
            }
            #[inline]
            pub const fn as_i32(&self) -> i32 {
                self.0.as_i32()
            }
            #[inline]
            pub fn one_more(&self) -> usize {
                self.0.one_more()
            }
            #[inline]
            pub fn from_ne_bytes(bytes: [u8; 4]) -> Result<$name, $err> {
                SmallIndex::from_ne_bytes(bytes).map($name).map_err($err)
            }
            #[inline]
            pub fn from_ne_bytes_unchecked(bytes: [u8; 4]) -> $name {
                $name(SmallIndex::from_ne_bytes_unchecked(bytes))
            }
            #[inline]
            pub fn to_ne_bytes(&self) -> [u8; 4] {
                self.0.to_ne_bytes()
            }
            pub(crate) fn iter(len: usize) -> $iter {
                $iter::new(len)
            }
        }
        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.as_u32())
                    .finish()
            }
        }
        impl<T> core::ops::Index<$name> for [T] {
            type Output = T;
            #[inline]
            fn index(&self, index: $name) -> &T {
                &self[index.as_usize()]
            }
        }
        impl<T> core::ops::IndexMut<$name> for [T] {
            #[inline]
            fn index_mut(&mut self, index: $name) -> &mut T {
                &mut self[index.as_usize()]
            }
        }
        impl<T> core::ops::Index<$name> for Vec<T> {
            type Output = T;
            #[inline]
            fn index(&self, index: $name) -> &T {
                &self[index.as_usize()]
            }
        }
        impl<T> core::ops::IndexMut<$name> for Vec<T> {
            #[inline]
            fn index_mut(&mut self, index: $name) -> &mut T {
                &mut self[index.as_usize()]
            }
        }
        impl From<SmallIndex> for $name {
            fn from(index: SmallIndex) -> $name {
                $name(index)
            }
        }
        impl From<u8> for $name {
            fn from(value: u8) -> $name {
                $name(SmallIndex::from(value))
            }
        }
        impl TryFrom<u16> for $name {
            type Error = $err;
            fn try_from(value: u16) -> Result<$name, $err> {
                SmallIndex::try_from(value).map($name).map_err($err)
            }
        }
        impl TryFrom<u32> for $name {
            type Error = $err;
            fn try_from(value: u32) -> Result<$name, $err> {
                SmallIndex::try_from(value).map($name).map_err($err)
            }
        }
        impl TryFrom<u64> for $name {
            type Error = $err;
            fn try_from(value: u64) -> Result<$name, $err> {
                SmallIndex::try_from(value).map($name).map_err($err)
            }
        }
        impl TryFrom<usize> for $name {
            type Error = $err;
            fn try_from(value: usize) -> Result<$name, $err> {
                SmallIndex::try_from(value).map($name).map_err($err)
            }
        }
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $err(SmallIndexError);
        impl $err {
            pub fn attempted(&self) -> u64 {
                self.0.attempted()
            }
        }
        impl std::error::Error for $err {}
        impl core::fmt::Display for $err {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                write!(
                    f,
                    "failed to create {} from {:?}, which exceeds {:?}",
                    stringify!($name),
                    self.attempted(),
                    $name::MAX,
                )
            }
        }
        #[derive(Clone, Debug)]
        pub(crate) struct $iter(SmallIndexIter);
        impl $iter {
            fn new(len: usize) -> $iter {
                assert!(
                    len <= $name::LIMIT,
                    "cannot create iterator for {} when number of \
                     elements exceed {:?}",
                    stringify!($name),
                    $name::LIMIT,
                );
                $iter(SmallIndexIter { rng: 0..len })
            }
        }
        impl Iterator for $iter {
            type Item = $name;
            fn next(&mut self) -> Option<$name> {
                self.0.next().map($name)
            }
        }
        #[derive(Clone, Debug)]
        pub(crate) struct $withiter<I> {
            it: I,
            ids: $iter,
        }
        impl<I: Iterator + ExactSizeIterator> $withiter<I> {
            fn new(it: I) -> $withiter<I> {
                let ids = $name::iter(it.len());
                $withiter { it, ids }
            }
        }
        impl<I: Iterator + ExactSizeIterator> Iterator for $withiter<I> {
            type Item = ($name, I::Item);
            fn next(&mut self) -> Option<($name, I::Item)> {
                let item = self.it.next()?;
                let id = self.ids.next().unwrap();
                Some((id, item))
            }
        }
    };
}
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PatternID(SmallIndex);
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct StateID(SmallIndex);
index_type_impls!(PatternID, PatternIDError, PatternIDIter, WithPatternIDIter);
index_type_impls!(StateID, StateIDError, StateIDIter, WithStateIDIter);
pub(crate) trait IteratorIndexExt: Iterator {
    fn with_pattern_ids(self) -> WithPatternIDIter<Self>
    where
        Self: Sized + ExactSizeIterator,
    {
        WithPatternIDIter::new(self)
    }
    fn with_state_ids(self) -> WithStateIDIter<Self>
    where
        Self: Sized + ExactSizeIterator,
    {
        WithStateIDIter::new(self)
    }
}
impl<I: Iterator> IteratorIndexExt for I {}
