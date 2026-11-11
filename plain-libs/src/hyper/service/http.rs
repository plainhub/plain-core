use crate::hyper::body::Body;
use crate::hyper::service::service::Service;
use crate::hyper::{Request, Response};
use std::error::Error as StdError;
use std::future::Future;
pub trait HttpService<ReqBody>: sealed::Sealed<ReqBody> {
    type ResBody: Body;
    type Error: Into<Box<dyn StdError + Send + Sync>>;
    type Future: Future<Output = Result<Response<Self::ResBody>, Self::Error>>;
    fn call(&mut self, req: Request<ReqBody>) -> Self::Future;
}
impl<T, B1, B2> HttpService<B1> for T
where
    T: Service<Request<B1>, Response = Response<B2>>,
    B2: Body,
    T::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type ResBody = B2;
    type Error = T::Error;
    type Future = T::Future;
    fn call(&mut self, req: Request<B1>) -> Self::Future {
        Service::call(self, req)
    }
}
impl<T, B1, B2> sealed::Sealed<B1> for T
where
    T: Service<Request<B1>, Response = Response<B2>>,
    B2: Body,
{
}
mod sealed {
    pub trait Sealed<T> {}
}
