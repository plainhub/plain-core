use crate::http::header::{HeaderMap, HeaderName, HeaderValue};
use crate::http::method::Method;
use crate::http::version::Version;
use crate::http::{Extensions, Result, Uri};
use std::any::Any;
use std::convert::TryInto;
use std::fmt;
#[derive(Clone)]
pub struct Request<T> {
    head: Parts,
    body: T,
}
#[derive(Clone)]
pub struct Parts {
    pub method: Method,
    pub uri: Uri,
    pub version: Version,
    pub headers: HeaderMap<HeaderValue>,
    pub extensions: Extensions,
    _priv: (),
}
#[derive(Debug)]
pub struct Builder {
    inner: Result<Parts>,
}
impl Request<()> {
    #[inline]
    pub fn builder() -> Builder {
        Builder::new()
    }
    pub fn get<T>(uri: T) -> Builder
    where
        T: TryInto<Uri>,
        <T as TryInto<Uri>>::Error: Into<crate::http::Error>,
    {
        Builder::new().method(Method::GET).uri(uri)
    }
    pub fn put<T>(uri: T) -> Builder
    where
        T: TryInto<Uri>,
        <T as TryInto<Uri>>::Error: Into<crate::http::Error>,
    {
        Builder::new().method(Method::PUT).uri(uri)
    }
    pub fn post<T>(uri: T) -> Builder
    where
        T: TryInto<Uri>,
        <T as TryInto<Uri>>::Error: Into<crate::http::Error>,
    {
        Builder::new().method(Method::POST).uri(uri)
    }
    pub fn delete<T>(uri: T) -> Builder
    where
        T: TryInto<Uri>,
        <T as TryInto<Uri>>::Error: Into<crate::http::Error>,
    {
        Builder::new().method(Method::DELETE).uri(uri)
    }
    pub fn options<T>(uri: T) -> Builder
    where
        T: TryInto<Uri>,
        <T as TryInto<Uri>>::Error: Into<crate::http::Error>,
    {
        Builder::new().method(Method::OPTIONS).uri(uri)
    }
    pub fn head<T>(uri: T) -> Builder
    where
        T: TryInto<Uri>,
        <T as TryInto<Uri>>::Error: Into<crate::http::Error>,
    {
        Builder::new().method(Method::HEAD).uri(uri)
    }
    pub fn connect<T>(uri: T) -> Builder
    where
        T: TryInto<Uri>,
        <T as TryInto<Uri>>::Error: Into<crate::http::Error>,
    {
        Builder::new().method(Method::CONNECT).uri(uri)
    }
    pub fn patch<T>(uri: T) -> Builder
    where
        T: TryInto<Uri>,
        <T as TryInto<Uri>>::Error: Into<crate::http::Error>,
    {
        Builder::new().method(Method::PATCH).uri(uri)
    }
    pub fn trace<T>(uri: T) -> Builder
    where
        T: TryInto<Uri>,
        <T as TryInto<Uri>>::Error: Into<crate::http::Error>,
    {
        Builder::new().method(Method::TRACE).uri(uri)
    }
}
impl<T> Request<T> {
    #[inline]
    pub fn new(body: T) -> Request<T> {
        Request {
            head: Parts::new(),
            body,
        }
    }
    #[inline]
    pub fn from_parts(parts: Parts, body: T) -> Request<T> {
        Request { head: parts, body }
    }
    #[inline]
    pub fn method(&self) -> &Method {
        &self.head.method
    }
    #[inline]
    pub fn method_mut(&mut self) -> &mut Method {
        &mut self.head.method
    }
    #[inline]
    pub fn uri(&self) -> &Uri {
        &self.head.uri
    }
    #[inline]
    pub fn uri_mut(&mut self) -> &mut Uri {
        &mut self.head.uri
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
    pub fn map<F, U>(self, f: F) -> Request<U>
    where
        F: FnOnce(T) -> U,
    {
        Request {
            body: f(self.body),
            head: self.head,
        }
    }
}
impl<T: Default> Default for Request<T> {
    fn default() -> Request<T> {
        Request::new(T::default())
    }
}
impl<T: fmt::Debug> fmt::Debug for Request<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("method", self.method())
            .field("uri", self.uri())
            .field("version", &self.version())
            .field("headers", self.headers())
            .field("body", self.body())
            .finish()
    }
}
impl Parts {
    fn new() -> Parts {
        Parts {
            method: Method::default(),
            uri: Uri::default(),
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
            .field("method", &self.method)
            .field("uri", &self.uri)
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
    pub fn method<T>(self, method: T) -> Builder
    where
        T: TryInto<Method>,
        <T as TryInto<Method>>::Error: Into<crate::http::Error>,
    {
        self.and_then(move |mut head| {
            let method = method.try_into().map_err(Into::into)?;
            head.method = method;
            Ok(head)
        })
    }

    pub fn uri<T>(self, uri: T) -> Builder
    where
        T: TryInto<Uri>,
        <T as TryInto<Uri>>::Error: Into<crate::http::Error>,
    {
        self.and_then(move |mut head| {
            head.uri = uri.try_into().map_err(Into::into)?;
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
    pub fn body<T>(self, body: T) -> Result<Request<T>> {
        self.inner.map(move |head| Request { head, body })
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
