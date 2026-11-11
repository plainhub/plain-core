use super::{ErrorKind, InvalidUri, MAX_LEN};
use crate::http::byte_str::ByteStr;
use bytes::Bytes;
use std::convert::TryFrom;
use std::str::FromStr;
use std::{cmp, fmt, hash, str};
#[derive(Clone)]
pub struct PathAndQuery {
    pub(super) data: ByteStr,
    pub(super) query: u16,
}
const NONE: u16 = u16::MAX;
impl PathAndQuery {
    pub(super) fn from_shared(mut src: Bytes) -> Result<Self, InvalidUri> {
        let Scanned {
            query,
            fragment,
            is_maybe_not_utf8,
        } = scan_path_and_query(&src)?;
        if let Some(i) = fragment {
            src.truncate(i as usize);
        }
        let data = if is_maybe_not_utf8 {
            ByteStr::from_utf8(src).map_err(|_| ErrorKind::InvalidUriChar)?
        } else {
            unsafe { ByteStr::from_utf8_unchecked(src) }
        };
        Ok(PathAndQuery { data, query })
    }
    #[inline]
    pub const fn from_static(src: &'static str) -> Self {
        match scan_path_and_query(src.as_bytes()) {
            Ok(Scanned {
                query,
                fragment: None,
                is_maybe_not_utf8: false,
            }) => PathAndQuery {
                data: ByteStr::from_static(src),
                query,
            },
            _ => panic!("static str is not valid path"),
        }
    }
    pub fn from_maybe_shared<T>(src: T) -> Result<Self, InvalidUri>
    where
        T: AsRef<[u8]> + 'static,
    {
        if_downcast_into!(T, Bytes, src, {
            return PathAndQuery::from_shared(src);
        });
        PathAndQuery::try_from(src.as_ref())
    }
    pub(super) fn empty() -> Self {
        PathAndQuery {
            data: ByteStr::new(),
            query: NONE,
        }
    }
    pub(super) fn slash() -> Self {
        PathAndQuery {
            data: ByteStr::from_static("/"),
            query: NONE,
        }
    }
    pub(super) fn star() -> Self {
        PathAndQuery {
            data: ByteStr::from_static("*"),
            query: NONE,
        }
    }
    #[inline]
    pub fn path(&self) -> &str {
        let ret = if self.query == NONE {
            &self.data[..]
        } else {
            &self.data[..self.query as usize]
        };
        if ret.is_empty() {
            return "/";
        }
        ret
    }
    #[inline]
    pub fn query(&self) -> Option<&str> {
        if self.query == NONE {
            None
        } else {
            let i = self.query + 1;
            Some(&self.data[i as usize..])
        }
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        let ret = &self.data[..];
        if ret.is_empty() {
            return "/";
        }
        ret
    }
}
impl TryFrom<&[u8]> for PathAndQuery {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &[u8]) -> Result<Self, Self::Error> {
        PathAndQuery::from_shared(Bytes::copy_from_slice(s))
    }
}
impl TryFrom<&str> for PathAndQuery {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        TryFrom::try_from(s.as_bytes())
    }
}
impl TryFrom<Vec<u8>> for PathAndQuery {
    type Error = InvalidUri;
    #[inline]
    fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
        PathAndQuery::from_shared(vec.into())
    }
}
impl TryFrom<String> for PathAndQuery {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: String) -> Result<Self, Self::Error> {
        PathAndQuery::from_shared(s.into())
    }
}
impl TryFrom<&String> for PathAndQuery {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &String) -> Result<Self, Self::Error> {
        TryFrom::try_from(s.as_bytes())
    }
}
impl FromStr for PathAndQuery {
    type Err = InvalidUri;
    #[inline]
    fn from_str(s: &str) -> Result<Self, InvalidUri> {
        TryFrom::try_from(s)
    }
}
impl fmt::Debug for PathAndQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl fmt::Display for PathAndQuery {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.data.is_empty() {
            match self.data.as_bytes()[0] {
                b'/' | b'*' => fmt.write_str(&self.data),
                _ => {
                    fmt.write_str("/")?;
                    fmt.write_str(&self.data)
                }
            }
        } else {
            fmt.write_str("/")
        }
    }
}
impl hash::Hash for PathAndQuery {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}
impl PartialEq for PathAndQuery {
    #[inline]
    fn eq(&self, other: &PathAndQuery) -> bool {
        self.data == other.data
    }
}
impl Eq for PathAndQuery {}
impl PartialEq<str> for PathAndQuery {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}
impl PartialEq<PathAndQuery> for &str {
    #[inline]
    fn eq(&self, other: &PathAndQuery) -> bool {
        self == &other.as_str()
    }
}
impl PartialEq<&str> for PathAndQuery {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}
impl PartialEq<PathAndQuery> for str {
    #[inline]
    fn eq(&self, other: &PathAndQuery) -> bool {
        self == other.as_str()
    }
}
impl PartialEq<String> for PathAndQuery {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}
impl PartialEq<PathAndQuery> for String {
    #[inline]
    fn eq(&self, other: &PathAndQuery) -> bool {
        self.as_str() == other.as_str()
    }
}
impl PartialOrd for PathAndQuery {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQuery) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}
impl PartialOrd<str> for PathAndQuery {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other)
    }
}
impl PartialOrd<PathAndQuery> for str {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQuery) -> Option<cmp::Ordering> {
        self.partial_cmp(other.as_str())
    }
}
impl PartialOrd<&str> for PathAndQuery {
    #[inline]
    fn partial_cmp(&self, other: &&str) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(*other)
    }
}
impl PartialOrd<PathAndQuery> for &str {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQuery) -> Option<cmp::Ordering> {
        self.partial_cmp(&other.as_str())
    }
}
impl PartialOrd<String> for PathAndQuery {
    #[inline]
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}
impl PartialOrd<PathAndQuery> for String {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQuery) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}
struct Scanned {
    query: u16,
    fragment: Option<u16>,
    is_maybe_not_utf8: bool,
}
const CLASS_VALID: u8 = 0;
const CLASS_QUERY: u8 = 1;
const CLASS_FRAGMENT: u8 = 2;
const CLASS_HIGH: u8 = 3;
const CLASS_INVALID: u8 = 4;
const fn build_path_map() -> [u8; 256] {
    let mut t = [CLASS_INVALID; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = match i as u8 {
            b'?' => CLASS_QUERY,
            b'#' => CLASS_FRAGMENT,
            0x21 | 0x24..=0x3B | 0x3D | 0x40..=0x5F | 0x61..=0x7A | 0x7C | 0x7E => CLASS_VALID,
            0x80..=0xFF => CLASS_HIGH,
            b'"' | b'{' | b'}' => CLASS_VALID,
            _ => CLASS_INVALID,
        };
        i += 1;
    }
    t
}
const fn build_query_map() -> [u8; 256] {
    let mut t = [CLASS_INVALID; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = match i as u8 {
            b'#' => CLASS_FRAGMENT,
            0x21 | 0x24..=0x3B | 0x3D | 0x3F..=0x7E => CLASS_VALID,
            0x80..=0xFF => CLASS_HIGH,
            _ => CLASS_INVALID,
        };
        i += 1;
    }
    t
}
const PATH_MAP: [u8; 256] = build_path_map();
const QUERY_MAP: [u8; 256] = build_query_map();
const fn scan_path_and_query(bytes: &[u8]) -> Result<Scanned, ErrorKind> {
    let mut i = 0;
    let mut query = NONE;
    let mut fragment = None;
    let mut is_maybe_not_utf8 = false;
    if bytes.is_empty() {
        return Err(ErrorKind::Empty);
    }
    if bytes.len() > MAX_LEN {
        return Err(ErrorKind::TooLong);
    }
    if bytes.len() == 1 && bytes[0] == b'*' {
        return Ok(Scanned {
            query,
            fragment,
            is_maybe_not_utf8: false,
        });
    }
    if !matches!(bytes[0], b'/' | b'?' | b'#') {
        return Err(ErrorKind::PathDoesNotStartWithSlash);
    }
    while i < bytes.len() {
        match PATH_MAP[bytes[i] as usize] {
            CLASS_VALID => {}
            CLASS_QUERY => {
                debug_assert!(query == NONE);
                query = i as u16;
                i += 1;
                break;
            }
            CLASS_FRAGMENT => {
                fragment = Some(i as u16);
                break;
            }
            CLASS_HIGH => {
                is_maybe_not_utf8 = true;
            }
            _ => return Err(ErrorKind::InvalidUriChar),
        }
        i += 1;
    }
    if query != NONE {
        while i < bytes.len() {
            match QUERY_MAP[bytes[i] as usize] {
                CLASS_VALID => {}
                CLASS_HIGH => {
                    is_maybe_not_utf8 = true;
                }
                CLASS_FRAGMENT => {
                    fragment = Some(i as u16);
                    break;
                }
                _ => return Err(ErrorKind::InvalidUriChar),
            }
            i += 1;
        }
    }
    Ok(Scanned {
        query,
        fragment,
        is_maybe_not_utf8,
    })
}
