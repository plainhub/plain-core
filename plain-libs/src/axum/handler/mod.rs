use crate::axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use crate::axum::{
    extract::{FromRequest, FromRequestParts, Request},
    response::{IntoResponse, Response},
    routing::IntoMakeService,
};
use crate::tower::ServiceExt;
use std::{convert::Infallible, fmt, future::Future, marker::PhantomData, pin::Pin};
use crate::tower::layer::Layer;
use crate::tower::Service;
pub mod future;
mod service;
pub use self::service::HandlerService;

pub trait Handler<T, S>: Clone + Send + Sized + 'static {
    type Future: Future<Output = Response> + Send + 'static;
    fn call(self, req: Request, state: S) -> Self::Future;
    fn layer<L>(self, layer: L) -> Layered<L, Self, T, S>
    where
        L: Layer<HandlerService<Self, T, S>> + Clone,
        L::Service: Service<Request>,
    {
        Layered {
            layer,
            handler: self,
            _marker: PhantomData,
        }
    }
    fn with_state(self, state: S) -> HandlerService<Self, T, S> {
        HandlerService::new(self, state)
    }
}
impl<F, Fut, Res, S> Handler<((),), S> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;
    fn call(self, _req: Request, _state: S) -> Self::Future {
        Box::pin(async move { self().await.into_response() })
    }
}
macro_rules! impl_handler { ([$($ty : ident : $var : ident) ,*] , $last : ident : $last_var : ident) => { impl < F , Fut , S , Res , M , $($ty ,) * $last > Handler < (M , $($ty ,) * $last ,) , S > for F where F : FnOnce ($($ty ,) * $last ,) -> Fut + Clone + Send + 'static , Fut : Future < Output = Res > + Send , S : Send + Sync + 'static , Res : IntoResponse , $($ty : FromRequestParts < S > + Send ,) * $last : FromRequest < S , M > + Send , { type Future = Pin < Box < dyn Future < Output = Response > + Send >>; fn call (self , req : Request , state : S) -> Self::Future { Box::pin (async move { let (mut parts , body) = req . into_parts () ; let state = & state ; $(let $var = match $ty::from_request_parts (& mut parts , state) . await { Ok (value) => value , Err (rejection) => return rejection . into_response () , } ;) * let req = Request::from_parts (parts , body) ; let $last_var = match $last::from_request (req , state) . await { Ok (value) => value , Err (rejection) => return rejection . into_response () , } ; let res = self ($($var ,) * $last_var ,) . await ; res . into_response () }) } } } ; }
for_http_tuples!(last, impl_handler);
mod private {
    pub enum IntoResponseHandler {}
}
impl<T, S> Handler<private::IntoResponseHandler, S> for T
where
    T: IntoResponse + Clone + Send + 'static,
{
    type Future = std::future::Ready<Response>;
    fn call(self, _req: Request, _state: S) -> Self::Future {
        std::future::ready(self.into_response())
    }
}
pub struct Layered<L, H, T, S> {
    layer: L,
    handler: H,
    _marker: PhantomData<fn() -> (T, S)>,
}
impl<L, H, T, S> fmt::Debug for Layered<L, H, T, S>
where
    L: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Layered")
            .field("layer", &self.layer)
            .finish()
    }
}
impl<L, H, T, S> Clone for Layered<L, H, T, S>
where
    L: Clone,
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            layer: self.layer.clone(),
            handler: self.handler.clone(),
            _marker: PhantomData,
        }
    }
}
impl<H, S, T, L> Handler<T, S> for Layered<L, H, T, S>
where
    L: Layer<HandlerService<H, T, S>> + Clone + Send + 'static,
    H: Handler<T, S>,
    L::Service: Service<Request, Error = Infallible> + Clone + Send + 'static,
    <L::Service as Service<Request>>::Response: IntoResponse,
    <L::Service as Service<Request>>::Future: Send,
    T: 'static,
    S: 'static,
{
    type Future = future::LayeredFuture<L::Service>;
    fn call(self, req: Request, state: S) -> Self::Future {
        use futures_util::future::{FutureExt, Map};
        let svc = self.handler.with_state(state);
        let svc = self.layer.layer(svc);
        let future: Map<
            _,
            fn(
                Result<
                    <L::Service as Service<Request>>::Response,
                    <L::Service as Service<Request>>::Error,
                >,
            ) -> _,
        > = svc.oneshot(req).map(|result| match result {
            Ok(res) => res.into_response(),
            Err(err) => match err {},
        });
        future::LayeredFuture::new(future)
    }
}
pub trait HandlerWithoutStateExt<T>: Handler<T, ()> {
    fn into_service(self) -> HandlerService<Self, T, ()>;
    fn into_make_service(self) -> IntoMakeService<HandlerService<Self, T, ()>>;
    fn into_make_service_with_connect_info<C>(
        self,
    ) -> IntoMakeServiceWithConnectInfo<HandlerService<Self, T, ()>, C>;
}
impl<H, T> HandlerWithoutStateExt<T> for H
where
    H: Handler<T, ()>,
{
    fn into_service(self) -> HandlerService<Self, T, ()> {
        self.with_state(())
    }
    fn into_make_service(self) -> IntoMakeService<HandlerService<Self, T, ()>> {
        self.into_service().into_make_service()
    }
    fn into_make_service_with_connect_info<C>(
        self,
    ) -> IntoMakeServiceWithConnectInfo<HandlerService<Self, T, ()>, C> {
        self.into_service().into_make_service_with_connect_info()
    }
}
