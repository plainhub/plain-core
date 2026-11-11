mod boxed_clone;
mod map_err;
mod map_future;
mod map_request;
mod map_response;
mod oneshot;
mod service_fn;
pub use boxed_clone::BoxCloneService;
pub use map_err::{MapErr, MapErrLayer};
pub use map_future::{MapFuture, MapFutureLayer};
pub use map_request::{MapRequest, MapRequestLayer};
pub use map_response::{MapResponse, MapResponseLayer};
pub use oneshot::Oneshot;
pub use service_fn::{ServiceFn, service_fn};
pub trait ServiceExt<Request>: tower_service::Service<Request> {
    fn oneshot(self, req: Request) -> Oneshot<Self, Request>
    where
        Self: Sized,
    {
        Oneshot::new(self, req)
    }

    fn map_response<F, Response>(self, f: F) -> MapResponse<Self, F>
    where
        Self: Sized,
        F: FnOnce(Self::Response) -> Response + Clone,
    {
        MapResponse::new(self, f)
    }
    fn map_err<F, Error>(self, f: F) -> MapErr<Self, F>
    where
        Self: Sized,
        F: FnOnce(Self::Error) -> Error + Clone,
    {
        MapErr::new(self, f)
    }

    fn map_request<F, NewRequest>(self, f: F) -> MapRequest<Self, F>
    where
        Self: Sized,
        F: FnMut(NewRequest) -> Request,
    {
        MapRequest::new(self, f)
    }

    fn map_future<F, Fut, Response, Error>(self, f: F) -> MapFuture<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Future) -> Fut,
        Error: From<Self::Error>,
        Fut: Future<Output = Result<Response, Error>>,
    {
        MapFuture::new(self, f)
    }
}
impl<T: ?Sized, Request> ServiceExt<Request> for T where T: tower_service::Service<Request> {}
