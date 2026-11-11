use super::{Authority, Parts, PathAndQuery, Scheme};
use crate::http::Uri;
use std::convert::TryInto;
#[derive(Debug)]
pub struct Builder {
    parts: Result<Parts, crate::http::Error>,
}
impl Builder {
    #[inline]
    pub fn new() -> Builder {
        Builder::default()
    }
    pub fn scheme<T>(self, scheme: T) -> Self
    where
        T: TryInto<Scheme>,
        <T as TryInto<Scheme>>::Error: Into<crate::http::Error>,
    {
        self.map(move |mut parts| {
            let scheme = scheme.try_into().map_err(Into::into)?;
            parts.scheme = Some(scheme);
            Ok(parts)
        })
    }
    pub fn authority<T>(self, auth: T) -> Self
    where
        T: TryInto<Authority>,
        <T as TryInto<Authority>>::Error: Into<crate::http::Error>,
    {
        self.map(move |mut parts| {
            let auth = auth.try_into().map_err(Into::into)?;
            parts.authority = Some(auth);
            Ok(parts)
        })
    }
    pub fn path_and_query<T>(self, p_and_q: T) -> Self
    where
        T: TryInto<PathAndQuery>,
        <T as TryInto<PathAndQuery>>::Error: Into<crate::http::Error>,
    {
        self.map(move |mut parts| {
            let p_and_q = match p_and_q.try_into() {
                Ok(p_and_q) => p_and_q,
                Err(err) => {
                    let err = err.into();
                    if err.is_empty_uri() {
                        PathAndQuery::empty()
                    } else {
                        return Err(err);
                    }
                }
            };
            parts.path_and_query = Some(p_and_q);
            Ok(parts)
        })
    }
    pub fn build(self) -> Result<Uri, crate::http::Error> {
        let parts = self.parts?;
        Uri::from_parts(parts).map_err(Into::into)
    }
    fn map<F>(self, func: F) -> Self
    where
        F: FnOnce(Parts) -> Result<Parts, crate::http::Error>,
    {
        Builder {
            parts: self.parts.and_then(func),
        }
    }
}
impl Default for Builder {
    #[inline]
    fn default() -> Builder {
        Builder {
            parts: Ok(Parts::default()),
        }
    }
}
impl From<Uri> for Builder {
    fn from(uri: Uri) -> Self {
        Self {
            parts: Ok(uri.into_parts()),
        }
    }
}
