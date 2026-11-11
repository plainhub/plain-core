use super::{ErrorKind, InvalidUri, Port, URI_CHARS};
use crate::http::byte_str::ByteStr;
use bytes::Bytes;
use std::convert::TryFrom;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::{cmp, fmt, str};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthorityError {
    Empty,
    InvalidUriChar,
    InvalidAuthority,
    TooManyColons,
    MismatchedBrackets,
    InvalidBracketUsage,
    EmptyAfterAt,
    InvalidPercent,
}
#[derive(Clone)]
pub struct Authority {
    pub(super) data: ByteStr,
}
impl Authority {
    pub(super) fn empty() -> Self {
        Authority {
            data: ByteStr::new(),
        }
    }
    pub(super) fn from_shared(s: Bytes) -> Result<Self, InvalidUri> {
        create_authority(s, |s| s)
    }
    #[inline]
    pub const fn from_static(src: &'static str) -> Self {
        match validate_authority_bytes(src.as_bytes()) {
            Ok(_) => Authority {
                data: ByteStr::from_static(src),
            },
            Err(_) => panic!("static str is not valid authority"),
        }
    }
    pub fn from_maybe_shared<T>(src: T) -> Result<Self, InvalidUri>
    where
        T: AsRef<[u8]> + 'static,
    {
        if_downcast_into!(T, Bytes, src, {
            return Authority::from_shared(src);
        });
        Authority::try_from(src.as_ref())
    }
    pub(super) fn parse(s: &[u8]) -> Result<usize, InvalidUri> {
        validate_authority_bytes(s).map_err(|e| {
            match e {
                AuthorityError::Empty => ErrorKind::Empty,
                AuthorityError::InvalidUriChar => ErrorKind::InvalidUriChar,
                AuthorityError::InvalidAuthority
                | AuthorityError::MismatchedBrackets
                | AuthorityError::InvalidBracketUsage
                | AuthorityError::EmptyAfterAt
                | AuthorityError::InvalidPercent
                | AuthorityError::TooManyColons => ErrorKind::InvalidAuthority,
            }
            .into()
        })
    }
    fn parse_non_empty(s: &[u8]) -> Result<usize, InvalidUri> {
        if s.is_empty() {
            return Err(ErrorKind::Empty.into());
        }
        Authority::parse(s)
    }
    #[inline]
    pub fn host(&self) -> &str {
        host(self.as_str())
    }
    pub fn port(&self) -> Option<Port<&str>> {
        let bytes = self.as_str();
        bytes
            .rfind(':')
            .and_then(|i| Port::from_str(&bytes[i + 1..]).ok())
    }
    pub fn port_u16(&self) -> Option<u16> {
        self.port().map(|p| p.as_u16())
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.data[..]
    }
}
impl AsRef<str> for Authority {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl PartialEq for Authority {
    fn eq(&self, other: &Authority) -> bool {
        self.data.eq_ignore_ascii_case(&other.data)
    }
}
impl Eq for Authority {}
impl PartialEq<str> for Authority {
    fn eq(&self, other: &str) -> bool {
        self.data.eq_ignore_ascii_case(other)
    }
}
impl PartialEq<Authority> for str {
    fn eq(&self, other: &Authority) -> bool {
        self.eq_ignore_ascii_case(other.as_str())
    }
}
impl PartialEq<Authority> for &str {
    fn eq(&self, other: &Authority) -> bool {
        self.eq_ignore_ascii_case(other.as_str())
    }
}
impl PartialEq<&str> for Authority {
    fn eq(&self, other: &&str) -> bool {
        self.data.eq_ignore_ascii_case(other)
    }
}
impl PartialEq<String> for Authority {
    fn eq(&self, other: &String) -> bool {
        self.data.eq_ignore_ascii_case(other.as_str())
    }
}
impl PartialEq<Authority> for String {
    fn eq(&self, other: &Authority) -> bool {
        self.as_str().eq_ignore_ascii_case(other.as_str())
    }
}
impl PartialOrd for Authority {
    fn partial_cmp(&self, other: &Authority) -> Option<cmp::Ordering> {
        let left = self.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}
impl PartialOrd<str> for Authority {
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        let left = self.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}
impl PartialOrd<Authority> for str {
    fn partial_cmp(&self, other: &Authority) -> Option<cmp::Ordering> {
        let left = self.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}
impl PartialOrd<Authority> for &str {
    fn partial_cmp(&self, other: &Authority) -> Option<cmp::Ordering> {
        let left = self.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}
impl PartialOrd<&str> for Authority {
    fn partial_cmp(&self, other: &&str) -> Option<cmp::Ordering> {
        let left = self.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}
impl PartialOrd<String> for Authority {
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        let left = self.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}
impl PartialOrd<Authority> for String {
    fn partial_cmp(&self, other: &Authority) -> Option<cmp::Ordering> {
        let left = self.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}
impl Hash for Authority {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.data.len().hash(state);
        for &b in self.data.as_bytes() {
            state.write_u8(b.to_ascii_lowercase());
        }
    }
}
impl TryFrom<&[u8]> for Authority {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &[u8]) -> Result<Self, Self::Error> {
        create_authority(s, Bytes::copy_from_slice)
    }
}
impl TryFrom<&str> for Authority {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        TryFrom::try_from(s.as_bytes())
    }
}
impl TryFrom<Vec<u8>> for Authority {
    type Error = InvalidUri;
    #[inline]
    fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
        Authority::from_shared(vec.into())
    }
}
impl TryFrom<String> for Authority {
    type Error = InvalidUri;
    #[inline]
    fn try_from(t: String) -> Result<Self, Self::Error> {
        Authority::from_shared(t.into())
    }
}
impl FromStr for Authority {
    type Err = InvalidUri;
    fn from_str(s: &str) -> Result<Self, InvalidUri> {
        TryFrom::try_from(s)
    }
}
impl fmt::Debug for Authority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl fmt::Display for Authority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
fn host(auth: &str) -> &str {
    let host_port = auth
        .rsplit('@')
        .next()
        .expect("split always has at least 1 item");
    if host_port.as_bytes()[0] == b'[' {
        let i = host_port
            .find(']')
            .expect("parsing should validate brackets");
        &host_port[0..i + 1]
    } else {
        host_port
            .split(':')
            .next()
            .expect("split always has at least 1 item")
    }
}
fn create_authority<B, F>(b: B, f: F) -> Result<Authority, InvalidUri>
where
    B: AsRef<[u8]>,
    F: FnOnce(B) -> Bytes,
{
    let s = b.as_ref();
    let authority_end = Authority::parse_non_empty(s)?;
    if authority_end != s.len() {
        return Err(ErrorKind::InvalidUriChar.into());
    }
    let bytes = f(b);
    Ok(Authority {
        data: unsafe { ByteStr::from_utf8_unchecked(bytes) },
    })
}
const fn validate_authority_bytes(s: &[u8]) -> Result<usize, AuthorityError> {
    if s.is_empty() {
        return Err(AuthorityError::Empty);
    }
    let mut colon_cnt: u32 = 0;
    let mut start_bracket = false;
    let mut end_bracket = false;
    let mut has_percent = false;
    let mut end = s.len();
    let mut at_sign_pos: usize = s.len();
    const MAX_COLONS: u32 = 8;
    let mut i = 0;
    while i < s.len() {
        let b = s[i];
        let ch = URI_CHARS[b as usize];
        if ch == b'/' || ch == b'?' || ch == b'#' {
            end = i;
            break;
        }
        if ch == 0 {
            if b == b'%' {
                has_percent = true;
            } else {
                return Err(AuthorityError::InvalidUriChar);
            }
        } else if ch == b':' {
            if colon_cnt >= MAX_COLONS {
                return Err(AuthorityError::TooManyColons);
            }
            colon_cnt += 1;
        } else if ch == b'[' {
            if has_percent || start_bracket {
                return Err(AuthorityError::InvalidBracketUsage);
            }
            start_bracket = true;
        } else if ch == b']' {
            if !start_bracket || end_bracket {
                return Err(AuthorityError::InvalidBracketUsage);
            }
            end_bracket = true;
            colon_cnt = 0;
            has_percent = false;
        } else if ch == b'@' {
            at_sign_pos = i;
            colon_cnt = 0;
            has_percent = false;
        }
        i += 1;
    }
    if start_bracket != end_bracket {
        return Err(AuthorityError::MismatchedBrackets);
    }
    if colon_cnt > 1 {
        return Err(AuthorityError::InvalidAuthority);
    }
    if end > 0 && at_sign_pos == end - 1 {
        return Err(AuthorityError::EmptyAfterAt);
    }
    if has_percent {
        return Err(AuthorityError::InvalidPercent);
    }
    Ok(end)
}
