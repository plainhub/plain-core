use futures_util::{TryFutureExt, future::MapOk};
use std::fmt;
use std::task::{Context, Poll};
use crate::tower::layer::Layer;
use crate::tower::Service;
#[derive(Clone)]
pub struct MapResponse<S, F> {
    inner: S,
    f: F,
}
impl<S, F> fmt::Debug for MapResponse<S, F>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapResponse")
            .field("inner", &self.inner)
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}
#[derive(Debug, Clone)]
pub struct MapResponseLayer<F> {
    f: F,
}
opaque_future! { pub type MapResponseFuture < F , N > = MapOk < F , N >; }
impl<S, F> MapResponse<S, F> {
    pub const fn new(inner: S, f: F) -> Self {
        MapResponse { f, inner }
    }
    pub fn layer(f: F) -> MapResponseLayer<F> {
        MapResponseLayer { f }
    }
}
impl<S, F, Request, Response> Service<Request> for MapResponse<S, F>
where
    S: Service<Request>,
    F: FnOnce(S::Response) -> Response + Clone,
{
    type Response = Response;
    type Error = S::Error;
    type Future = MapResponseFuture<S::Future, F>;
    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    #[inline]
    fn call(&mut self, request: Request) -> Self::Future {
        MapResponseFuture::new(self.inner.call(request).map_ok(self.f.clone()))
    }
}
impl<F> MapResponseLayer<F> {
    pub const fn new(f: F) -> Self {
        MapResponseLayer { f }
    }
}
impl<S, F> Layer<S> for MapResponseLayer<F>
where
    F: Clone,
{
    type Service = MapResponse<S, F>;
    fn layer(&self, inner: S) -> Self::Service {
        MapResponse {
            f: self.f.clone(),
            inner,
        }
    }
}
