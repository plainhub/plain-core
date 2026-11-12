use std::future::Future;
use std::task::{Context, Poll};

// Adapted from tower-service0.3.3; see SOURCE.json and LICENSE.
pub trait Service<Request> {
    type Response;
    type Error;
    type Future: Future<Output = Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    fn call(&mut self, request: Request) -> Self::Future;
}

impl<S, Request> Service<Request> for &mut S
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        (**self).poll_ready(cx)
    }
    fn call(&mut self, request: Request) -> Self::Future {
        (**self).call(request)
    }
}

impl<S, Request> Service<Request> for Box<S>
where
    S: Service<Request> + ?Sized,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        (**self).poll_ready(cx)
    }
    fn call(&mut self, request: Request) -> Self::Future {
        (**self).call(request)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/tower/service.rs"]
mod tests;
