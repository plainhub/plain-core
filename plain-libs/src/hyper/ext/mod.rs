use crate::http::header::HeaderName;
use crate::http::header::{HeaderMap, IntoHeaderName, ValueIter};
use bytes::Bytes;
use std::fmt;
mod h1_reason_phrase;
pub use h1_reason_phrase::ReasonPhrase;
mod informational;
pub(crate) use informational::OnInformational;
pub use informational::on_informational;
#[derive(Clone, Eq, PartialEq)]
pub struct Protocol {
    inner: crate::h2::ext::Protocol,
}
impl Protocol {
    pub const fn from_static(value: &'static str) -> Self {
        Self {
            inner: crate::h2::ext::Protocol::from_static(value),
        }
    }
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }
    pub(crate) fn from_inner(inner: crate::h2::ext::Protocol) -> Self {
        Self { inner }
    }
    pub(crate) fn into_inner(self) -> crate::h2::ext::Protocol {
        self.inner
    }
}
impl<'a> From<&'a str> for Protocol {
    fn from(value: &'a str) -> Self {
        Self {
            inner: crate::h2::ext::Protocol::from(value),
        }
    }
}
impl AsRef<[u8]> for Protocol {
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}
impl fmt::Debug for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}
#[derive(Clone, Debug)]
pub(crate) struct HeaderCaseMap(HeaderMap<Bytes>);
impl HeaderCaseMap {
    pub(crate) fn get_all<'a>(
        &'a self,
        name: &HeaderName,
    ) -> impl Iterator<Item = impl AsRef<[u8]> + 'a> + 'a {
        self.get_all_internal(name)
    }
    pub(crate) fn get_all_internal(&self, name: &HeaderName) -> ValueIter<'_, Bytes> {
        self.0.get_all(name).into_iter()
    }
    pub(crate) fn default() -> Self {
        Self(HeaderMap::default())
    }
    pub(crate) fn append<N>(&mut self, name: N, orig: Bytes)
    where
        N: IntoHeaderName,
    {
        self.0.append(name, orig);
    }
}
