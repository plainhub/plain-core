use super::group::{BITMASK_ITER_MASK, BITMASK_STRIDE, BitMaskWord, NonZeroBitMaskWord};
#[derive(Copy, Clone)]
pub(crate) struct BitMask(pub(crate) BitMaskWord);
impl BitMask {
    #[inline]
    #[must_use]
    fn remove_lowest_bit(self) -> Self {
        BitMask(self.0 & (self.0 - 1))
    }
    #[inline]
    pub(crate) fn any_bit_set(self) -> bool {
        self.0 != 0
    }
    #[inline]
    pub(crate) fn lowest_set_bit(self) -> Option<usize> {
        if let Some(nonzero) = NonZeroBitMaskWord::new(self.0) {
            Some(Self::nonzero_trailing_zeros(nonzero))
        } else {
            None
        }
    }
    #[inline]
    pub(crate) fn trailing_zeros(self) -> usize {
        if cfg!(target_arch = "arm") && BITMASK_STRIDE % 8 == 0 {
            self.0.swap_bytes().leading_zeros() as usize / BITMASK_STRIDE
        } else {
            self.0.trailing_zeros() as usize / BITMASK_STRIDE
        }
    }
    #[inline]
    fn nonzero_trailing_zeros(nonzero: NonZeroBitMaskWord) -> usize {
        if cfg!(target_arch = "arm") && BITMASK_STRIDE % 8 == 0 {
            let swapped = unsafe { NonZeroBitMaskWord::new_unchecked(nonzero.get().swap_bytes()) };
            swapped.leading_zeros() as usize / BITMASK_STRIDE
        } else {
            nonzero.trailing_zeros() as usize / BITMASK_STRIDE
        }
    }
    #[inline]
    pub(crate) fn leading_zeros(self) -> usize {
        self.0.leading_zeros() as usize / BITMASK_STRIDE
    }
}
impl IntoIterator for BitMask {
    type Item = usize;
    type IntoIter = BitMaskIter;
    #[inline]
    fn into_iter(self) -> BitMaskIter {
        BitMaskIter(BitMask(self.0 & BITMASK_ITER_MASK))
    }
}
#[derive(Clone)]
pub(crate) struct BitMaskIter(pub(crate) BitMask);
impl Iterator for BitMaskIter {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        let bit = self.0.lowest_set_bit()?;
        self.0 = self.0.remove_lowest_bit();
        Some(bit)
    }
}
