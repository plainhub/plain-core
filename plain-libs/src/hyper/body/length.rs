use std::fmt;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecodedLength(u64);
impl From<Option<u64>> for DecodedLength {
    fn from(len: Option<u64>) -> Self {
        len.and_then(|len| Self::checked_new(len).ok())
            .unwrap_or(DecodedLength::CHUNKED)
    }
}
const MAX_LEN: u64 = u64::MAX - 2;
impl DecodedLength {
    pub(crate) const CLOSE_DELIMITED: DecodedLength = DecodedLength(u64::MAX);
    pub(crate) const CHUNKED: DecodedLength = DecodedLength(u64::MAX - 1);
    pub(crate) const ZERO: DecodedLength = DecodedLength(0);
    #[inline]
    pub(crate) fn danger_len(self) -> u64 {
        debug_assert!(self.0 < Self::CHUNKED.0);
        self.0
    }
    pub(crate) fn into_opt(self) -> Option<u64> {
        match self {
            DecodedLength::CHUNKED | DecodedLength::CLOSE_DELIMITED => None,
            DecodedLength(known) => Some(known),
        }
    }
    pub(crate) fn checked_new(len: u64) -> Result<Self, crate::hyper::error::Parse> {
        if len <= MAX_LEN {
            Ok(DecodedLength(len))
        } else {
            warn!("content-length bigger than maximum: {} > {}", len, MAX_LEN);
            Err(crate::hyper::error::Parse::TooLarge)
        }
    }
    pub(crate) fn sub_if(&mut self, amt: u64) {
        match self {
            &mut DecodedLength::CHUNKED | &mut DecodedLength::CLOSE_DELIMITED => (),
            DecodedLength(known) => {
                *known -= amt;
            }
        }
    }
    pub(crate) fn is_exact(&self) -> bool {
        self.0 <= MAX_LEN
    }
}
impl fmt::Debug for DecodedLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DecodedLength::CLOSE_DELIMITED => f.write_str("CLOSE_DELIMITED"),
            DecodedLength::CHUNKED => f.write_str("CHUNKED"),
            DecodedLength(n) => f.debug_tuple("DecodedLength").field(&n).finish(),
        }
    }
}
impl fmt::Display for DecodedLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DecodedLength::CLOSE_DELIMITED => f.write_str("close-delimited"),
            DecodedLength::CHUNKED => f.write_str("chunked encoding"),
            DecodedLength::ZERO => f.write_str("empty"),
            DecodedLength(n) => write!(f, "content-length ({n} bytes)"),
        }
    }
}
