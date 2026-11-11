use crate::axum::response::Response;
use crate::axum_core::response::IntoResponse;
use crate::http::{Request, StatusCode};
use std::{
    convert::Infallible,
    future::ready,
    task::{Context, Poll},
};
use tower_service::Service;
#[derive(Clone, Copy, Debug)]
pub(super) struct NotFound;
impl<B> Service<Request<B>> for NotFound
where
    B: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = std::future::Ready<Result<Response, Self::Error>>;
    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, _req: Request<B>) -> Self::Future {
        ready(Ok(StatusCode::NOT_FOUND.into_response()))
    }
}
