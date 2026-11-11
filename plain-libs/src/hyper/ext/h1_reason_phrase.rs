use bytes::Bytes;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReasonPhrase(Bytes);
impl ReasonPhrase {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
    pub const fn from_static(reason: &'static [u8]) -> Self {
        assert!(
            find_invalid_byte(reason).is_none(),
            "invalid byte in static reason phrase"
        );
        Self(Bytes::from_static(reason))
    }
    pub(crate) fn from_bytes_unchecked(reason: Bytes) -> Self {
        Self(reason)
    }
}
impl TryFrom<&[u8]> for ReasonPhrase {
    type Error = InvalidReasonPhrase;
    fn try_from(reason: &[u8]) -> Result<Self, Self::Error> {
        if let Some(bad_byte) = find_invalid_byte(reason) {
            Err(InvalidReasonPhrase { bad_byte })
        } else {
            Ok(Self(Bytes::copy_from_slice(reason)))
        }
    }
}
impl TryFrom<Vec<u8>> for ReasonPhrase {
    type Error = InvalidReasonPhrase;
    fn try_from(reason: Vec<u8>) -> Result<Self, Self::Error> {
        if let Some(bad_byte) = find_invalid_byte(&reason) {
            Err(InvalidReasonPhrase { bad_byte })
        } else {
            Ok(Self(Bytes::from(reason)))
        }
    }
}
impl TryFrom<String> for ReasonPhrase {
    type Error = InvalidReasonPhrase;
    fn try_from(reason: String) -> Result<Self, Self::Error> {
        if let Some(bad_byte) = find_invalid_byte(reason.as_bytes()) {
            Err(InvalidReasonPhrase { bad_byte })
        } else {
            Ok(Self(Bytes::from(reason)))
        }
    }
}
impl TryFrom<Bytes> for ReasonPhrase {
    type Error = InvalidReasonPhrase;
    fn try_from(reason: Bytes) -> Result<Self, Self::Error> {
        if let Some(bad_byte) = find_invalid_byte(&reason) {
            Err(InvalidReasonPhrase { bad_byte })
        } else {
            Ok(Self(reason))
        }
    }
}
impl From<ReasonPhrase> for Bytes {
    fn from(reason: ReasonPhrase) -> Self {
        reason.0
    }
}
impl AsRef<[u8]> for ReasonPhrase {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
#[derive(Debug)]
pub struct InvalidReasonPhrase {
    bad_byte: u8,
}
impl std::fmt::Display for InvalidReasonPhrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid byte in reason phrase: {}", self.bad_byte)
    }
}
impl std::error::Error for InvalidReasonPhrase {}
const fn is_valid_byte(b: u8) -> bool {
    const fn is_vchar(b: u8) -> bool {
        0x21 <= b && b <= 0x7E
    }
    const fn is_obs_text(b: u8) -> bool {
        0x80 <= b && b <= 0xFF
    }
    b == b'\t' || b == b' ' || is_vchar(b) || is_obs_text(b)
}
const fn find_invalid_byte(bytes: &[u8]) -> Option<u8> {
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if !is_valid_byte(b) {
            return Some(b);
        }
        i += 1;
    }
    None
}
