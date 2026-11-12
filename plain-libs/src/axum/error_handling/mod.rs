use crate::axum::{
    extract::FromRequestParts,
    http::Request,
    response::{IntoResponse, Response},
};
use crate::tower::ServiceExt;
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    task::{Context, Poll},
};
use crate::tower::layer::Layer;
use crate::tower::Service;
pub struct HandleErrorLayer<F, T> {
    f: F,
    _extractor: PhantomData<fn() -> T>,
}
impl<F, T> HandleErrorLayer<F, T> {
    pub fn new(f: F) -> Self {
        Self {
            f,
            _extractor: PhantomData,
        }
    }
}
impl<F, T> Clone for HandleErrorLayer<F, T>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            _extractor: PhantomData,
        }
    }
}
impl<F, E> fmt::Debug for HandleErrorLayer<F, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandleErrorLayer")
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}
impl<S, F, T> Layer<S> for HandleErrorLayer<F, T>
where
    F: Clone,
{
    type Service = HandleError<S, F, T>;
    fn layer(&self, inner: S) -> Self::Service {
        HandleError::new(inner, self.f.clone())
    }
}
pub struct HandleError<S, F, T> {
    inner: S,
    f: F,
    _extractor: PhantomData<fn() -> T>,
}
impl<S, F, T> HandleError<S, F, T> {
    pub fn new(inner: S, f: F) -> Self {
        Self {
            inner,
            f,
            _extractor: PhantomData,
        }
    }
}
impl<S, F, T> Clone for HandleError<S, F, T>
where
    S: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            f: self.f.clone(),
            _extractor: PhantomData,
        }
    }
}
impl<S, F, E> fmt::Debug for HandleError<S, F, E>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandleError")
            .field("inner", &self.inner)
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}
impl<S, F, B, Fut, Res> Service<Request<B>> for HandleError<S, F, ()>
where
    S: Service<Request<B>> + Clone + Send + 'static,
    S::Response: IntoResponse + Send,
    S::Error: Send,
    S::Future: Send,
    F: FnOnce(S::Error) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = future::HandleErrorFuture;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let f = self.f.clone();
        let clone = self.inner.clone();
        let inner = std::mem::replace(&mut self.inner, clone);
        let future = Box::pin(async move {
            match inner.oneshot(req).await {
                Ok(res) => Ok(res.into_response()),
                Err(err) => Ok(f(err).await.into_response()),
            }
        });
        future::HandleErrorFuture { future }
    }
}
macro_rules! impl_service { ($($ty : ident : $var : ident) ,* $(,) ?) => { impl < S , F , B , Res , Fut , $($ty ,) *> Service < Request < B >> for HandleError < S , F , ($($ty ,) *) > where S : Service < Request < B >> + Clone + Send + 'static , S::Response : IntoResponse + Send , S::Error : Send , S::Future : Send , F : FnOnce ($($ty) ,*, S::Error) -> Fut + Clone + Send + 'static , Fut : Future < Output = Res > + Send , Res : IntoResponse , $($ty : FromRequestParts < () > + Send ,) * B : Send + 'static , { type Response = Response ; type Error = Infallible ; type Future = future::HandleErrorFuture ; fn poll_ready (& mut self , _ : & mut Context <'_ >) -> Poll < Result < () , Self::Error >> { Poll::Ready (Ok (())) } fn call (& mut self , req : Request < B >) -> Self::Future { let f = self . f . clone () ; let clone = self . inner . clone () ; let inner = std::mem::replace (& mut self . inner , clone) ; let future = Box::pin (async move { let (mut parts , body) = req . into_parts () ; $(let $var = match $ty::from_request_parts (& mut parts , & ()) . await { Ok (value) => value , Err (rejection) => return Ok (rejection . into_response ()) , } ;) * let req = Request::from_parts (parts , body) ; match inner . oneshot (req) . await { Ok (res) => Ok (res . into_response ()) , Err (err) => Ok (f ($($var) ,*, err) . await . into_response ()) , } }) ; future::HandleErrorFuture { future } } } } }
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
pub mod future {
    use crate::axum::response::Response;
    use pin_project_lite::pin_project;
    use std::{
        convert::Infallible,
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };
    pin_project! { pub struct HandleErrorFuture { #[pin] pub (super) future : Pin < Box < dyn Future < Output = Result < Response , Infallible >> + Send + 'static >>, } }
    impl Future for HandleErrorFuture {
        type Output = Result<Response, Infallible>;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.project().future.poll(cx)
        }
    }
}
