use crate::rusqlite::ffi;
use std::ffi::{CStr, c_char};
use std::marker::PhantomData;
use std::ptr::NonNull;
pub(crate) fn alloc(s: &str) -> *mut c_char {
    SqliteMallocString::from_str(s).into_raw()
}
#[repr(transparent)]
pub(crate) struct SqliteMallocString {
    ptr: NonNull<c_char>,
    _boo: PhantomData<Box<[c_char]>>,
}
impl SqliteMallocString {
    #[inline]
    pub(crate) unsafe fn from_raw_nonnull(ptr: NonNull<c_char>) -> Self {
        Self {
            ptr,
            _boo: PhantomData,
        }
    }
    #[inline]
    pub(crate) unsafe fn from_raw(ptr: *mut c_char) -> Option<Self> {
        unsafe { NonNull::new(ptr).map(|p| Self::from_raw_nonnull(p)) }
    }
    #[inline]
    pub(crate) fn into_inner(self) -> NonNull<c_char> {
        let p = self.ptr;
        std::mem::forget(self);
        p
    }
    #[inline]
    pub(crate) fn into_raw(self) -> *mut c_char {
        self.into_inner().as_ptr()
    }
    #[inline]
    pub(crate) fn as_ptr(&self) -> *const c_char {
        self.ptr.as_ptr()
    }
    #[inline]
    pub(crate) fn as_cstr(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.as_ptr()) }
    }
    #[inline]
    pub(crate) fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        self.as_cstr().to_string_lossy()
    }
    pub(crate) fn from_str(s: &str) -> Self {
        let s = if s.as_bytes().contains(&0) {
            std::borrow::Cow::Owned(make_nonnull(s))
        } else {
            std::borrow::Cow::Borrowed(s)
        };
        debug_assert!(!s.as_bytes().contains(&0));
        let bytes: &[u8] = s.as_ref().as_bytes();
        let src_ptr: *const c_char = bytes.as_ptr().cast();
        let src_len = bytes.len();
        let maybe_len_plus_1 = s.len().checked_add(1).and_then(|v| v.try_into().ok());
        unsafe {
            let res_ptr = maybe_len_plus_1
                .and_then(|len_to_alloc| {
                    debug_assert!(len_to_alloc > 0);
                    debug_assert_eq!((len_to_alloc - 1) as usize, src_len);
                    NonNull::new(ffi::sqlite3_malloc64(len_to_alloc).cast::<c_char>())
                })
                .unwrap_or_else(|| {
                    use std::alloc::{Layout, handle_alloc_error};
                    let len = s.len().saturating_add(1).min(isize::MAX as usize);
                    let layout = Layout::from_size_align_unchecked(len, 1);
                    handle_alloc_error(layout);
                });
            let buf: *mut c_char = res_ptr.as_ptr().cast::<c_char>();
            src_ptr.copy_to_nonoverlapping(buf, src_len);
            buf.add(src_len).write(0);
            debug_assert_eq!(CStr::from_ptr(res_ptr.as_ptr()).to_bytes(), bytes);
            Self::from_raw_nonnull(res_ptr)
        }
    }
}
const NUL_REPLACE: &str = "␀";
#[cold]
fn make_nonnull(v: &str) -> String {
    v.replace('\0', NUL_REPLACE)
}
impl Drop for SqliteMallocString {
    #[inline]
    fn drop(&mut self) {
        unsafe { ffi::sqlite3_free(self.ptr.as_ptr().cast()) };
    }
}
