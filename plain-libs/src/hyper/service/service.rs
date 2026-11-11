use std::future::Future;
pub trait Service<Request> {
    type Response;
    type Error;
    type Future: Future<Output = Result<Self::Response, Self::Error>>;
    fn call(&self, req: Request) -> Self::Future;
}
impl<Request, S: Service<Request> + ?Sized> Service<Request> for &'_ S {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    #[inline]
    fn call(&self, req: Request) -> Self::Future {
        (**self).call(req)
    }
}
impl<Request, S: Service<Request> + ?Sized> Service<Request> for &'_ mut S {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    #[inline]
    fn call(&self, req: Request) -> Self::Future {
        (**self).call(req)
    }
}
impl<Request, S: Service<Request> + ?Sized> Service<Request> for Box<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    #[inline]
    fn call(&self, req: Request) -> Self::Future {
        (**self).call(req)
    }
}
impl<Request, S: Service<Request> + ?Sized> Service<Request> for std::rc::Rc<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    #[inline]
    fn call(&self, req: Request) -> Self::Future {
        (**self).call(req)
    }
}
impl<Request, S: Service<Request> + ?Sized> Service<Request> for std::sync::Arc<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    #[inline]
    fn call(&self, req: Request) -> Self::Future {
        (**self).call(req)
    }
}
