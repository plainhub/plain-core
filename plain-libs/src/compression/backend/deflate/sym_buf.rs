use crate::compression::backend::weak_slice::WeakSliceMut;
pub(crate) struct SymBuf<'a> {
    buf: WeakSliceMut<'a, u8>,
    filled: usize,
}
impl<'a> SymBuf<'a> {
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (u16, u8)> + '_ {
        self.buf.as_slice()[..self.filled]
            .chunks_exact(3)
            .map(|chunk| match *chunk {
                [dist_low, dist_high, lc] => (u16::from_le_bytes([dist_low, dist_high]), lc),
                _ => unreachable!("chunks are exactly 3 elements wide"),
            })
    }
    #[inline]
    pub fn should_flush_block(&self) -> bool {
        self.filled == self.buf.len() - 3
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.filled == 0
    }
    #[inline]
    pub fn clear(&mut self) {
        self.buf.as_mut_slice().fill(0);
        self.filled = 0;
    }
    #[inline(always)]
    pub fn push_lit(&mut self, byte: u8) {
        self.buf.as_mut_slice()[self.filled + 2] = byte;
        self.filled += 3;
    }
    #[inline(always)]
    pub fn push_dist(&mut self, dist: u16, len: u8) {
        let buf = &mut self.buf.as_mut_slice()[self.filled..][..3];
        unsafe {
            core::ptr::write_unaligned(buf.as_mut_ptr().cast::<[u8; 2]>(), dist.to_le_bytes())
        };
        buf[2] = len;
        self.filled += 3;
    }
    pub(crate) unsafe fn from_raw_parts(ptr: *mut u8, lit_bufsize: usize) -> Self {
        {
            Self {
                buf: unsafe { WeakSliceMut::from_raw_parts_mut(ptr, lit_bufsize * 3) },
                filled: 0,
            }
        }
    }
}
