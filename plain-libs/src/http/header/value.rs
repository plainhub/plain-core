use crate::http::header::name::HeaderName;
use bytes::{Bytes, BytesMut};
use std::convert::TryFrom;
use std::error::Error;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::{cmp, fmt, str};
#[derive(Clone)]
pub struct HeaderValue {
    inner: Bytes,
    is_sensitive: bool,
}
pub struct InvalidHeaderValue {
    _priv: (),
}
#[derive(Debug)]
pub struct ToStrError {
    _priv: (),
}
impl HeaderValue {
    #[inline]
    pub const fn from_static(src: &'static str) -> HeaderValue {
        let bytes = src.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if !is_visible_ascii(bytes[i]) {
                panic!("HeaderValue::from_static with invalid bytes")
            }
            i += 1;
        }
        HeaderValue {
            inner: Bytes::from_static(bytes),
            is_sensitive: false,
        }
    }
    #[inline]
    pub fn from_str(src: &str) -> Result<HeaderValue, InvalidHeaderValue> {
        HeaderValue::try_from_generic(src, |s| Bytes::copy_from_slice(s.as_bytes()))
    }
    #[inline]
    pub fn from_name(name: HeaderName) -> HeaderValue {
        name.into()
    }
    #[inline]
    pub fn from_bytes(src: &[u8]) -> Result<HeaderValue, InvalidHeaderValue> {
        HeaderValue::try_from_generic(src, Bytes::copy_from_slice)
    }
    pub fn from_maybe_shared<T>(src: T) -> Result<HeaderValue, InvalidHeaderValue>
    where
        T: AsRef<[u8]> + 'static,
    {
        if_downcast_into!(T, Bytes, src, {
            return HeaderValue::from_shared(src);
        });
        HeaderValue::from_bytes(src.as_ref())
    }
    pub unsafe fn from_maybe_shared_unchecked<T>(src: T) -> HeaderValue
    where
        T: AsRef<[u8]> + 'static,
    {
        if cfg!(debug_assertions) {
            match HeaderValue::from_maybe_shared(src) {
                Ok(val) => val,
                Err(_err) => {
                    panic!("HeaderValue::from_maybe_shared_unchecked() with invalid bytes");
                }
            }
        } else {
            if_downcast_into!(T, Bytes, src, {
                return HeaderValue {
                    inner: src,
                    is_sensitive: false,
                };
            });
            let src = Bytes::copy_from_slice(src.as_ref());
            HeaderValue {
                inner: src,
                is_sensitive: false,
            }
        }
    }
    fn from_shared(src: Bytes) -> Result<HeaderValue, InvalidHeaderValue> {
        HeaderValue::try_from_generic(src, std::convert::identity)
    }
    fn try_from_generic<T: AsRef<[u8]>, F: FnOnce(T) -> Bytes>(
        src: T,
        into: F,
    ) -> Result<HeaderValue, InvalidHeaderValue> {
        let mut bad = false;
        for &b in src.as_ref() {
            bad |= !is_valid(b);
        }
        if bad {
            return Err(InvalidHeaderValue { _priv: () });
        }
        Ok(HeaderValue {
            inner: into(src),
            is_sensitive: false,
        })
    }
    pub fn to_str(&self) -> Result<&str, ToStrError> {
        let bytes = self.as_ref();
        let mut bad = false;
        for &b in bytes {
            bad |= !is_visible_ascii(b);
        }
        if bad {
            return Err(ToStrError { _priv: () });
        }
        unsafe { Ok(str::from_utf8_unchecked(bytes)) }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.as_ref()
    }
    #[inline]
    pub fn set_sensitive(&mut self, val: bool) {
        self.is_sensitive = val;
    }
    #[inline]
    pub fn is_sensitive(&self) -> bool {
        self.is_sensitive
    }
}
impl AsRef<[u8]> for HeaderValue {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}
impl fmt::Debug for HeaderValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_sensitive {
            f.write_str("Sensitive")
        } else {
            f.write_str("\"")?;
            let mut from = 0;
            let bytes = self.as_bytes();
            for (i, &b) in bytes.iter().enumerate() {
                if !is_visible_ascii(b) || b == b'"' {
                    if from != i {
                        f.write_str(unsafe { str::from_utf8_unchecked(&bytes[from..i]) })?;
                    }
                    if b == b'"' {
                        f.write_str("\\\"")?;
                    } else {
                        write!(f, "\\x{:x}", b)?;
                    }
                    from = i + 1;
                }
            }
            f.write_str(unsafe { str::from_utf8_unchecked(&bytes[from..]) })?;
            f.write_str("\"")
        }
    }
}
impl From<HeaderName> for HeaderValue {
    #[inline]
    fn from(h: HeaderName) -> HeaderValue {
        HeaderValue {
            inner: h.into_bytes(),
            is_sensitive: false,
        }
    }
}
macro_rules! from_integers { ($($name : ident : $t : ident => $max_len : expr) ,*) => { $(impl From <$t > for HeaderValue { fn from (num : $t) -> HeaderValue { let mut buf = BytesMut::with_capacity ($max_len) ; let _ = buf . write_str (crate::itoa::Buffer::new () . format (num)) ; HeaderValue { inner : buf . freeze () , is_sensitive : false , } } } #[test] fn $name () { let n : $t = 55 ; let val = HeaderValue::from (n) ; assert_eq ! (val , & n . to_string ()) ; let n =::std::$t::MAX ; let val = HeaderValue::from (n) ; assert_eq ! (val , & n . to_string ()) ; }) * } ; }
from_integers! { from_u16 : u16 => 5 , from_i16 : i16 => 6 , from_u32 : u32 => 10 , from_i32 : i32 => 11 , from_u64 : u64 => 20 , from_i64 : i64 => 20 }
#[cfg(target_pointer_width = "16")]
from_integers! { from_usize : usize => 5 , from_isize : isize => 6 }
#[cfg(target_pointer_width = "32")]
from_integers! { from_usize : usize => 10 , from_isize : isize => 11 }
#[cfg(target_pointer_width = "64")]
from_integers! { from_usize : usize => 20 , from_isize : isize => 20 }
impl FromStr for HeaderValue {
    type Err = InvalidHeaderValue;
    #[inline]
    fn from_str(s: &str) -> Result<HeaderValue, Self::Err> {
        HeaderValue::from_str(s)
    }
}
impl From<&HeaderValue> for HeaderValue {
    #[inline]
    fn from(t: &HeaderValue) -> Self {
        t.clone()
    }
}
impl TryFrom<&str> for HeaderValue {
    type Error = InvalidHeaderValue;
    #[inline]
    fn try_from(t: &str) -> Result<Self, Self::Error> {
        t.parse()
    }
}
impl TryFrom<&String> for HeaderValue {
    type Error = InvalidHeaderValue;
    #[inline]
    fn try_from(s: &String) -> Result<Self, Self::Error> {
        Self::from_bytes(s.as_bytes())
    }
}
impl TryFrom<&[u8]> for HeaderValue {
    type Error = InvalidHeaderValue;
    #[inline]
    fn try_from(t: &[u8]) -> Result<Self, Self::Error> {
        HeaderValue::from_bytes(t)
    }
}
impl TryFrom<String> for HeaderValue {
    type Error = InvalidHeaderValue;
    #[inline]
    fn try_from(t: String) -> Result<Self, Self::Error> {
        HeaderValue::from_shared(t.into())
    }
}
impl TryFrom<Vec<u8>> for HeaderValue {
    type Error = InvalidHeaderValue;
    #[inline]
    fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
        HeaderValue::from_shared(vec.into())
    }
}
const fn is_visible_ascii(b: u8) -> bool {
    b >= 32 && b < 127 || b == b'\t'
}
#[inline]
fn is_valid(b: u8) -> bool {
    b >= 32 && b != 127 || b == b'\t'
}
impl fmt::Debug for InvalidHeaderValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InvalidHeaderValue").finish()
    }
}
impl fmt::Display for InvalidHeaderValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failed to parse header value")
    }
}
impl Error for InvalidHeaderValue {}
impl fmt::Display for ToStrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failed to convert header to a str")
    }
}
impl Error for ToStrError {}
impl Hash for HeaderValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}
impl PartialEq for HeaderValue {
    #[inline]
    fn eq(&self, other: &HeaderValue) -> bool {
        self.inner == other.inner
    }
}
impl Eq for HeaderValue {}
impl PartialOrd for HeaderValue {
    #[inline]
    fn partial_cmp(&self, other: &HeaderValue) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeaderValue {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.inner.cmp(&other.inner)
    }
}
impl PartialEq<str> for HeaderValue {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.inner == other.as_bytes()
    }
}
impl PartialEq<[u8]> for HeaderValue {
    #[inline]
    fn eq(&self, other: &[u8]) -> bool {
        self.inner == other
    }
}
impl PartialOrd<str> for HeaderValue {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        (*self.inner).partial_cmp(other.as_bytes())
    }
}
impl PartialOrd<[u8]> for HeaderValue {
    #[inline]
    fn partial_cmp(&self, other: &[u8]) -> Option<cmp::Ordering> {
        (*self.inner).partial_cmp(other)
    }
}
impl PartialEq<HeaderValue> for str {
    #[inline]
    fn eq(&self, other: &HeaderValue) -> bool {
        *other == *self
    }
}
impl PartialEq<HeaderValue> for [u8] {
    #[inline]
    fn eq(&self, other: &HeaderValue) -> bool {
        *other == *self
    }
}
impl PartialOrd<HeaderValue> for str {
    #[inline]
    fn partial_cmp(&self, other: &HeaderValue) -> Option<cmp::Ordering> {
        self.as_bytes().partial_cmp(other.as_bytes())
    }
}
impl PartialOrd<HeaderValue> for [u8] {
    #[inline]
    fn partial_cmp(&self, other: &HeaderValue) -> Option<cmp::Ordering> {
        self.partial_cmp(other.as_bytes())
    }
}
impl PartialEq<String> for HeaderValue {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        *self == other[..]
    }
}
impl PartialOrd<String> for HeaderValue {
    #[inline]
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        self.inner.partial_cmp(other.as_bytes())
    }
}
impl PartialEq<HeaderValue> for String {
    #[inline]
    fn eq(&self, other: &HeaderValue) -> bool {
        *other == *self
    }
}
impl PartialOrd<HeaderValue> for String {
    #[inline]
    fn partial_cmp(&self, other: &HeaderValue) -> Option<cmp::Ordering> {
        self.as_bytes().partial_cmp(other.as_bytes())
    }
}
impl PartialEq<HeaderValue> for &HeaderValue {
    #[inline]
    fn eq(&self, other: &HeaderValue) -> bool {
        **self == *other
    }
}
impl PartialOrd<HeaderValue> for &HeaderValue {
    #[inline]
    fn partial_cmp(&self, other: &HeaderValue) -> Option<cmp::Ordering> {
        (**self).partial_cmp(other)
    }
}
impl<T: ?Sized> PartialEq<&T> for HeaderValue
where
    HeaderValue: PartialEq<T>,
{
    #[inline]
    fn eq(&self, other: &&T) -> bool {
        *self == **other
    }
}
impl<T: ?Sized> PartialOrd<&T> for HeaderValue
where
    HeaderValue: PartialOrd<T>,
{
    #[inline]
    fn partial_cmp(&self, other: &&T) -> Option<cmp::Ordering> {
        self.partial_cmp(*other)
    }
}
impl PartialEq<HeaderValue> for &str {
    #[inline]
    fn eq(&self, other: &HeaderValue) -> bool {
        *other == *self
    }
}
impl PartialOrd<HeaderValue> for &str {
    #[inline]
    fn partial_cmp(&self, other: &HeaderValue) -> Option<cmp::Ordering> {
        self.as_bytes().partial_cmp(other.as_bytes())
    }
}
