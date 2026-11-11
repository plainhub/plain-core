#[derive(Copy, Eq, PartialEq, Clone, Debug)]
pub struct Utf8Error {
    pub(crate) valid_up_to: usize,
    pub(crate) error_len: Option<u8>,
}
impl Utf8Error {
    #[inline]
    #[must_use]
    pub fn valid_up_to(&self) -> usize {
        self.valid_up_to
    }
    #[inline]
    #[must_use]
    pub fn error_len(&self) -> Option<usize> {
        self.error_len.map(|len| len as usize)
    }
}
