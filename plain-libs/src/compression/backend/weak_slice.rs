use core::marker::PhantomData;
#[derive(Debug)]
pub(crate) struct WeakSliceMut<'a, T> {
    ptr: *mut T,
    len: usize,
    _marker: PhantomData<&'a mut [T]>,
}
impl<'a, T> WeakSliceMut<'a, T> {
    pub(crate) unsafe fn from_raw_parts_mut(ptr: *mut T, len: usize) -> Self {
        {
            Self {
                ptr,
                len,
                _marker: PhantomData,
            }
        }
    }

    pub(crate) fn as_slice(&self) -> &'a [T] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
    pub(crate) fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
    pub(crate) fn as_ptr(&self) -> *const T {
        self.ptr
    }
    pub(crate) fn as_mut_ptr(&mut self) -> *mut T {
        self.ptr
    }
    pub(crate) fn len(&self) -> usize {
        self.len
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(crate) fn empty() -> Self {
        let buf = &mut [];
        Self {
            ptr: buf.as_mut_ptr(),
            len: buf.len(),
            _marker: PhantomData,
        }
    }
}
#[derive(Debug)]
pub(crate) struct WeakArrayMut<'a, T, const N: usize> {
    ptr: *mut [T; N],
    _marker: PhantomData<&'a mut [T; N]>,
}
impl<'a, T, const N: usize> WeakArrayMut<'a, T, N> {
    pub(crate) unsafe fn from_ptr(ptr: *mut [T; N]) -> Self {
        Self {
            ptr,
            _marker: PhantomData,
        }
    }
    pub(crate) fn as_slice(&self) -> &'a [T] {
        unsafe { core::slice::from_raw_parts(self.ptr.cast(), N) }
    }
    pub(crate) fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.cast(), N) }
    }
    pub(crate) fn as_ptr(&self) -> *const [T; N] {
        self.ptr
    }
}
