pub(crate) trait Pointer {
    unsafe fn distance(self, origin: Self) -> usize;
    fn as_usize(self) -> usize;
}
impl<T> Pointer for *const T {
    unsafe fn distance(self, origin: *const T) -> usize {
        unsafe { usize::try_from(self.offset_from(origin)).unwrap_unchecked() }
    }
    fn as_usize(self) -> usize {
        self as usize
    }
}
impl<T> Pointer for *mut T {
    unsafe fn distance(self, origin: *mut T) -> usize {
        unsafe { (self as *const T).distance(origin as *const T) }
    }
    fn as_usize(self) -> usize {
        (self as *const T).as_usize()
    }
}
