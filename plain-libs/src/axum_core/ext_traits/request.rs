use crate::axum_core::body::Body;
use crate::axum_core::extract::{DefaultBodyLimitKind, FromRequest, FromRequestParts, Request};
use futures_util::future::BoxFuture;
mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::http::Request<crate::axum_core::body::Body> {}
}
pub trait RequestExt: sealed::Sealed + Sized {
    fn extract<E, M>(self) -> BoxFuture<'static, Result<E, E::Rejection>>
    where
        E: FromRequest<(), M> + 'static,
        M: 'static;
    fn extract_with_state<E, S, M>(self, state: &S) -> BoxFuture<'_, Result<E, E::Rejection>>
    where
        E: FromRequest<S, M> + 'static,
        S: Send + Sync;
    fn extract_parts<E>(&mut self) -> BoxFuture<'_, Result<E, E::Rejection>>
    where
        E: FromRequestParts<()> + 'static;
    fn extract_parts_with_state<'a, E, S>(
        &'a mut self,
        state: &'a S,
    ) -> BoxFuture<'a, Result<E, E::Rejection>>
    where
        E: FromRequestParts<S> + 'static,
        S: Send + Sync;
    fn with_limited_body(self) -> Request;
    fn into_limited_body(self) -> Body;
}
impl RequestExt for Request {
    fn extract<E, M>(self) -> BoxFuture<'static, Result<E, E::Rejection>>
    where
        E: FromRequest<(), M> + 'static,
        M: 'static,
    {
        self.extract_with_state(&())
    }
    fn extract_with_state<E, S, M>(self, state: &S) -> BoxFuture<'_, Result<E, E::Rejection>>
    where
        E: FromRequest<S, M> + 'static,
        S: Send + Sync,
    {
        E::from_request(self, state)
    }
    fn extract_parts<E>(&mut self) -> BoxFuture<'_, Result<E, E::Rejection>>
    where
        E: FromRequestParts<()> + 'static,
    {
        self.extract_parts_with_state(&())
    }
    fn extract_parts_with_state<'a, E, S>(
        &'a mut self,
        state: &'a S,
    ) -> BoxFuture<'a, Result<E, E::Rejection>>
    where
        E: FromRequestParts<S> + 'static,
        S: Send + Sync,
    {
        let mut req = Request::new(());
        *req.version_mut() = self.version();
        *req.method_mut() = self.method().clone();
        *req.uri_mut() = self.uri().clone();
        *req.headers_mut() = std::mem::take(self.headers_mut());
        *req.extensions_mut() = std::mem::take(self.extensions_mut());
        let (mut parts, ()) = req.into_parts();
        Box::pin(async move {
            let result = E::from_request_parts(&mut parts, state).await;
            *self.version_mut() = parts.version;
            *self.method_mut() = parts.method.clone();
            *self.uri_mut() = parts.uri.clone();
            *self.headers_mut() = std::mem::take(&mut parts.headers);
            *self.extensions_mut() = std::mem::take(&mut parts.extensions);
            result
        })
    }
    fn with_limited_body(self) -> Request {
        const DEFAULT_LIMIT: usize = 2_097_152;
        match self.extensions().get::<DefaultBodyLimitKind>().copied() {
            Some(DefaultBodyLimitKind::Disable) => self,
            Some(DefaultBodyLimitKind::Limit(limit)) => {
                self.map(|b| Body::new(crate::http_body_util::Limited::new(b, limit)))
            }
            None => self.map(|b| Body::new(crate::http_body_util::Limited::new(b, DEFAULT_LIMIT))),
        }
    }
    fn into_limited_body(self) -> Body {
        self.with_limited_body().into_body()
    }
}
