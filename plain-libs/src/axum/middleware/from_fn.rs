use crate::axum::response::{IntoResponse, Response};
use crate::axum_core::extract::{FromRequest, FromRequestParts, Request};
use crate::tower::{ServiceExt, util::BoxCloneService};
use futures_util::future::BoxFuture;
use std::{
    any::type_name,
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;
pub fn from_fn<F, T>(f: F) -> FromFnLayer<F, (), T> {
    from_fn_with_state((), f)
}
pub fn from_fn_with_state<F, S, T>(state: S, f: F) -> FromFnLayer<F, S, T> {
    FromFnLayer {
        f,
        state,
        _extractor: PhantomData,
    }
}
#[must_use]
pub struct FromFnLayer<F, S, T> {
    f: F,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}
impl<F, S, T> Clone for FromFnLayer<F, S, T>
where
    F: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            state: self.state.clone(),
            _extractor: self._extractor,
        }
    }
}
impl<S, I, F, T> Layer<I> for FromFnLayer<F, S, T>
where
    F: Clone,
    S: Clone,
{
    type Service = FromFn<F, S, I, T>;
    fn layer(&self, inner: I) -> Self::Service {
        FromFn {
            f: self.f.clone(),
            state: self.state.clone(),
            inner,
            _extractor: PhantomData,
        }
    }
}
impl<F, S, T> fmt::Debug for FromFnLayer<F, S, T>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromFnLayer")
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("state", &self.state)
            .finish()
    }
}
pub struct FromFn<F, S, I, T> {
    f: F,
    inner: I,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}
impl<F, S, I, T> Clone for FromFn<F, S, I, T>
where
    F: Clone,
    I: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            inner: self.inner.clone(),
            state: self.state.clone(),
            _extractor: self._extractor,
        }
    }
}
macro_rules! impl_service { ([$($ty : ident : $var : ident) ,*] , $last : ident : $last_var : ident) => { impl < F , Fut , Out , S , I , $($ty ,) * $last > Service < Request > for FromFn < F , S , I , ($($ty ,) * $last ,) > where F : FnMut ($($ty ,) * $last , Next) -> Fut + Clone + Send + 'static , $($ty : FromRequestParts < S > + Send ,) * $last : FromRequest < S > + Send , Fut : Future < Output = Out > + Send + 'static , Out : IntoResponse + 'static , I : Service < Request , Error = Infallible > + Clone + Send + 'static , I::Response : IntoResponse , I::Future : Send + 'static , S : Clone + Send + Sync + 'static , { type Response = Response ; type Error = Infallible ; type Future = ResponseFuture ; fn poll_ready (& mut self , cx : & mut Context <'_ >) -> Poll < Result < () , Self::Error >> { self . inner . poll_ready (cx) } fn call (& mut self , req : Request) -> Self::Future { let not_ready_inner = self . inner . clone () ; let ready_inner = std::mem::replace (& mut self . inner , not_ready_inner) ; let mut f = self . f . clone () ; let state = self . state . clone () ; let future = Box::pin (async move { let (mut parts , body) = req . into_parts () ; $(let $var = match $ty::from_request_parts (& mut parts , & state) . await { Ok (value) => value , Err (rejection) => return rejection . into_response () , } ;) * let req = Request::from_parts (parts , body) ; let $last_var = match $last::from_request (req , & state) . await { Ok (value) => value , Err (rejection) => return rejection . into_response () , } ; let inner = BoxCloneService::new(ready_inner.map_response(IntoResponse::into_response)) ; let next = Next { inner } ; f ($($var ,) * $last_var , next) . await . into_response () }) ; ResponseFuture { inner : future } } } } ; }
for_http_tuples!(last, impl_service);
impl<F, S, I, T> fmt::Debug for FromFn<F, S, I, T>
where
    S: fmt::Debug,
    I: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromFnLayer")
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("inner", &self.inner)
            .field("state", &self.state)
            .finish()
    }
}
#[derive(Debug, Clone)]
pub struct Next {
    inner: BoxCloneService<Request, Response, Infallible>,
}
impl Next {
    pub async fn run(mut self, req: Request) -> Response {
        match self.inner.call(req).await {
            Ok(res) => res,
            Err(err) => match err {},
        }
    }
}
impl Service<Request> for Next {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request) -> Self::Future {
        self.inner.call(req)
    }
}
pub struct ResponseFuture {
    inner: BoxFuture<'static, Response>,
}
impl Future for ResponseFuture {
    type Output = Result<Response, Infallible>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.as_mut().poll(cx).map(Ok)
    }
}
impl fmt::Debug for ResponseFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseFuture").finish()
    }
}
