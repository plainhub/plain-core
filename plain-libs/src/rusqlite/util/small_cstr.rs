use smallvec::{SmallVec, smallvec};
use std::ffi::{CStr, CString, NulError};
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SmallCString(SmallVec<[u8; 16]>);
impl SmallCString {
    #[inline]
    pub fn new(s: &str) -> Result<Self, NulError> {
        if s.as_bytes().contains(&0_u8) {
            return Err(Self::fabricate_nul_error(s));
        }
        let mut buf = SmallVec::with_capacity(s.len() + 1);
        buf.extend_from_slice(s.as_bytes());
        buf.push(0);
        let res = Self(buf);
        res.debug_checks();
        Ok(res)
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        self.debug_checks();
        unsafe { std::str::from_utf8_unchecked(self.as_bytes_without_nul()) }
    }
    #[inline]
    pub fn as_bytes_without_nul(&self) -> &[u8] {
        self.debug_checks();
        &self.0[..self.len()]
    }
    #[inline]
    pub fn as_bytes_with_nul(&self) -> &[u8] {
        self.debug_checks();
        &self.0
    }
    #[inline]
    #[cfg(debug_assertions)]
    fn debug_checks(&self) {
        debug_assert_ne!(self.0.len(), 0);
        debug_assert_eq!(self.0[self.0.len() - 1], 0);
        let strbytes = &self.0[..(self.0.len() - 1)];
        debug_assert!(!strbytes.contains(&0));
        debug_assert!(std::str::from_utf8(strbytes).is_ok());
    }
    #[inline]
    #[cfg(not(debug_assertions))]
    fn debug_checks(&self) {}
    #[inline]
    pub fn len(&self) -> usize {
        debug_assert_ne!(self.0.len(), 0);
        self.0.len() - 1
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub fn as_cstr(&self) -> &CStr {
        let bytes = self.as_bytes_with_nul();
        debug_assert!(CStr::from_bytes_with_nul(bytes).is_ok());
        unsafe { CStr::from_bytes_with_nul_unchecked(bytes) }
    }
    #[cold]
    fn fabricate_nul_error(b: &str) -> NulError {
        CString::new(b).unwrap_err()
    }
}
impl Default for SmallCString {
    #[inline]
    fn default() -> Self {
        Self(smallvec![0])
    }
}
impl std::fmt::Debug for SmallCString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SmallCString").field(&self.as_str()).finish()
    }
}
impl std::ops::Deref for SmallCString {
    type Target = CStr;
    #[inline]
    fn deref(&self) -> &CStr {
        self.as_cstr()
    }
}
impl std::borrow::Borrow<str> for SmallCString {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
