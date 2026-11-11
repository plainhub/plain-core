use crate::hyper::body::Body;
use crate::hyper::service::service::Service;
use crate::hyper::{Request, Response};
use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
pub fn service_fn<F, R, S>(f: F) -> ServiceFn<F, R>
where
    F: Fn(Request<R>) -> S,
    S: Future,
{
    ServiceFn {
        f,
        _req: PhantomData,
    }
}
pub struct ServiceFn<F, R> {
    f: F,
    _req: PhantomData<fn(R)>,
}
impl<F, ReqBody, Ret, ResBody, E> Service<Request<ReqBody>> for ServiceFn<F, ReqBody>
where
    F: Fn(Request<ReqBody>) -> Ret,
    ReqBody: Body,
    Ret: Future<Output = Result<Response<ResBody>, E>>,
    E: Into<Box<dyn StdError + Send + Sync>>,
    ResBody: Body,
{
    type Response = crate::hyper::Response<ResBody>;
    type Error = E;
    type Future = Ret;
    fn call(&self, req: Request<ReqBody>) -> Self::Future {
        (self.f)(req)
    }
}
impl<F, R> fmt::Debug for ServiceFn<F, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("impl Service").finish()
    }
}
impl<F, R> Clone for ServiceFn<F, R>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        ServiceFn {
            f: self.f.clone(),
            _req: PhantomData,
        }
    }
}
impl<F, R> Copy for ServiceFn<F, R> where F: Copy {}
