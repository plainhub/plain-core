use super::Oneshot;
use pin_project_lite::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
#[derive(Debug, Copy, Clone)]
pub struct TowerToHyperService<S> {
    service: S,
}
impl<S> TowerToHyperService<S> {
    pub fn new(tower_service: S) -> Self {
        Self {
            service: tower_service,
        }
    }
}
impl<S, R> crate::hyper::service::Service<R> for TowerToHyperService<S>
where
    S: tower_service::Service<R> + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = TowerToHyperServiceFuture<S, R>;
    fn call(&self, req: R) -> Self::Future {
        TowerToHyperServiceFuture {
            future: Oneshot::new(self.service.clone(), req),
        }
    }
}
pin_project! { pub struct TowerToHyperServiceFuture < S , R > where S : tower_service::Service < R >, { #[pin] future : Oneshot < S , R >, } }
impl<S, R> Future for TowerToHyperServiceFuture<S, R>
where
    S: tower_service::Service<R>,
{
    type Output = Result<S::Response, S::Error>;
    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().future.poll(cx)
    }
}
