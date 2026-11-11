use crate::axum::response::{IntoResponse, Response};
use crate::axum_core::extract::FromRequestParts;
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
pub fn map_response<F, T>(f: F) -> MapResponseLayer<F, (), T> {
    map_response_with_state((), f)
}
pub fn map_response_with_state<F, S, T>(state: S, f: F) -> MapResponseLayer<F, S, T> {
    MapResponseLayer {
        f,
        state,
        _extractor: PhantomData,
    }
}
#[must_use]
pub struct MapResponseLayer<F, S, T> {
    f: F,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}
impl<F, S, T> Clone for MapResponseLayer<F, S, T>
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
impl<S, I, F, T> Layer<I> for MapResponseLayer<F, S, T>
where
    F: Clone,
    S: Clone,
{
    type Service = MapResponse<F, S, I, T>;
    fn layer(&self, inner: I) -> Self::Service {
        MapResponse {
            f: self.f.clone(),
            state: self.state.clone(),
            inner,
            _extractor: PhantomData,
        }
    }
}
impl<F, S, T> fmt::Debug for MapResponseLayer<F, S, T>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapResponseLayer")
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("state", &self.state)
            .finish()
    }
}
pub struct MapResponse<F, S, I, T> {
    f: F,
    inner: I,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}
impl<F, S, I, T> Clone for MapResponse<F, S, I, T>
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
macro_rules! impl_service { ($($ty : ident : $var : ident) ,*) => { impl < F , Fut , S , I , B , ResBody , $($ty ,) *> Service < Request < B >> for MapResponse < F , S , I , ($($ty ,) *) > where F : FnMut ($($ty ,) * Response < ResBody >) -> Fut + Clone + Send + 'static , $($ty : FromRequestParts < S > + Send ,) * Fut : Future + Send + 'static , Fut::Output : IntoResponse + Send + 'static , I : Service < Request < B >, Response = Response < ResBody >, Error = Infallible > + Clone + Send + 'static , I::Future : Send + 'static , B : Send + 'static , ResBody : Send + 'static , S : Clone + Send + Sync + 'static , { type Response = Response ; type Error = Infallible ; type Future = ResponseFuture ; fn poll_ready (& mut self , cx : & mut Context <'_ >) -> Poll < Result < () , Self::Error >> { self . inner . poll_ready (cx) } fn call (& mut self , req : Request < B >) -> Self::Future { let not_ready_inner = self . inner . clone () ; let mut ready_inner = std::mem::replace (& mut self . inner , not_ready_inner) ; let mut f = self . f . clone () ; let _state = self . state . clone () ; let future = Box::pin (async move { let (mut parts , body) = req . into_parts () ; $(let $var = match $ty::from_request_parts (& mut parts , & _state) . await { Ok (value) => value , Err (rejection) => return rejection . into_response () , } ;) * let req = Request::from_parts (parts , body) ; match ready_inner . call (req) . await { Ok (res) => { f ($($var ,) * res) . await . into_response () } Err (err) => match err { } } }) ; ResponseFuture { inner : future } } } } ; }
impl_service!();
impl_service!(T1: t1);
impl_service!(T1: t1, T2: t2);
impl_service!(T1: t1, T2: t2, T3: t3);
impl_service!(T1: t1, T2: t2, T3: t3, T4: t4);
impl_service!(T1: t1, T2: t2, T3: t3, T4: t4, T5: t5);
impl_service!(T1: t1, T2: t2, T3: t3, T4: t4, T5: t5, T6: t6);
impl_service!(T1: t1, T2: t2, T3: t3, T4: t4, T5: t5, T6: t6, T7: t7);
impl_service!(T1: t1, T2: t2, T3: t3, T4: t4, T5: t5, T6: t6, T7: t7, T8: t8);
impl_service!(T1: t1, T2: t2, T3: t3, T4: t4, T5: t5, T6: t6, T7: t7, T8: t8, T9: t9);
impl_service!(T1: t1, T2: t2, T3: t3, T4: t4, T5: t5, T6: t6, T7: t7, T8: t8, T9: t9, T10: t10);
impl_service!(T1: t1, T2: t2, T3: t3, T4: t4, T5: t5, T6: t6, T7: t7, T8: t8, T9: t9, T10: t10, T11: t11);
impl_service!(T1: t1, T2: t2, T3: t3, T4: t4, T5: t5, T6: t6, T7: t7, T8: t8, T9: t9, T10: t10, T11: t11, T12: t12);
impl_service!(T1: t1, T2: t2, T3: t3, T4: t4, T5: t5, T6: t6, T7: t7, T8: t8, T9: t9, T10: t10, T11: t11, T12: t12, T13: t13);
impl_service!(T1: t1, T2: t2, T3: t3, T4: t4, T5: t5, T6: t6, T7: t7, T8: t8, T9: t9, T10: t10, T11: t11, T12: t12, T13: t13, T14: t14);
impl_service!(
    T1: t1, T2: t2, T3: t3, T4: t4, T5: t5, T6: t6, T7: t7, T8: t8, T9: t9, T10: t10, T11: t11, T12: t12, T13: t13, T14: t14, T15: t15
);
impl_service!(
    T1: t1, T2: t2, T3: t3, T4: t4, T5: t5, T6: t6, T7: t7, T8: t8, T9: t9, T10: t10, T11: t11, T12: t12, T13: t13, T14: t14, T15: t15, T16: t16
);
impl<F, S, I, T> fmt::Debug for MapResponse<F, S, I, T>
where
    S: fmt::Debug,
    I: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapResponse")
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
