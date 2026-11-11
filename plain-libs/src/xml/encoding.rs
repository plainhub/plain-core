use std::str::Utf8Error;
pub(crate) const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodingError {
    Utf8(Utf8Error),
}
impl From<Utf8Error> for EncodingError {
    #[inline]
    fn from(e: Utf8Error) -> Self {
        Self::Utf8(e)
    }
}
impl std::error::Error for EncodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Utf8(e) => Some(e),
        }
    }
}
impl std::fmt::Display for EncodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Utf8(e) => write!(f, "cannot decode input using UTF-8: {}", e),
        }
    }
}
