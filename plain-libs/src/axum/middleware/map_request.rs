use crate::axum::BoxError;
use crate::axum::body::{Body, Bytes, HttpBody};
use crate::axum::response::{IntoResponse, Response};
use crate::axum_core::extract::{FromRequest, FromRequestParts};
use crate::http::Request;
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
pub fn map_request<F, T>(f: F) -> MapRequestLayer<F, (), T> {
    map_request_with_state((), f)
}
pub fn map_request_with_state<F, S, T>(state: S, f: F) -> MapRequestLayer<F, S, T> {
    MapRequestLayer {
        f,
        state,
        _extractor: PhantomData,
    }
}
#[must_use]
pub struct MapRequestLayer<F, S, T> {
    f: F,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}
impl<F, S, T> Clone for MapRequestLayer<F, S, T>
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
impl<S, I, F, T> Layer<I> for MapRequestLayer<F, S, T>
where
    F: Clone,
    S: Clone,
{
    type Service = MapRequest<F, S, I, T>;
    fn layer(&self, inner: I) -> Self::Service {
        MapRequest {
            f: self.f.clone(),
            state: self.state.clone(),
            inner,
            _extractor: PhantomData,
        }
    }
}
impl<F, S, T> fmt::Debug for MapRequestLayer<F, S, T>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapRequestLayer")
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("state", &self.state)
            .finish()
    }
}
pub struct MapRequest<F, S, I, T> {
    f: F,
    inner: I,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}
impl<F, S, I, T> Clone for MapRequest<F, S, I, T>
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
macro_rules! impl_service { ([$($ty : ident : $var : ident) ,*] , $last : ident : $last_var : ident) => { impl < F , Fut , S , I , B , $($ty ,) * $last > Service < Request < B >> for MapRequest < F , S , I , ($($ty ,) * $last ,) > where F : FnMut ($($ty ,) * $last) -> Fut + Clone + Send + 'static , $($ty : FromRequestParts < S > + Send ,) * $last : FromRequest < S > + Send , Fut : Future + Send + 'static , Fut::Output : IntoMapRequestResult < B > + Send + 'static , I : Service < Request < B >, Error = Infallible > + Clone + Send + 'static , I::Response : IntoResponse , I::Future : Send + 'static , B : HttpBody < Data = Bytes > + Send + 'static , B::Error : Into < BoxError >, S : Clone + Send + Sync + 'static , { type Response = Response ; type Error = Infallible ; type Future = ResponseFuture ; fn poll_ready (& mut self , cx : & mut Context <'_ >) -> Poll < Result < () , Self::Error >> { self . inner . poll_ready (cx) } fn call (& mut self , req : Request < B >) -> Self::Future { let req = req . map (Body::new) ; let not_ready_inner = self . inner . clone () ; let mut ready_inner = std::mem::replace (& mut self . inner , not_ready_inner) ; let mut f = self . f . clone () ; let state = self . state . clone () ; let future = Box::pin (async move { let (mut parts , body) = req . into_parts () ; $(let $var = match $ty::from_request_parts (& mut parts , & state) . await { Ok (value) => value , Err (rejection) => return rejection . into_response () , } ;) * let req = Request::from_parts (parts , body) ; let $last_var = match $last::from_request (req , & state) . await { Ok (value) => value , Err (rejection) => return rejection . into_response () , } ; match f ($($var ,) * $last_var) . await . into_map_request_result () { Ok (req) => { ready_inner . call (req) . await . into_response () } Err (res) => { res } } }) ; ResponseFuture { inner : future } } } } ; }
for_http_tuples!(last, impl_service);
impl<F, S, I, T> fmt::Debug for MapRequest<F, S, I, T>
where
    S: fmt::Debug,
    I: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapRequest")
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("inner", &self.inner)
            .field("state", &self.state)
            .finish()
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
mod private {
    use crate::axum::{http::Request, response::IntoResponse};
    pub trait Sealed<B> {}
    impl<B, E> Sealed<B> for Result<Request<B>, E> where E: IntoResponse {}
    impl<B> Sealed<B> for Request<B> {}
}
pub trait IntoMapRequestResult<B>: private::Sealed<B> {
    fn into_map_request_result(self) -> Result<Request<B>, Response>;
}
impl<B, E> IntoMapRequestResult<B> for Result<Request<B>, E>
where
    E: IntoResponse,
{
    fn into_map_request_result(self) -> Result<Request<B>, Response> {
        self.map_err(IntoResponse::into_response)
    }
}
impl<B> IntoMapRequestResult<B> for Request<B> {
    fn into_map_request_result(self) -> Result<Request<B>, Response> {
        Ok(self)
    }
}
