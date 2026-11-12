use crate::axum_core::body::Body;
use crate::axum_core::extract::{DefaultBodyLimitKind, Request};
mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::http::Request<crate::axum_core::body::Body> {}
}
pub trait RequestExt: sealed::Sealed + Sized {
    fn with_limited_body(self) -> Request;
    fn into_limited_body(self) -> Body;
}
impl RequestExt for Request {
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
