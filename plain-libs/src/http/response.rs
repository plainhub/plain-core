use crate::http::header::{HeaderMap, HeaderName, HeaderValue};
use crate::http::status::StatusCode;
use crate::http::version::Version;
use crate::http::{Extensions, Result};
use std::any::Any;
use std::convert::TryInto;
use std::fmt;
#[derive(Clone)]
pub struct Response<T> {
    head: Parts,
    body: T,
}
#[derive(Clone)]
pub struct Parts {
    pub status: StatusCode,
    pub version: Version,
    pub headers: HeaderMap<HeaderValue>,
    pub extensions: Extensions,
    _priv: (),
}
#[derive(Debug)]
pub struct Builder {
    inner: Result<Parts>,
}
impl Response<()> {
    #[inline]
    pub fn builder() -> Builder {
        Builder::new()
    }
}
impl<T> Response<T> {
    #[inline]
    pub fn new(body: T) -> Response<T> {
        Response {
            head: Parts::new(),
            body,
        }
    }
    #[inline]
    pub fn from_parts(parts: Parts, body: T) -> Response<T> {
        Response { head: parts, body }
    }
    #[inline]
    pub fn status(&self) -> StatusCode {
        self.head.status
    }
    #[inline]
    pub fn status_mut(&mut self) -> &mut StatusCode {
        &mut self.head.status
    }
    #[inline]
    pub fn version(&self) -> Version {
        self.head.version
    }
    #[inline]
    pub fn version_mut(&mut self) -> &mut Version {
        &mut self.head.version
    }
    #[inline]
    pub fn headers(&self) -> &HeaderMap<HeaderValue> {
        &self.head.headers
    }
    #[inline]
    pub fn headers_mut(&mut self) -> &mut HeaderMap<HeaderValue> {
        &mut self.head.headers
    }
    #[inline]
    pub fn extensions(&self) -> &Extensions {
        &self.head.extensions
    }
    #[inline]
    pub fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.head.extensions
    }
    #[inline]
    pub fn body(&self) -> &T {
        &self.body
    }
    #[inline]
    pub fn body_mut(&mut self) -> &mut T {
        &mut self.body
    }
    #[inline]
    pub fn into_body(self) -> T {
        self.body
    }
    #[inline]
    pub fn into_parts(self) -> (Parts, T) {
        (self.head, self.body)
    }
    #[inline]
    pub fn map<F, U>(self, f: F) -> Response<U>
    where
        F: FnOnce(T) -> U,
    {
        Response {
            body: f(self.body),
            head: self.head,
        }
    }
}
impl<T: Default> Default for Response<T> {
    #[inline]
    fn default() -> Response<T> {
        Response::new(T::default())
    }
}
impl<T: fmt::Debug> fmt::Debug for Response<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status())
            .field("version", &self.version())
            .field("headers", self.headers())
            .field("body", self.body())
            .finish()
    }
}
impl Parts {
    fn new() -> Parts {
        Parts {
            status: StatusCode::default(),
            version: Version::default(),
            headers: HeaderMap::default(),
            extensions: Extensions::default(),
            _priv: (),
        }
    }
}
impl fmt::Debug for Parts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Parts")
            .field("status", &self.status)
            .field("version", &self.version)
            .field("headers", &self.headers)
            .finish()
    }
}
impl Builder {
    #[inline]
    pub fn new() -> Builder {
        Builder::default()
    }
    pub fn status<T>(self, status: T) -> Builder
    where
        T: TryInto<StatusCode>,
        <T as TryInto<StatusCode>>::Error: Into<crate::http::Error>,
    {
        self.and_then(move |mut head| {
            head.status = status.try_into().map_err(Into::into)?;
            Ok(head)
        })
    }
    pub fn version(self, version: Version) -> Builder {
        self.and_then(move |mut head| {
            head.version = version;
            Ok(head)
        })
    }
    pub fn header<K, V>(self, key: K, value: V) -> Builder
    where
        K: TryInto<HeaderName>,
        <K as TryInto<HeaderName>>::Error: Into<crate::http::Error>,
        V: TryInto<HeaderValue>,
        <V as TryInto<HeaderValue>>::Error: Into<crate::http::Error>,
    {
        self.and_then(move |mut head| {
            let name = key.try_into().map_err(Into::into)?;
            let value = value.try_into().map_err(Into::into)?;
            head.headers.try_append(name, value)?;
            Ok(head)
        })
    }
    pub fn headers_ref(&self) -> Option<&HeaderMap<HeaderValue>> {
        self.inner.as_ref().ok().map(|h| &h.headers)
    }
    pub fn headers_mut(&mut self) -> Option<&mut HeaderMap<HeaderValue>> {
        self.inner.as_mut().ok().map(|h| &mut h.headers)
    }
    pub fn extension<T>(self, extension: T) -> Builder
    where
        T: Clone + Any + Send + Sync + 'static,
    {
        self.and_then(move |mut head| {
            head.extensions.insert(extension);
            Ok(head)
        })
    }
    pub fn extensions_ref(&self) -> Option<&Extensions> {
        self.inner.as_ref().ok().map(|h| &h.extensions)
    }
    pub fn extensions_mut(&mut self) -> Option<&mut Extensions> {
        self.inner.as_mut().ok().map(|h| &mut h.extensions)
    }
    pub fn body<T>(self, body: T) -> Result<Response<T>> {
        self.inner.map(move |head| Response { head, body })
    }
    fn and_then<F>(self, func: F) -> Self
    where
        F: FnOnce(Parts) -> Result<Parts>,
    {
        Builder {
            inner: self.inner.and_then(func),
        }
    }
}
impl Default for Builder {
    #[inline]
    fn default() -> Builder {
        Builder {
            inner: Ok(Parts::new()),
        }
    }
}
