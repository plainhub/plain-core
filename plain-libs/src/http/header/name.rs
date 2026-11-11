use crate::http::byte_str::ByteStr;
use bytes::{Bytes, BytesMut};
use std::borrow::Borrow;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::str::FromStr;
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct HeaderName {
    inner: Repr<Custom>,
}
#[derive(Debug, Hash)]
pub struct HdrName<'a> {
    inner: Repr<MaybeLower<'a>>,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum Repr<T> {
    Standard(StandardHeader),
    Custom(T),
}
#[derive(Debug, Clone, Eq, PartialEq)]
struct Custom(ByteStr);
#[derive(Debug, Clone)]
struct MaybeLower<'a> {
    buf: &'a [u8],
    lower: bool,
}
pub struct InvalidHeaderName {
    _priv: (),
}
macro_rules! standard_headers { ($($(#[$docs : meta]) * ($konst : ident , $upcase : ident , $name_bytes : literal) ;) +) => { #[derive (Debug , Clone , Copy , Eq , PartialEq , Hash)] enum StandardHeader { $($konst ,) + } $($(#[$docs]) * pub const $upcase : HeaderName = HeaderName { inner : Repr::Standard (StandardHeader::$konst) , } ;) + impl StandardHeader { #[inline] fn as_str (& self) -> &'static str { match * self { $(StandardHeader::$konst => unsafe { std::str::from_utf8_unchecked ($name_bytes) } ,) + } } const fn from_bytes (name_bytes : & [u8]) -> Option < StandardHeader > { match name_bytes { $($name_bytes => Some (StandardHeader::$konst) ,) + _ => None , } } } #[cfg (test)] const TEST_HEADERS : &'static [(StandardHeader , &'static [u8])] = & [$((StandardHeader::$konst , $name_bytes) ,) +] ; #[test] fn test_parse_standard_headers () { for & (std , name_bytes) in TEST_HEADERS { assert_eq ! (HeaderName::from_bytes (name_bytes) . unwrap () , HeaderName::from (std)) ; let upper = std::str::from_utf8 (name_bytes) . expect ("byte string constants are all utf-8") . to_uppercase () ; assert_eq ! (HeaderName::from_bytes (upper . as_bytes ()) . unwrap () , HeaderName::from (std)) ; } } #[test] fn test_standard_headers_into_bytes () { for & (std , name_bytes) in TEST_HEADERS { let name = std::str::from_utf8 (name_bytes) . unwrap () ; let std = HeaderName::from (std) ; let bytes : Bytes = HeaderName::from_bytes (name_bytes) . unwrap () . inner . into () ; assert_eq ! (bytes , name) ; assert_eq ! (HeaderName::from_bytes (name_bytes) . unwrap () , std) ; let upper = name . to_uppercase () ; let bytes : Bytes = HeaderName::from_bytes (upper . as_bytes ()) . unwrap () . inner . into () ; assert_eq ! (bytes , name_bytes) ; assert_eq ! (HeaderName::from_bytes (upper . as_bytes ()) . unwrap () , std) ; } } } }
standard_headers! { (Accept , ACCEPT , b"accept") ; (AcceptCharset , ACCEPT_CHARSET , b"accept-charset") ; (AcceptEncoding , ACCEPT_ENCODING , b"accept-encoding") ; (AcceptLanguage , ACCEPT_LANGUAGE , b"accept-language") ; (AcceptRanges , ACCEPT_RANGES , b"accept-ranges") ; (AccessControlAllowCredentials , ACCESS_CONTROL_ALLOW_CREDENTIALS , b"access-control-allow-credentials") ; (AccessControlAllowHeaders , ACCESS_CONTROL_ALLOW_HEADERS , b"access-control-allow-headers") ; (AccessControlAllowMethods , ACCESS_CONTROL_ALLOW_METHODS , b"access-control-allow-methods") ; (AccessControlAllowOrigin , ACCESS_CONTROL_ALLOW_ORIGIN , b"access-control-allow-origin") ; (AccessControlExposeHeaders , ACCESS_CONTROL_EXPOSE_HEADERS , b"access-control-expose-headers") ; (AccessControlMaxAge , ACCESS_CONTROL_MAX_AGE , b"access-control-max-age") ; (AccessControlRequestHeaders , ACCESS_CONTROL_REQUEST_HEADERS , b"access-control-request-headers") ; (AccessControlRequestMethod , ACCESS_CONTROL_REQUEST_METHOD , b"access-control-request-method") ; (Age , AGE , b"age") ; (Allow , ALLOW , b"allow") ; (AltSvc , ALT_SVC , b"alt-svc") ; (Authorization , AUTHORIZATION , b"authorization") ; (CacheControl , CACHE_CONTROL , b"cache-control") ; (CacheStatus , CACHE_STATUS , b"cache-status") ; (CdnCacheControl , CDN_CACHE_CONTROL , b"cdn-cache-control") ; (Connection , CONNECTION , b"connection") ; (ContentDisposition , CONTENT_DISPOSITION , b"content-disposition") ; (ContentEncoding , CONTENT_ENCODING , b"content-encoding") ; (ContentLanguage , CONTENT_LANGUAGE , b"content-language") ; (ContentLength , CONTENT_LENGTH , b"content-length") ; (ContentLocation , CONTENT_LOCATION , b"content-location") ; (ContentRange , CONTENT_RANGE , b"content-range") ; (ContentSecurityPolicy , CONTENT_SECURITY_POLICY , b"content-security-policy") ; (ContentSecurityPolicyReportOnly , CONTENT_SECURITY_POLICY_REPORT_ONLY , b"content-security-policy-report-only") ; (ContentType , CONTENT_TYPE , b"content-type") ; (Cookie , COOKIE , b"cookie") ; (Dnt , DNT , b"dnt") ; (Date , DATE , b"date") ; (Etag , ETAG , b"etag") ; (Expect , EXPECT , b"expect") ; (Expires , EXPIRES , b"expires") ; (Forwarded , FORWARDED , b"forwarded") ; (From , FROM , b"from") ; (Host , HOST , b"host") ; (IfMatch , IF_MATCH , b"if-match") ; (IfModifiedSince , IF_MODIFIED_SINCE , b"if-modified-since") ; (IfNoneMatch , IF_NONE_MATCH , b"if-none-match") ; (IfRange , IF_RANGE , b"if-range") ; (IfUnmodifiedSince , IF_UNMODIFIED_SINCE , b"if-unmodified-since") ; (LastModified , LAST_MODIFIED , b"last-modified") ; (Link , LINK , b"link") ; (Location , LOCATION , b"location") ; (MaxForwards , MAX_FORWARDS , b"max-forwards") ; (Origin , ORIGIN , b"origin") ; (Pragma , PRAGMA , b"pragma") ; (ProxyAuthenticate , PROXY_AUTHENTICATE , b"proxy-authenticate") ; (ProxyAuthorization , PROXY_AUTHORIZATION , b"proxy-authorization") ; (PublicKeyPins , PUBLIC_KEY_PINS , b"public-key-pins") ; (PublicKeyPinsReportOnly , PUBLIC_KEY_PINS_REPORT_ONLY , b"public-key-pins-report-only") ; (Range , RANGE , b"range") ; (Referer , REFERER , b"referer") ; (ReferrerPolicy , REFERRER_POLICY , b"referrer-policy") ; (Refresh , REFRESH , b"refresh") ; (RetryAfter , RETRY_AFTER , b"retry-after") ; (SecWebSocketAccept , SEC_WEBSOCKET_ACCEPT , b"sec-websocket-accept") ; (SecWebSocketExtensions , SEC_WEBSOCKET_EXTENSIONS , b"sec-websocket-extensions") ; (SecWebSocketKey , SEC_WEBSOCKET_KEY , b"sec-websocket-key") ; (SecWebSocketProtocol , SEC_WEBSOCKET_PROTOCOL , b"sec-websocket-protocol") ; (SecWebSocketVersion , SEC_WEBSOCKET_VERSION , b"sec-websocket-version") ; (Server , SERVER , b"server") ; (SetCookie , SET_COOKIE , b"set-cookie") ; (StrictTransportSecurity , STRICT_TRANSPORT_SECURITY , b"strict-transport-security") ; (Te , TE , b"te") ; (Trailer , TRAILER , b"trailer") ; (TransferEncoding , TRANSFER_ENCODING , b"transfer-encoding") ; (UserAgent , USER_AGENT , b"user-agent") ; (Upgrade , UPGRADE , b"upgrade") ; (UpgradeInsecureRequests , UPGRADE_INSECURE_REQUESTS , b"upgrade-insecure-requests") ; (Vary , VARY , b"vary") ; (Via , VIA , b"via") ; (Warning , WARNING , b"warning") ; (WwwAuthenticate , WWW_AUTHENTICATE , b"www-authenticate") ; (XContentTypeOptions , X_CONTENT_TYPE_OPTIONS , b"x-content-type-options") ; (XDnsPrefetchControl , X_DNS_PREFETCH_CONTROL , b"x-dns-prefetch-control") ; (XFrameOptions , X_FRAME_OPTIONS , b"x-frame-options") ; (XXssProtection , X_XSS_PROTECTION , b"x-xss-protection") ; }
#[rustfmt::skip] const HEADER_CHARS : [u8 ; 256] = [0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , b'!' , 0 , b'#' , b'$' , b'%' , b'&' , b'\'' , 0 , 0 , b'*' , b'+' , 0 , b'-' , b'.' , 0 , b'0' , b'1' , b'2' , b'3' , b'4' , b'5' , b'6' , b'7' , b'8' , b'9' , 0 , 0 , 0 , 0 , 0 , 0 , 0 , b'a' , b'b' , b'c' , b'd' , b'e' , b'f' , b'g' , b'h' , b'i' , b'j' , b'k' , b'l' , b'm' , b'n' , b'o' , b'p' , b'q' , b'r' , b's' , b't' , b'u' , b'v' , b'w' , b'x' , b'y' , b'z' , 0 , 0 , 0 , b'^' , b'_' , b'`' , b'a' , b'b' , b'c' , b'd' , b'e' , b'f' , b'g' , b'h' , b'i' , b'j' , b'k' , b'l' , b'm' , b'n' , b'o' , b'p' , b'q' , b'r' , b's' , b't' , b'u' , b'v' , b'w' , b'x' , b'y' , b'z' , 0 , b'|' , 0 , b'~' , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] ;
#[rustfmt::skip] const HEADER_CHARS_H2 : [u8 ; 256] = [0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , b'!' , b'"' , b'#' , b'$' , b'%' , b'&' , b'\'' , 0 , 0 , b'*' , b'+' , 0 , b'-' , b'.' , 0 , b'0' , b'1' , b'2' , b'3' , b'4' , b'5' , b'6' , b'7' , b'8' , b'9' , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , b'^' , b'_' , b'`' , b'a' , b'b' , b'c' , b'd' , b'e' , b'f' , b'g' , b'h' , b'i' , b'j' , b'k' , b'l' , b'm' , b'n' , b'o' , b'p' , b'q' , b'r' , b's' , b't' , b'u' , b'v' , b'w' , b'x' , b'y' , b'z' , 0 , b'|' , 0 , b'~' , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] ;
fn parse_hdr<'a>(
    data: &'a [u8],
    b: &'a mut [MaybeUninit<u8>; SCRATCH_BUF_SIZE],
    table: &[u8; 256],
) -> Result<HdrName<'a>, InvalidHeaderName> {
    match data.len() {
        0 => Err(InvalidHeaderName::new()),
        len @ 1..=SCRATCH_BUF_SIZE => {
            data.iter()
                .zip(b.iter_mut())
                .for_each(|(index, out)| *out = MaybeUninit::new(table[*index as usize]));
            let name: &'a [u8] = unsafe { slice_assume_init(&b[0..len]) };
            match StandardHeader::from_bytes(name) {
                Some(sh) => Ok(sh.into()),
                None => {
                    if name.contains(&0) {
                        Err(InvalidHeaderName::new())
                    } else {
                        Ok(HdrName::custom(name, true))
                    }
                }
            }
        }
        SCRATCH_BUF_OVERFLOW..=super::MAX_HEADER_NAME_LEN => Ok(HdrName::custom(data, false)),
        _ => Err(InvalidHeaderName::new()),
    }
}
impl<'a> From<StandardHeader> for HdrName<'a> {
    fn from(hdr: StandardHeader) -> HdrName<'a> {
        HdrName {
            inner: Repr::Standard(hdr),
        }
    }
}
impl HeaderName {
    pub fn from_bytes(src: &[u8]) -> Result<HeaderName, InvalidHeaderName> {
        let mut buf = uninit_u8_array();
        match parse_hdr(src, &mut buf, &HEADER_CHARS)?.inner {
            Repr::Standard(std) => Ok(std.into()),
            Repr::Custom(MaybeLower { buf, lower: true }) => {
                let buf = Bytes::copy_from_slice(buf);
                let val = unsafe { ByteStr::from_utf8_unchecked(buf) };
                Ok(Custom(val).into())
            }
            Repr::Custom(MaybeLower { buf, lower: false }) => {
                use bytes::BufMut;
                let mut dst = BytesMut::with_capacity(buf.len());
                for b in buf.iter() {
                    let b = HEADER_CHARS[*b as usize];
                    if b == 0 {
                        return Err(InvalidHeaderName::new());
                    }
                    dst.put_u8(b);
                }
                let val = unsafe { ByteStr::from_utf8_unchecked(dst.freeze()) };
                Ok(Custom(val).into())
            }
        }
    }
    pub fn from_lowercase(src: &[u8]) -> Result<HeaderName, InvalidHeaderName> {
        let mut buf = uninit_u8_array();
        match parse_hdr(src, &mut buf, &HEADER_CHARS_H2)?.inner {
            Repr::Standard(std) => Ok(std.into()),
            Repr::Custom(MaybeLower { buf, lower: true }) => {
                let buf = Bytes::copy_from_slice(buf);
                let val = unsafe { ByteStr::from_utf8_unchecked(buf) };
                Ok(Custom(val).into())
            }
            Repr::Custom(MaybeLower { buf, lower: false }) => {
                for &b in buf.iter() {
                    if HEADER_CHARS_H2[b as usize] == 0 {
                        return Err(InvalidHeaderName::new());
                    }
                }
                let buf = Bytes::copy_from_slice(buf);
                let val = unsafe { ByteStr::from_utf8_unchecked(buf) };
                Ok(Custom(val).into())
            }
        }
    }
    pub const fn from_static(src: &'static str) -> HeaderName {
        let name_bytes = src.as_bytes();
        if let Some(standard) = StandardHeader::from_bytes(name_bytes) {
            return HeaderName {
                inner: Repr::Standard(standard),
            };
        }
        if name_bytes.is_empty() || name_bytes.len() > super::MAX_HEADER_NAME_LEN || {
            let mut i = 0;
            loop {
                if i >= name_bytes.len() {
                    break false;
                } else if HEADER_CHARS_H2[name_bytes[i] as usize] == 0 {
                    break true;
                }
                i += 1;
            }
        } {
            panic!("HeaderName::from_static with invalid bytes")
        }
        HeaderName {
            inner: Repr::Custom(Custom(ByteStr::from_static(src))),
        }
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        match self.inner {
            Repr::Standard(v) => v.as_str(),
            Repr::Custom(ref v) => &v.0,
        }
    }
    pub(super) fn into_bytes(self) -> Bytes {
        self.inner.into()
    }
}
impl FromStr for HeaderName {
    type Err = InvalidHeaderName;
    fn from_str(s: &str) -> Result<HeaderName, InvalidHeaderName> {
        HeaderName::from_bytes(s.as_bytes()).map_err(|_| InvalidHeaderName { _priv: () })
    }
}
impl AsRef<str> for HeaderName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<[u8]> for HeaderName {
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}
impl Borrow<str> for HeaderName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
impl fmt::Debug for HeaderName {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), fmt)
    }
}
impl fmt::Display for HeaderName {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), fmt)
    }
}
impl InvalidHeaderName {
    pub(super) fn new() -> InvalidHeaderName {
        InvalidHeaderName { _priv: () }
    }
}
impl From<&HeaderName> for HeaderName {
    fn from(src: &HeaderName) -> HeaderName {
        src.clone()
    }
}
impl<T> From<Repr<T>> for Bytes
where
    T: Into<Bytes>,
{
    fn from(repr: Repr<T>) -> Bytes {
        match repr {
            Repr::Standard(header) => Bytes::from_static(header.as_str().as_bytes()),
            Repr::Custom(header) => header.into(),
        }
    }
}
impl From<Custom> for Bytes {
    #[inline]
    fn from(Custom(inner): Custom) -> Bytes {
        Bytes::from(inner)
    }
}
impl TryFrom<&str> for HeaderName {
    type Error = InvalidHeaderName;
    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::from_bytes(s.as_bytes())
    }
}
impl TryFrom<&String> for HeaderName {
    type Error = InvalidHeaderName;
    #[inline]
    fn try_from(s: &String) -> Result<Self, Self::Error> {
        Self::from_bytes(s.as_bytes())
    }
}
impl TryFrom<&[u8]> for HeaderName {
    type Error = InvalidHeaderName;
    #[inline]
    fn try_from(s: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(s)
    }
}
impl TryFrom<String> for HeaderName {
    type Error = InvalidHeaderName;
    #[inline]
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_bytes(s.as_bytes())
    }
}
impl TryFrom<Vec<u8>> for HeaderName {
    type Error = InvalidHeaderName;
    #[inline]
    fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
        Self::from_bytes(&vec)
    }
}
impl From<StandardHeader> for HeaderName {
    fn from(src: StandardHeader) -> HeaderName {
        HeaderName {
            inner: Repr::Standard(src),
        }
    }
}
impl From<Custom> for HeaderName {
    fn from(src: Custom) -> HeaderName {
        HeaderName {
            inner: Repr::Custom(src),
        }
    }
}
impl PartialEq<&HeaderName> for HeaderName {
    #[inline]
    fn eq(&self, other: &&HeaderName) -> bool {
        *self == **other
    }
}
impl PartialEq<HeaderName> for &HeaderName {
    #[inline]
    fn eq(&self, other: &HeaderName) -> bool {
        *other == *self
    }
}
impl PartialEq<str> for HeaderName {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        eq_ignore_ascii_case(self.as_ref(), other.as_bytes())
    }
}
impl PartialEq<HeaderName> for str {
    #[inline]
    fn eq(&self, other: &HeaderName) -> bool {
        *other == *self
    }
}
impl PartialEq<&str> for HeaderName {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        *self == **other
    }
}
impl PartialEq<HeaderName> for &str {
    #[inline]
    fn eq(&self, other: &HeaderName) -> bool {
        *other == *self
    }
}
impl fmt::Debug for InvalidHeaderName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InvalidHeaderName").finish()
    }
}
impl fmt::Display for InvalidHeaderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid HTTP header name")
    }
}
impl Error for InvalidHeaderName {}
impl<'a> HdrName<'a> {
    fn custom(buf: &'a [u8], lower: bool) -> HdrName<'a> {
        HdrName {
            inner: Repr::Custom(MaybeLower { buf, lower }),
        }
    }
    pub fn from_bytes<F, U>(hdr: &[u8], f: F) -> Result<U, InvalidHeaderName>
    where
        F: FnOnce(HdrName<'_>) -> U,
    {
        let mut buf = uninit_u8_array();
        let hdr = parse_hdr(hdr, &mut buf, &HEADER_CHARS)?;
        Ok(f(hdr))
    }
    pub fn from_static<F, U>(hdr: &'static str, f: F) -> U
    where
        F: FnOnce(HdrName<'_>) -> U,
    {
        let mut buf = uninit_u8_array();
        let hdr =
            parse_hdr(hdr.as_bytes(), &mut buf, &HEADER_CHARS).expect("static str is invalid name");
        f(hdr)
    }
}
impl<'a> From<HdrName<'a>> for HeaderName {
    fn from(src: HdrName<'a>) -> HeaderName {
        match src.inner {
            Repr::Standard(s) => HeaderName {
                inner: Repr::Standard(s),
            },
            Repr::Custom(maybe_lower) => {
                if maybe_lower.lower {
                    let buf = Bytes::copy_from_slice(maybe_lower.buf);
                    let byte_str = unsafe { ByteStr::from_utf8_unchecked(buf) };
                    HeaderName {
                        inner: Repr::Custom(Custom(byte_str)),
                    }
                } else {
                    use bytes::BufMut;
                    let mut dst = BytesMut::with_capacity(maybe_lower.buf.len());
                    for b in maybe_lower.buf.iter() {
                        dst.put_u8(HEADER_CHARS[*b as usize]);
                    }
                    let buf = unsafe { ByteStr::from_utf8_unchecked(dst.freeze()) };
                    HeaderName {
                        inner: Repr::Custom(Custom(buf)),
                    }
                }
            }
        }
    }
}
impl<'a> PartialEq<HdrName<'a>> for HeaderName {
    #[inline]
    fn eq(&self, other: &HdrName<'a>) -> bool {
        match self.inner {
            Repr::Standard(a) => match other.inner {
                Repr::Standard(b) => a == b,
                _ => false,
            },
            Repr::Custom(Custom(ref a)) => match other.inner {
                Repr::Custom(ref b) => {
                    if b.lower {
                        a.as_bytes() == b.buf
                    } else {
                        eq_ignore_ascii_case(a.as_bytes(), b.buf)
                    }
                }
                _ => false,
            },
        }
    }
}
impl Hash for Custom {
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        hasher.write(self.0.as_bytes())
    }
}
impl<'a> Hash for MaybeLower<'a> {
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        if self.lower {
            hasher.write(self.buf);
        } else {
            for &b in self.buf {
                hasher.write(&[HEADER_CHARS[b as usize]]);
            }
        }
    }
}
#[inline]
fn eq_ignore_ascii_case(lower: &[u8], s: &[u8]) -> bool {
    if lower.len() != s.len() {
        return false;
    }
    lower
        .iter()
        .zip(s)
        .all(|(a, b)| *a == HEADER_CHARS[*b as usize])
}
const SCRATCH_BUF_SIZE: usize = 64;
const SCRATCH_BUF_OVERFLOW: usize = SCRATCH_BUF_SIZE + 1;
fn uninit_u8_array() -> [MaybeUninit<u8>; SCRATCH_BUF_SIZE] {
    let arr = MaybeUninit::<[MaybeUninit<u8>; SCRATCH_BUF_SIZE]>::uninit();
    unsafe { arr.assume_init() }
}
unsafe fn slice_assume_init<T>(slice: &[MaybeUninit<T>]) -> &[T] {
    unsafe { &*(slice as *const [MaybeUninit<T>] as *const [T]) }
}
