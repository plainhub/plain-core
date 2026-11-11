use core::ops::{Deref, DerefMut};
use std::vec;
use std::vec::Vec;
#[derive(Clone, Default)]
pub struct AlignedBytes {
    words: Vec<u128>,
    len: usize,
}
impl AlignedBytes {
    pub fn zeroed(len: usize) -> Self {
        Self {
            words: vec![0u128; (len + 15) / 16],
            len,
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
impl core::fmt::Debug for AlignedBytes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AlignedBytes({} bytes)", self.len)
    }
}
impl Deref for AlignedBytes {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &[u8] {
        &crate::byte_cast::cast_slice(&self.words)[..self.len]
    }
}
impl DerefMut for AlignedBytes {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut crate::byte_cast::cast_slice_mut(&mut self.words)[..self.len]
    }
}
