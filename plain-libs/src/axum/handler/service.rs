use super::Handler;
use crate::axum::BoxError;
use crate::axum::body::{Body, Bytes, HttpBody};
use crate::axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use crate::axum::response::Response;
use crate::axum::routing::IntoMakeService;
use crate::http::Request;
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower_service::Service;
pub struct HandlerService<H, T, S> {
    handler: H,
    state: S,
    _marker: PhantomData<fn() -> T>,
}
impl<H, T, S> HandlerService<H, T, S> {
    pub fn state(&self) -> &S {
        &self.state
    }
    pub fn into_make_service(self) -> IntoMakeService<HandlerService<H, T, S>> {
        IntoMakeService::new(self)
    }
    pub fn into_make_service_with_connect_info<C>(
        self,
    ) -> IntoMakeServiceWithConnectInfo<HandlerService<H, T, S>, C> {
        IntoMakeServiceWithConnectInfo::new(self)
    }
}
impl<H, T, S> HandlerService<H, T, S> {
    pub(super) fn new(handler: H, state: S) -> Self {
        Self {
            handler,
            state,
            _marker: PhantomData,
        }
    }
}
impl<H, T, S> fmt::Debug for HandlerService<H, T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoService").finish_non_exhaustive()
    }
}
impl<H, T, S> Clone for HandlerService<H, T, S>
where
    H: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            state: self.state.clone(),
            _marker: PhantomData,
        }
    }
}
impl<H, T, S, B> Service<Request<B>> for HandlerService<H, T, S>
where
    H: Handler<T, S> + Clone + Send + 'static,
    B: HttpBody<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
    S: Clone + Send + Sync,
{
    type Response = Response;
    type Error = Infallible;
    type Future = super::future::IntoServiceFuture<H::Future>;
    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        use futures_util::future::FutureExt;
        let req = req.map(Body::new);
        let handler = self.handler.clone();
        let future = Handler::call(handler, req, self.state.clone());
        let future = future.map(Ok as _);
        super::future::IntoServiceFuture::new(future)
    }
}
const _: () = {
    use crate::axum::serve::IncomingStream;
    impl<H, T, S> Service<IncomingStream<'_>> for HandlerService<H, T, S>
    where
        H: Clone,
        S: Clone,
    {
        type Response = Self;
        type Error = Infallible;
        type Future = std::future::Ready<Result<Self::Response, Self::Error>>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, _req: IncomingStream<'_>) -> Self::Future {
            std::future::ready(Ok(self.clone()))
        }
    }
};
