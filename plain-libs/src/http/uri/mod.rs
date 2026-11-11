pub use self::authority::Authority;
pub use self::builder::Builder;
pub use self::path::PathAndQuery;
pub use self::port::Port;
pub use self::scheme::Scheme;
use self::scheme::Scheme2;
use crate::http::byte_str::ByteStr;
use bytes::Bytes;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::{self, FromStr};
mod authority;
mod builder;
mod path;
mod port;
mod scheme;
#[derive(Clone)]
pub struct Uri {
    scheme: Scheme,
    authority: Authority,
    path_and_query: PathAndQuery,
}
#[derive(Debug, Default)]
pub struct Parts {
    pub scheme: Option<Scheme>,
    pub authority: Option<Authority>,
    pub path_and_query: Option<PathAndQuery>,
    _priv: (),
}
#[derive(Debug)]
pub struct InvalidUri(ErrorKind);
#[derive(Debug)]
pub struct InvalidUriParts(InvalidUri);
#[derive(Debug, Eq, PartialEq)]
enum ErrorKind {
    InvalidUriChar,
    InvalidScheme,
    InvalidAuthority,
    InvalidPort,
    InvalidFormat,
    SchemeMissing,
    AuthorityMissing,
    PathAndQueryMissing,
    PathDoesNotStartWithSlash,
    TooLong,
    Empty,
    SchemeTooLong,
}
const MAX_LEN: usize = (u16::MAX - 1) as usize;
#[rustfmt::skip] const URI_CHARS : [u8 ; 256] = [0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , b'!' , 0 , b'#' , b'$' , 0 , b'&' , b'\'' , b'(' , b')' , b'*' , b'+' , b',' , b'-' , b'.' , b'/' , b'0' , b'1' , b'2' , b'3' , b'4' , b'5' , b'6' , b'7' , b'8' , b'9' , b':' , b';' , 0 , b'=' , 0 , b'?' , b'@' , b'A' , b'B' , b'C' , b'D' , b'E' , b'F' , b'G' , b'H' , b'I' , b'J' , b'K' , b'L' , b'M' , b'N' , b'O' , b'P' , b'Q' , b'R' , b'S' , b'T' , b'U' , b'V' , b'W' , b'X' , b'Y' , b'Z' , b'[' , 0 , b']' , 0 , b'_' , 0 , b'a' , b'b' , b'c' , b'd' , b'e' , b'f' , b'g' , b'h' , b'i' , b'j' , b'k' , b'l' , b'm' , b'n' , b'o' , b'p' , b'q' , b'r' , b's' , b't' , b'u' , b'v' , b'w' , b'x' , b'y' , b'z' , 0 , 0 , 0 , b'~' , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] ;
impl Uri {
    pub fn builder() -> Builder {
        Builder::new()
    }
    pub fn from_parts(src: Parts) -> Result<Uri, InvalidUriParts> {
        if src.scheme.is_some() {
            if src.authority.is_none() {
                return Err(ErrorKind::AuthorityMissing.into());
            }
            if src.path_and_query.is_none() {
                return Err(ErrorKind::PathAndQueryMissing.into());
            }
        } else if src.authority.is_some() && src.path_and_query.is_some() {
            return Err(ErrorKind::SchemeMissing.into());
        }
        let scheme = match src.scheme {
            Some(scheme) => scheme,
            None => Scheme {
                inner: Scheme2::None,
            },
        };
        let authority = match src.authority {
            Some(authority) => authority,
            None => Authority::empty(),
        };
        let path_and_query = match src.path_and_query {
            Some(path_and_query) => path_and_query,
            None => PathAndQuery::empty(),
        };
        Ok(Uri {
            scheme,
            authority,
            path_and_query,
        })
    }
    pub fn from_maybe_shared<T>(src: T) -> Result<Self, InvalidUri>
    where
        T: AsRef<[u8]> + 'static,
    {
        if_downcast_into!(T, Bytes, src, {
            return Uri::from_shared(src);
        });
        Uri::try_from(src.as_ref())
    }
    fn from_shared(s: Bytes) -> Result<Uri, InvalidUri> {
        use self::ErrorKind::*;
        if s.len() > MAX_LEN {
            return Err(TooLong.into());
        }
        match s.len() {
            0 => {
                return Err(Empty.into());
            }
            1 => match s[0] {
                b'/' => {
                    return Ok(Uri {
                        scheme: Scheme::empty(),
                        authority: Authority::empty(),
                        path_and_query: PathAndQuery::slash(),
                    });
                }
                b'*' => {
                    return Ok(Uri {
                        scheme: Scheme::empty(),
                        authority: Authority::empty(),
                        path_and_query: PathAndQuery::star(),
                    });
                }
                _ => {
                    let authority = Authority::from_shared(s)?;
                    return Ok(Uri {
                        scheme: Scheme::empty(),
                        authority,
                        path_and_query: PathAndQuery::empty(),
                    });
                }
            },
            _ => {}
        }
        if s[0] == b'/' {
            return Ok(Uri {
                scheme: Scheme::empty(),
                authority: Authority::empty(),
                path_and_query: PathAndQuery::from_shared(s)?,
            });
        }
        parse_full(s)
    }
    pub fn from_static(src: &'static str) -> Self {
        let s = Bytes::from_static(src.as_bytes());
        match Uri::from_shared(s) {
            Ok(uri) => uri,
            Err(e) => panic!("static str is not valid URI: {}", e),
        }
    }
    #[inline]
    pub fn into_parts(self) -> Parts {
        self.into()
    }
    #[inline]
    pub fn path_and_query(&self) -> Option<&PathAndQuery> {
        if !self.scheme.inner.is_none() || self.authority.data.is_empty() {
            Some(&self.path_and_query)
        } else {
            None
        }
    }
    #[inline]
    pub fn path(&self) -> &str {
        if self.has_path() {
            self.path_and_query.path()
        } else {
            ""
        }
    }
    #[inline]
    pub fn scheme(&self) -> Option<&Scheme> {
        if self.scheme.inner.is_none() {
            None
        } else {
            Some(&self.scheme)
        }
    }
    #[inline]
    pub fn scheme_str(&self) -> Option<&str> {
        if self.scheme.inner.is_none() {
            None
        } else {
            Some(self.scheme.as_str())
        }
    }
    #[inline]
    pub fn authority(&self) -> Option<&Authority> {
        if self.authority.data.is_empty() {
            None
        } else {
            Some(&self.authority)
        }
    }
    #[inline]
    pub fn host(&self) -> Option<&str> {
        self.authority().map(|a| a.host())
    }
    pub fn port(&self) -> Option<Port<&str>> {
        self.authority().and_then(|a| a.port())
    }
    pub fn port_u16(&self) -> Option<u16> {
        self.port().map(|p| p.as_u16())
    }
    #[inline]
    pub fn query(&self) -> Option<&str> {
        self.path_and_query.query()
    }
    fn has_path(&self) -> bool {
        !self.path_and_query.data.is_empty() || !self.scheme.inner.is_none()
    }
}
impl TryFrom<&[u8]> for Uri {
    type Error = InvalidUri;
    #[inline]
    fn try_from(t: &[u8]) -> Result<Self, Self::Error> {
        Uri::from_shared(Bytes::copy_from_slice(t))
    }
}
impl TryFrom<&str> for Uri {
    type Error = InvalidUri;
    #[inline]
    fn try_from(t: &str) -> Result<Self, Self::Error> {
        t.parse()
    }
}
impl TryFrom<&String> for Uri {
    type Error = InvalidUri;
    #[inline]
    fn try_from(t: &String) -> Result<Self, Self::Error> {
        t.parse()
    }
}
impl TryFrom<String> for Uri {
    type Error = InvalidUri;
    #[inline]
    fn try_from(t: String) -> Result<Self, Self::Error> {
        Uri::from_shared(Bytes::from(t))
    }
}
impl TryFrom<Vec<u8>> for Uri {
    type Error = InvalidUri;
    #[inline]
    fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
        Uri::from_shared(Bytes::from(vec))
    }
}
impl TryFrom<Parts> for Uri {
    type Error = InvalidUriParts;
    #[inline]
    fn try_from(src: Parts) -> Result<Self, Self::Error> {
        Uri::from_parts(src)
    }
}
impl TryFrom<&Uri> for Uri {
    type Error = crate::http::Error;
    #[inline]
    fn try_from(src: &Uri) -> Result<Self, Self::Error> {
        Ok(src.clone())
    }
}
impl From<Authority> for Uri {
    fn from(authority: Authority) -> Self {
        Self {
            scheme: Scheme::empty(),
            authority,
            path_and_query: PathAndQuery::empty(),
        }
    }
}
impl From<PathAndQuery> for Uri {
    fn from(path_and_query: PathAndQuery) -> Self {
        Self {
            scheme: Scheme::empty(),
            authority: Authority::empty(),
            path_and_query,
        }
    }
}
impl From<Uri> for Parts {
    fn from(src: Uri) -> Self {
        let path_and_query = if src.has_path() {
            Some(src.path_and_query)
        } else {
            None
        };
        let scheme = match src.scheme.inner {
            Scheme2::None => None,
            _ => Some(src.scheme),
        };
        let authority = if src.authority.data.is_empty() {
            None
        } else {
            Some(src.authority)
        };
        Parts {
            scheme,
            authority,
            path_and_query,
            _priv: (),
        }
    }
}
fn parse_full(mut s: Bytes) -> Result<Uri, InvalidUri> {
    let scheme = match Scheme2::parse(&s[..])? {
        Scheme2::None => Scheme2::None,
        Scheme2::Standard(p) => {
            let _ = s.split_to(p.len() + 3);
            Scheme2::Standard(p)
        }
        Scheme2::Other(n) => {
            let mut scheme = s.split_to(n + 3);
            let _ = scheme.split_off(n);
            let val = unsafe { ByteStr::from_utf8_unchecked(scheme) };
            Scheme2::Other(Box::new(val))
        }
    };
    let authority_end = Authority::parse(&s[..])?;
    if scheme.is_none() {
        if authority_end != s.len() {
            return Err(ErrorKind::InvalidFormat.into());
        }
        let authority = Authority {
            data: unsafe { ByteStr::from_utf8_unchecked(s) },
        };
        return Ok(Uri {
            scheme: scheme.into(),
            authority,
            path_and_query: PathAndQuery::empty(),
        });
    }
    if authority_end == 0 {
        return Err(ErrorKind::InvalidFormat.into());
    }
    let authority = s.split_to(authority_end);
    let authority = Authority {
        data: unsafe { ByteStr::from_utf8_unchecked(authority) },
    };
    let path_and_query = if s.is_empty() {
        PathAndQuery::slash()
    } else {
        PathAndQuery::from_shared(s)?
    };
    Ok(Uri {
        scheme: scheme.into(),
        authority,
        path_and_query,
    })
}
impl FromStr for Uri {
    type Err = InvalidUri;
    #[inline]
    fn from_str(s: &str) -> Result<Uri, InvalidUri> {
        Uri::try_from(s.as_bytes())
    }
}
impl PartialEq for Uri {
    fn eq(&self, other: &Uri) -> bool {
        if self.scheme() != other.scheme() {
            return false;
        }
        if self.authority() != other.authority() {
            return false;
        }
        if self.path() != other.path() {
            return false;
        }
        if self.query() != other.query() {
            return false;
        }
        true
    }
}
impl PartialEq<str> for Uri {
    fn eq(&self, other: &str) -> bool {
        let mut other = other.as_bytes();
        let mut absolute = false;
        if let Some(scheme) = self.scheme() {
            let scheme = scheme.as_str().as_bytes();
            absolute = true;
            if other.len() < scheme.len() + 3 {
                return false;
            }
            if !scheme.eq_ignore_ascii_case(&other[..scheme.len()]) {
                return false;
            }
            other = &other[scheme.len()..];
            if &other[..3] != b"://" {
                return false;
            }
            other = &other[3..];
        }
        if let Some(auth) = self.authority() {
            let len = auth.data.len();
            absolute = true;
            if other.len() < len {
                return false;
            }
            if !auth.data.as_bytes().eq_ignore_ascii_case(&other[..len]) {
                return false;
            }
            other = &other[len..];
        }
        let path = self.path();
        if other.len() < path.len() || path.as_bytes() != &other[..path.len()] {
            if absolute && path == "/" {
            } else {
                return false;
            }
        } else {
            other = &other[path.len()..];
        }
        if let Some(query) = self.query() {
            if other.is_empty() {
                return query.is_empty();
            }
            if other[0] != b'?' {
                return false;
            }
            other = &other[1..];
            if other.len() < query.len() {
                return false;
            }
            if query.as_bytes() != &other[..query.len()] {
                return false;
            }
            other = &other[query.len()..];
        }
        other.is_empty() || other[0] == b'#'
    }
}
impl PartialEq<Uri> for str {
    fn eq(&self, uri: &Uri) -> bool {
        uri == self
    }
}
impl PartialEq<&str> for Uri {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}
impl PartialEq<Uri> for &str {
    fn eq(&self, uri: &Uri) -> bool {
        uri == *self
    }
}
impl Eq for Uri {}
impl Default for Uri {
    #[inline]
    fn default() -> Uri {
        Uri {
            scheme: Scheme::empty(),
            authority: Authority::empty(),
            path_and_query: PathAndQuery::slash(),
        }
    }
}
impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(scheme) = self.scheme() {
            f.write_str(scheme.as_str())?;
            f.write_str("://")?;
        }
        if let Some(authority) = self.authority() {
            f.write_str(authority.as_str())?;
        }
        f.write_str(self.path())?;
        if let Some(query) = self.query() {
            f.write_str("?")?;
            f.write_str(query)?;
        }
        Ok(())
    }
}
impl fmt::Debug for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl From<ErrorKind> for InvalidUri {
    fn from(src: ErrorKind) -> InvalidUri {
        InvalidUri(src)
    }
}
impl From<ErrorKind> for InvalidUriParts {
    fn from(src: ErrorKind) -> InvalidUriParts {
        InvalidUriParts(src.into())
    }
}
impl InvalidUri {
    pub(crate) fn is_empty(&self) -> bool {
        self.0 == ErrorKind::Empty
    }
    fn s(&self) -> &str {
        match self.0 {
            ErrorKind::InvalidUriChar => "invalid uri character",
            ErrorKind::InvalidScheme => "invalid scheme",
            ErrorKind::InvalidAuthority => "invalid authority",
            ErrorKind::InvalidPort => "invalid port",
            ErrorKind::InvalidFormat => "invalid format",
            ErrorKind::SchemeMissing => "scheme missing",
            ErrorKind::AuthorityMissing => "authority missing",
            ErrorKind::PathAndQueryMissing => "path missing",
            ErrorKind::PathDoesNotStartWithSlash => "path does not start with slash",
            ErrorKind::TooLong => "uri too long",
            ErrorKind::Empty => "empty string",
            ErrorKind::SchemeTooLong => "scheme too long",
        }
    }
}
impl fmt::Display for InvalidUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.s().fmt(f)
    }
}
impl Error for InvalidUri {}
impl fmt::Display for InvalidUriParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for InvalidUriParts {}
impl Hash for Uri {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        if !self.scheme.inner.is_none() {
            self.scheme.hash(state);
            state.write_u8(0xff);
        }
        if let Some(auth) = self.authority() {
            auth.hash(state);
        }
        Hash::hash_slice(self.path().as_bytes(), state);
        if let Some(query) = self.query() {
            b'?'.hash(state);
            Hash::hash_slice(query.as_bytes(), state);
        }
    }
}
