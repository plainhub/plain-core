use core::convert::TryFrom;
use core::convert::TryInto;
pub struct Bytes<'a> {
    start: *const u8,
    end: *const u8,
    cursor: *const u8,
    phantom: core::marker::PhantomData<&'a ()>,
}
impl<'a> Bytes<'a> {
    #[inline]
    pub fn new(slice: &'a [u8]) -> Bytes<'a> {
        let start = slice.as_ptr();
        let end = unsafe { start.add(slice.len()) };
        let cursor = start;
        Bytes {
            start,
            end,
            cursor,
            phantom: core::marker::PhantomData,
        }
    }
    #[inline]
    pub fn pos(&self) -> usize {
        self.cursor as usize - self.start as usize
    }
    #[inline]
    pub fn peek(&self) -> Option<u8> {
        if self.cursor < self.end {
            Some(unsafe { *self.cursor })
        } else {
            None
        }
    }
    #[inline]
    pub unsafe fn peek_ahead(&self, n: usize) -> Option<u8> {
        debug_assert!(n <= self.len());
        let p = unsafe { self.cursor.add(n) };
        if p < self.end {
            Some(unsafe { *p })
        } else {
            None
        }
    }
    #[inline]
    pub fn peek_n<'b: 'a, U: TryFrom<&'a [u8]>>(&'b self, n: usize) -> Option<U> {
        self.as_ref().get(..n)?.try_into().ok()
    }
    #[inline]
    pub unsafe fn bump(&mut self) {
        unsafe { self.advance(1) }
    }
    #[inline]
    pub unsafe fn advance(&mut self, n: usize) {
        unsafe {
            self.cursor = self.cursor.add(n);
            debug_assert!(self.cursor <= self.end, "overflow");
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.end as usize - self.cursor as usize
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub fn slice(&mut self) -> &'a [u8] {
        let slice = unsafe { slice_from_ptr_range(self.start, self.cursor) };
        self.commit();
        slice
    }
    #[inline]
    pub unsafe fn slice_skip(&mut self, skip: usize) -> &'a [u8] {
        unsafe {
            debug_assert!(skip <= self.cursor.offset_from(self.start) as usize);
            let head = slice_from_ptr_range(self.start, self.cursor.sub(skip));
            self.commit();
            head
        }
    }
    #[inline]
    pub fn commit(&mut self) {
        self.start = self.cursor
    }

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.cursor
    }
    #[inline]
    pub fn start(&self) -> *const u8 {
        self.start
    }
    #[inline]
    pub fn end(&self) -> *const u8 {
        self.end
    }
}
impl AsRef<[u8]> for Bytes<'_> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        unsafe { slice_from_ptr_range(self.cursor, self.end) }
    }
}
#[inline]
unsafe fn slice_from_ptr_range<'a>(start: *const u8, end: *const u8) -> &'a [u8] {
    unsafe {
        debug_assert!(start <= end);
        core::slice::from_raw_parts(start, end as usize - start as usize)
    }
}
impl Iterator for Bytes<'_> {
    type Item = u8;
    #[inline]
    fn next(&mut self) -> Option<u8> {
        if self.cursor < self.end {
            unsafe {
                let b = *self.cursor;
                self.bump();
                Some(b)
            }
        } else {
            None
        }
    }
}
