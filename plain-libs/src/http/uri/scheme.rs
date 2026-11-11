use super::{ErrorKind, InvalidUri};
use crate::http::byte_str::ByteStr;
use bytes::Bytes;
use std::convert::TryFrom;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
#[derive(Clone)]
pub struct Scheme {
    pub(super) inner: Scheme2,
}
#[derive(Clone, Debug)]
pub(super) enum Scheme2<T = Box<ByteStr>> {
    None,
    Standard(Protocol),
    Other(T),
}
#[derive(Copy, Clone, Debug)]
pub(super) enum Protocol {
    Http,
    Https,
}
impl Scheme {
    pub const HTTP: Scheme = Scheme {
        inner: Scheme2::Standard(Protocol::Http),
    };
    pub const HTTPS: Scheme = Scheme {
        inner: Scheme2::Standard(Protocol::Https),
    };
    pub(super) fn empty() -> Self {
        Scheme {
            inner: Scheme2::None,
        }
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        use self::Protocol::*;
        use self::Scheme2::*;
        match self.inner {
            Standard(Http) => "http",
            Standard(Https) => "https",
            Other(ref v) => &v[..],
            None => unreachable!(),
        }
    }
}
impl TryFrom<&[u8]> for Scheme {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &[u8]) -> Result<Self, Self::Error> {
        use self::Scheme2::*;
        match Scheme2::parse_exact(s)? {
            None => Err(ErrorKind::InvalidScheme.into()),
            Standard(p) => Ok(Standard(p).into()),
            Other(_) => {
                let bytes = Bytes::copy_from_slice(s);
                let string = unsafe { ByteStr::from_utf8_unchecked(bytes) };
                Ok(Other(Box::new(string)).into())
            }
        }
    }
}
impl TryFrom<&str> for Scheme {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        TryFrom::try_from(s.as_bytes())
    }
}
impl FromStr for Scheme {
    type Err = InvalidUri;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TryFrom::try_from(s)
    }
}
impl fmt::Debug for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}
impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl AsRef<str> for Scheme {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl PartialEq for Scheme {
    fn eq(&self, other: &Scheme) -> bool {
        use self::Protocol::*;
        use self::Scheme2::*;
        match (&self.inner, &other.inner) {
            (&Standard(Http), &Standard(Http)) => true,
            (&Standard(Https), &Standard(Https)) => true,
            (Other(a), Other(b)) => a.eq_ignore_ascii_case(b),
            (&None, _) | (_, &None) => unreachable!(),
            _ => false,
        }
    }
}
impl Eq for Scheme {}
impl PartialEq<str> for Scheme {
    fn eq(&self, other: &str) -> bool {
        self.as_str().eq_ignore_ascii_case(other)
    }
}
impl PartialEq<Scheme> for str {
    fn eq(&self, other: &Scheme) -> bool {
        other == self
    }
}
impl Hash for Scheme {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        match self.inner {
            Scheme2::None => (),
            Scheme2::Standard(Protocol::Http) => state.write_u8(1),
            Scheme2::Standard(Protocol::Https) => state.write_u8(2),
            Scheme2::Other(ref other) => {
                other.len().hash(state);
                for &b in other.as_bytes() {
                    state.write_u8(b.to_ascii_lowercase());
                }
            }
        }
    }
}
impl<T> Scheme2<T> {
    pub(super) fn is_none(&self) -> bool {
        matches!(*self, Scheme2::None)
    }
}
const MAX_SCHEME_LEN: usize = 64;
#[rustfmt::skip] const SCHEME_CHARS : [u8 ; 256] = [0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , b'+' , 0 , b'-' , b'.' , 0 , b'0' , b'1' , b'2' , b'3' , b'4' , b'5' , b'6' , b'7' , b'8' , b'9' , b':' , 0 , 0 , 0 , 0 , 0 , 0 , b'A' , b'B' , b'C' , b'D' , b'E' , b'F' , b'G' , b'H' , b'I' , b'J' , b'K' , b'L' , b'M' , b'N' , b'O' , b'P' , b'Q' , b'R' , b'S' , b'T' , b'U' , b'V' , b'W' , b'X' , b'Y' , b'Z' , 0 , 0 , 0 , 0 , 0 , 0 , b'a' , b'b' , b'c' , b'd' , b'e' , b'f' , b'g' , b'h' , b'i' , b'j' , b'k' , b'l' , b'm' , b'n' , b'o' , b'p' , b'q' , b'r' , b's' , b't' , b'u' , b'v' , b'w' , b'x' , b'y' , b'z' , 0 , 0 , 0 , b'~' , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] ;
impl Scheme2<usize> {
    fn parse_exact(s: &[u8]) -> Result<Scheme2<()>, InvalidUri> {
        match s {
            b"http" => Ok(Protocol::Http.into()),
            b"https" => Ok(Protocol::Https.into()),
            _ => {
                if s.len() > MAX_SCHEME_LEN {
                    return Err(ErrorKind::SchemeTooLong.into());
                }
                for &b in s {
                    match SCHEME_CHARS[b as usize] {
                        b':' => {
                            return Err(ErrorKind::InvalidScheme.into());
                        }
                        0 => {
                            return Err(ErrorKind::InvalidScheme.into());
                        }
                        _ => {}
                    }
                }
                Ok(Scheme2::Other(()))
            }
        }
    }
    pub(super) fn parse(s: &[u8]) -> Result<Scheme2<usize>, InvalidUri> {
        if s.len() >= 7 {
            if s[..7].eq_ignore_ascii_case(b"http://") {
                return Ok(Protocol::Http.into());
            }
        }
        if s.len() >= 8 {
            if s[..8].eq_ignore_ascii_case(b"https://") {
                return Ok(Protocol::Https.into());
            }
        }
        if s.len() > 3 {
            for i in 0..s.len() {
                let b = s[i];
                match SCHEME_CHARS[b as usize] {
                    b':' => {
                        if s.len() < i + 3 {
                            break;
                        }
                        if &s[i + 1..i + 3] != b"//" {
                            break;
                        }
                        if i > MAX_SCHEME_LEN {
                            return Err(ErrorKind::SchemeTooLong.into());
                        }
                        return Ok(Scheme2::Other(i));
                    }
                    0 => break,
                    _ => {}
                }
            }
        }
        Ok(Scheme2::None)
    }
}
impl Protocol {
    pub(super) fn len(&self) -> usize {
        match *self {
            Protocol::Http => 4,
            Protocol::Https => 5,
        }
    }
}
impl<T> From<Protocol> for Scheme2<T> {
    fn from(src: Protocol) -> Self {
        Scheme2::Standard(src)
    }
}
impl From<Scheme2> for Scheme {
    fn from(src: Scheme2) -> Self {
        Scheme { inner: src }
    }
}
