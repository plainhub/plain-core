use crate::compression::backend::weak_slice::WeakSliceMut;
use core::{marker::PhantomData, mem::MaybeUninit};
pub struct Pending<'a> {
    buf: WeakSliceMut<'a, MaybeUninit<u8>>,
    out: usize,
    pub(crate) pending: usize,
    _marker: PhantomData<&'a mut [u8]>,
}
impl<'a> Pending<'a> {
    pub fn reset_keep(&mut self) {
        self.pending = 0;
    }
    pub fn pending(&self) -> &[u8] {
        let slice = &self.buf.as_slice()[self.out..][..self.pending];
        unsafe { &*(slice as *const [MaybeUninit<u8>] as *const [u8]) }
    }
    pub(crate) fn remaining(&self) -> usize {
        self.buf.len() - (self.out + self.pending)
    }
    pub(crate) fn capacity(&self) -> usize {
        self.buf.len()
    }
    #[inline(always)]
    #[track_caller]
    pub fn advance(&mut self, number_of_bytes: usize) {
        debug_assert!(self.pending >= number_of_bytes);
        self.out = self.out.wrapping_add(number_of_bytes);
        self.pending -= number_of_bytes;
        if self.pending == 0 {
            self.out = 0;
        }
    }
    #[inline(always)]
    #[track_caller]
    pub fn rewind(&mut self, n: usize) {
        assert!(n <= self.pending, "rewinding past then start");
        self.pending -= n;
        if self.pending == 0 {
            self.out = 0;
        }
    }
    #[inline(always)]
    #[track_caller]
    pub fn extend(&mut self, buf: &[u8]) {
        assert!(
            self.remaining() >= buf.len(),
            "buf.len() must fit in remaining()"
        );
        let buf = unsafe { &*(buf as *const [u8] as *const [MaybeUninit<u8>]) };
        self.buf.as_mut_slice()[self.out + self.pending..][..buf.len()].copy_from_slice(buf);
        self.pending += buf.len();
    }
    pub(crate) unsafe fn from_raw_parts(ptr: *mut MaybeUninit<u8>, len: usize) -> Self {
        {
            let buf = unsafe { WeakSliceMut::from_raw_parts_mut(ptr, len) };
            Self {
                buf,
                out: 0,
                pending: 0,
                _marker: PhantomData,
            }
        }
    }
}
