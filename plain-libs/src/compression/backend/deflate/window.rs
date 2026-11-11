use crate::compression::backend::weak_slice::WeakSliceMut;
#[derive(Debug)]
pub struct Window<'a> {
    buf: WeakSliceMut<'a, u8>,
}
impl<'a> Window<'a> {
    pub unsafe fn from_raw_parts(ptr: *mut u8, window_bits: usize) -> Self {
        {
            let len = (1 << window_bits) * 2;
            let buf = unsafe { WeakSliceMut::from_raw_parts_mut(ptr, len) };
            Self { buf }
        }
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.buf.as_ptr()
    }

    #[inline]
    pub fn filled(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.buf.as_ptr().cast(), self.buf.len()) }
    }
    #[inline]
    pub fn filled_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.buf.as_mut_ptr().cast(), self.buf.len()) }
    }
    pub unsafe fn copy_and_initialize(&mut self, range: core::ops::Range<usize>, src: *const u8) {
        {
            let (start, end) = (range.start, range.end);
            let dst = self.buf.as_mut_slice()[range].as_mut_ptr();
            unsafe { core::ptr::copy_nonoverlapping(src, dst, end - start) };
        }
    }
}
