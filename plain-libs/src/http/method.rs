use self::Inner::*;
use self::extension::{AllocatedExtension, InlineExtension};
use std::convert::TryFrom;
use std::error::Error;
use std::str::FromStr;
use std::{fmt, str};
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Method(Inner);
pub struct InvalidMethod {
    _priv: (),
}
#[derive(Clone, PartialEq, Eq, Hash)]
enum Inner {
    Options,
    Get,
    Post,
    Put,
    Delete,
    Head,
    Trace,
    Connect,
    Patch,
    Query,
    ExtensionInline(InlineExtension),
    ExtensionAllocated(AllocatedExtension),
}
impl Method {
    pub const GET: Method = Method(Get);
    pub const POST: Method = Method(Post);
    pub const PUT: Method = Method(Put);
    pub const DELETE: Method = Method(Delete);
    pub const HEAD: Method = Method(Head);
    pub const OPTIONS: Method = Method(Options);
    pub const CONNECT: Method = Method(Connect);
    pub const PATCH: Method = Method(Patch);
    pub const TRACE: Method = Method(Trace);
    pub const QUERY: Method = Method(Query);
    pub fn from_bytes(src: &[u8]) -> Result<Method, InvalidMethod> {
        match src.len() {
            0 => Err(InvalidMethod::new()),
            3 => match src {
                b"GET" => Ok(Method(Get)),
                b"PUT" => Ok(Method(Put)),
                _ => Method::extension_inline(src),
            },
            4 => match src {
                b"POST" => Ok(Method(Post)),
                b"HEAD" => Ok(Method(Head)),
                _ => Method::extension_inline(src),
            },
            5 => match src {
                b"PATCH" => Ok(Method(Patch)),
                b"TRACE" => Ok(Method(Trace)),
                b"QUERY" => Ok(Method(Query)),
                _ => Method::extension_inline(src),
            },
            6 => match src {
                b"DELETE" => Ok(Method(Delete)),
                _ => Method::extension_inline(src),
            },
            7 => match src {
                b"OPTIONS" => Ok(Method(Options)),
                b"CONNECT" => Ok(Method(Connect)),
                _ => Method::extension_inline(src),
            },
            _ => {
                if src.len() <= InlineExtension::MAX {
                    Method::extension_inline(src)
                } else {
                    let allocated = AllocatedExtension::new(src)?;
                    Ok(Method(ExtensionAllocated(allocated)))
                }
            }
        }
    }
    fn extension_inline(src: &[u8]) -> Result<Method, InvalidMethod> {
        let inline = InlineExtension::new(src)?;
        Ok(Method(ExtensionInline(inline)))
    }
    pub fn is_safe(&self) -> bool {
        matches!(self.0, Get | Head | Options | Trace | Query)
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        match self.0 {
            Options => "OPTIONS",
            Get => "GET",
            Post => "POST",
            Put => "PUT",
            Delete => "DELETE",
            Head => "HEAD",
            Trace => "TRACE",
            Connect => "CONNECT",
            Patch => "PATCH",
            Query => "QUERY",
            ExtensionInline(ref inline) => inline.as_str(),
            ExtensionAllocated(ref allocated) => allocated.as_str(),
        }
    }
}
impl AsRef<str> for Method {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl Ord for Method {
    #[inline]
    fn cmp(&self, other: &Method) -> std::cmp::Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}
impl PartialOrd for Method {
    #[inline]
    fn partial_cmp(&self, other: &Method) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq<&Method> for Method {
    #[inline]
    fn eq(&self, other: &&Method) -> bool {
        self == *other
    }
}
impl PartialEq<Method> for &Method {
    #[inline]
    fn eq(&self, other: &Method) -> bool {
        *self == other
    }
}
impl PartialEq<str> for Method {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_ref() == other
    }
}
impl PartialEq<Method> for str {
    #[inline]
    fn eq(&self, other: &Method) -> bool {
        self == other.as_ref()
    }
}
impl PartialEq<&str> for Method {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_ref() == *other
    }
}
impl PartialEq<Method> for &str {
    #[inline]
    fn eq(&self, other: &Method) -> bool {
        *self == other.as_ref()
    }
}
impl fmt::Debug for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}
impl fmt::Display for Method {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str(self.as_ref())
    }
}
impl Default for Method {
    #[inline]
    fn default() -> Method {
        Method::GET
    }
}
impl From<&Method> for Method {
    #[inline]
    fn from(t: &Method) -> Self {
        t.clone()
    }
}
impl TryFrom<&[u8]> for Method {
    type Error = InvalidMethod;
    #[inline]
    fn try_from(t: &[u8]) -> Result<Self, Self::Error> {
        Method::from_bytes(t)
    }
}
impl TryFrom<&str> for Method {
    type Error = InvalidMethod;
    #[inline]
    fn try_from(t: &str) -> Result<Self, Self::Error> {
        TryFrom::try_from(t.as_bytes())
    }
}
impl FromStr for Method {
    type Err = InvalidMethod;
    #[inline]
    fn from_str(t: &str) -> Result<Self, Self::Err> {
        TryFrom::try_from(t)
    }
}
impl InvalidMethod {
    fn new() -> InvalidMethod {
        InvalidMethod { _priv: () }
    }
}
impl fmt::Debug for InvalidMethod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InvalidMethod").finish()
    }
}
impl fmt::Display for InvalidMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid HTTP method")
    }
}
impl Error for InvalidMethod {}
mod extension {
    use super::InvalidMethod;
    use std::str;
    #[derive(Clone, PartialEq, Eq, Hash)]
    pub struct InlineExtension([u8; InlineExtension::MAX], u8);
    #[derive(Clone, PartialEq, Eq, Hash)]
    pub struct AllocatedExtension(Box<[u8]>);
    impl InlineExtension {
        pub const MAX: usize = 15;
        pub fn new(src: &[u8]) -> Result<InlineExtension, InvalidMethod> {
            let mut data: [u8; InlineExtension::MAX] = Default::default();
            write_checked(src, &mut data)?;
            Ok(InlineExtension(data, src.len() as u8))
        }
        pub fn as_str(&self) -> &str {
            let InlineExtension(data, len) = self;
            unsafe { str::from_utf8_unchecked(&data[..*len as usize]) }
        }
    }
    impl AllocatedExtension {
        pub fn new(src: &[u8]) -> Result<AllocatedExtension, InvalidMethod> {
            let mut data: Vec<u8> = vec![0; src.len()];
            write_checked(src, &mut data)?;
            Ok(AllocatedExtension(data.into_boxed_slice()))
        }
        pub fn as_str(&self) -> &str {
            unsafe { str::from_utf8_unchecked(&self.0) }
        }
    }
    #[rustfmt::skip] const METHOD_CHARS : [u8 ; 256] = [b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'!' , b'\0' , b'#' , b'$' , b'%' , b'&' , b'\'' , b'\0' , b'\0' , b'*' , b'+' , b'\0' , b'-' , b'.' , b'\0' , b'0' , b'1' , b'2' , b'3' , b'4' , b'5' , b'6' , b'7' , b'8' , b'9' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'A' , b'B' , b'C' , b'D' , b'E' , b'F' , b'G' , b'H' , b'I' , b'J' , b'K' , b'L' , b'M' , b'N' , b'O' , b'P' , b'Q' , b'R' , b'S' , b'T' , b'U' , b'V' , b'W' , b'X' , b'Y' , b'Z' , b'\0' , b'\0' , b'\0' , b'^' , b'_' , b'`' , b'a' , b'b' , b'c' , b'd' , b'e' , b'f' , b'g' , b'h' , b'i' , b'j' , b'k' , b'l' , b'm' , b'n' , b'o' , b'p' , b'q' , b'r' , b's' , b't' , b'u' , b'v' , b'w' , b'x' , b'y' , b'z' , b'\0' , b'|' , b'\0' , b'~' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0' , b'\0'] ;
    fn write_checked(src: &[u8], dst: &mut [u8]) -> Result<(), InvalidMethod> {
        for (i, &b) in src.iter().enumerate() {
            let b = METHOD_CHARS[b as usize];
            if b == 0 {
                return Err(InvalidMethod::new());
            }
            dst[i] = b;
        }
        Ok(())
    }
}
