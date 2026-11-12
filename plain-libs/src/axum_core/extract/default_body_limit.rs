use self::private::DefaultBodyLimitService;
use crate::tower::layer::Layer;
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct DefaultBodyLimit {
    kind: DefaultBodyLimitKind,
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum DefaultBodyLimitKind {
    Disable,
    Limit(usize),
}
impl DefaultBodyLimit {
    pub const fn disable() -> Self {
        Self {
            kind: DefaultBodyLimitKind::Disable,
        }
    }
    pub const fn max(limit: usize) -> Self {
        Self {
            kind: DefaultBodyLimitKind::Limit(limit),
        }
    }
}
impl<S> Layer<S> for DefaultBodyLimit {
    type Service = DefaultBodyLimitService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        DefaultBodyLimitService {
            inner,
            kind: self.kind,
        }
    }
}
mod private {
    use super::DefaultBodyLimitKind;
    use crate::http::Request;
    use std::task::Context;
    use crate::tower::Service;
    #[derive(Debug, Clone, Copy)]
    pub struct DefaultBodyLimitService<S> {
        pub(super) inner: S,
        pub(super) kind: DefaultBodyLimitKind,
    }
    impl<B, S> Service<Request<B>> for DefaultBodyLimitService<S>
    where
        S: Service<Request<B>>,
    {
        type Response = S::Response;
        type Error = S::Error;
        type Future = S::Future;
        #[inline]
        fn poll_ready(&mut self, cx: &mut Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
            self.inner.poll_ready(cx)
        }
        #[inline]
        fn call(&mut self, mut req: Request<B>) -> Self::Future {
            req.extensions_mut().insert(self.kind);
            self.inner.call(req)
        }
    }
}
