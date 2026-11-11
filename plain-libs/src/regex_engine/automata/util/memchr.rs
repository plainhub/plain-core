pub(crate) use self::inner::*;
pub(super) mod inner {
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn memchr(n1: u8, haystack: &[u8]) -> Option<usize> {
        crate::memchr::memchr(n1, haystack)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn memchr2(n1: u8, n2: u8, haystack: &[u8]) -> Option<usize> {
        crate::memchr::memchr2(n1, n2, haystack)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn memchr3(n1: u8, n2: u8, n3: u8, haystack: &[u8]) -> Option<usize> {
        crate::memchr::memchr3(n1, n2, n3, haystack)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn memrchr(n1: u8, haystack: &[u8]) -> Option<usize> {
        crate::memchr::memrchr(n1, haystack)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn memrchr2(n1: u8, n2: u8, haystack: &[u8]) -> Option<usize> {
        crate::memchr::memrchr2(n1, n2, haystack)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn memrchr3(n1: u8, n2: u8, n3: u8, haystack: &[u8]) -> Option<usize> {
        crate::memchr::memrchr3(n1, n2, n3, haystack)
    }
}
