use self::{future::RouteFuture, not_found::NotFound, path_router::PathRouter};
use crate::axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use crate::axum::{
    body::{Body, HttpBody},
    boxed::BoxedIntoRoute,
    handler::Handler,
    util::try_downcast,
};
use crate::axum_core::{
    extract::Request,
    response::{IntoResponse, Response},
};
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    sync::Arc,
    task::{Context, Poll},
};
use crate::tower::layer::Layer;
use crate::tower::Service;
pub mod future;
mod into_make_service;
mod method_filter;
pub mod method_routing;
mod not_found;
pub(crate) mod path_router;
mod route;
mod strip_prefix;
pub(crate) mod url_params;
pub use self::method_routing::{
    MethodRouter, any, any_service, connect, connect_service, delete, delete_service, get,
    get_service, head, head_service, on, on_service, options, options_service, patch,
    patch_service, post, post_service, put, put_service, trace, trace_service,
};
pub use self::{into_make_service::IntoMakeService, method_filter::MethodFilter, route::Route};
macro_rules! panic_on_err {
    ($expr : expr) => {
        match $expr {
            Ok(x) => x,
            Err(err) => panic!("{err}"),
        }
    };
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RouteId(u32);
#[must_use]
pub struct Router<S = ()> {
    inner: Arc<RouterInner<S>>,
}
impl<S> Clone for Router<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
struct RouterInner<S> {
    path_router: PathRouter<S, false>,
    fallback_router: PathRouter<S, true>,
    default_fallback: bool,
    catch_all_fallback: Fallback<S>,
}
impl<S> Default for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
impl<S> fmt::Debug for Router<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("path_router", &self.inner.path_router)
            .field("fallback_router", &self.inner.fallback_router)
            .field("default_fallback", &self.inner.default_fallback)
            .field("catch_all_fallback", &self.inner.catch_all_fallback)
            .finish()
    }
}
pub(crate) const NEST_TAIL_PARAM: &str = "__private__axum_nest_tail_param";
pub(crate) const NEST_TAIL_PARAM_CAPTURE: &str = "/*__private__axum_nest_tail_param";
pub(crate) const FALLBACK_PARAM: &str = "__private__axum_fallback";
pub(crate) const FALLBACK_PARAM_PATH: &str = "/*__private__axum_fallback";
macro_rules! map_inner {
    ($self_ : ident , $inner : pat_param => $expr : expr) => {{
        let $inner = $self_.into_inner();
        Router {
            inner: Arc::new($expr),
        }
    }};
}
macro_rules! tap_inner { ($self_ : ident , mut $inner : ident => { $($stmt : stmt) * }) => { { let mut $inner = $self_ . into_inner () ; $($stmt) * Router { inner : Arc::new ($inner) , } } } ; }
impl<S> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RouterInner {
                path_router: Default::default(),
                fallback_router: PathRouter::new_fallback(),
                default_fallback: true,
                catch_all_fallback: Fallback::Default(Route::new(NotFound)),
            }),
        }
    }
    fn into_inner(self) -> RouterInner<S> {
        match Arc::try_unwrap(self.inner) {
            Ok(inner) => inner,
            Err(arc) => RouterInner {
                path_router: arc.path_router.clone(),
                fallback_router: arc.fallback_router.clone(),
                default_fallback: arc.default_fallback,
                catch_all_fallback: arc.catch_all_fallback.clone(),
            },
        }
    }
    #[track_caller]
    pub fn route(self, path: &str, method_router: MethodRouter<S>) -> Self {
        tap_inner ! (self , mut this => { panic_on_err ! (this . path_router . route (path , method_router)) ; })
    }
    pub fn route_service<T>(self, path: &str, service: T) -> Self
    where
        T: Service<Request, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        let service = match try_downcast::<Router<S>, _>(service) {
            Ok(_) => {
                panic!(
                    "Invalid route: `Router::route_service` cannot be used with `Router`s. \
                     Use `Router::nest` instead"
                );
            }
            Err(service) => service,
        };
        tap_inner ! (self , mut this => { panic_on_err ! (this . path_router . route_service (path , service)) ; })
    }
    #[track_caller]
    pub fn nest(self, path: &str, router: Router<S>) -> Self {
        let RouterInner {
            path_router,
            fallback_router,
            default_fallback,
            catch_all_fallback: _,
        } = router.into_inner();
        tap_inner ! (self , mut this => { panic_on_err ! (this . path_router . nest (path , path_router)) ; if ! default_fallback { panic_on_err ! (this . fallback_router . nest (path , fallback_router)) ; } })
    }
    #[track_caller]
    pub fn nest_service<T>(self, path: &str, service: T) -> Self
    where
        T: Service<Request, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        tap_inner ! (self , mut this => { panic_on_err ! (this . path_router . nest_service (path , service)) ; })
    }
    #[track_caller]
    pub fn merge<R>(self, other: R) -> Self
    where
        R: Into<Router<S>>,
    {
        const PANIC_MSG: &str =
            "Failed to merge fallbacks. This is a bug in axum. Please file an issue";
        let other: Router<S> = other.into();
        let RouterInner {
            path_router,
            fallback_router: mut other_fallback,
            default_fallback,
            catch_all_fallback,
        } = other.into_inner();
        map_inner ! (self , mut this => { panic_on_err ! (this . path_router . merge (path_router)) ; match (this . default_fallback , default_fallback) { (true , true) => { this . fallback_router . merge (other_fallback) . expect (PANIC_MSG) ; } (true , false) => { this . fallback_router . merge (other_fallback) . expect (PANIC_MSG) ; this . default_fallback = false ; } (false , true) => { let fallback_router = std::mem::take (& mut this . fallback_router) ; other_fallback . merge (fallback_router) . expect (PANIC_MSG) ; this . fallback_router = other_fallback ; } (false , false) => { panic ! ("Cannot merge two `Router`s that both have a fallback") } } ; this . catch_all_fallback = this . catch_all_fallback . merge (catch_all_fallback) . unwrap_or_else (|| panic ! ("Cannot merge two `Router`s that both have a fallback")) ; this })
    }
    pub fn layer<L>(self, layer: L) -> Router<S>
    where
        L: Layer<Route> + Clone + Send + 'static,
        L::Service: Service<Request> + Clone + Send + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        map_inner ! (self , this => RouterInner { path_router : this . path_router . layer (layer . clone ()) , fallback_router : this . fallback_router . layer (layer . clone ()) , default_fallback : this . default_fallback , catch_all_fallback : this . catch_all_fallback . map (| route | route . layer (layer)) , })
    }
    #[track_caller]
    pub fn route_layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + Send + 'static,
        L::Service: Service<Request> + Clone + Send + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        map_inner ! (self , this => RouterInner { path_router : this . path_router . route_layer (layer) , fallback_router : this . fallback_router , default_fallback : this . default_fallback , catch_all_fallback : this . catch_all_fallback , })
    }
    pub fn has_routes(&self) -> bool {
        self.inner.path_router.has_routes()
    }
    #[track_caller]
    pub fn fallback<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        tap_inner ! (self , mut this => { this . catch_all_fallback = Fallback::BoxedHandler (BoxedIntoRoute::from_handler (handler . clone ())) ; }) . fallback_endpoint (Endpoint::MethodRouter (any (handler)))
    }
    pub fn fallback_service<T>(self, service: T) -> Self
    where
        T: Service<Request, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        let route = Route::new(service);
        tap_inner ! (self , mut this => { this . catch_all_fallback = Fallback::Service (route . clone ()) ; }) . fallback_endpoint (Endpoint::Route (route))
    }
    pub fn method_not_allowed_fallback<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        tap_inner ! (self , mut this => { this . path_router . method_not_allowed_fallback (handler . clone ()) })
    }
    fn fallback_endpoint(self, endpoint: Endpoint<S>) -> Self {
        tap_inner ! (self , mut this => { this . fallback_router . set_fallback (endpoint) ; this . default_fallback = false ; })
    }
    pub fn with_state<S2>(self, state: S) -> Router<S2> {
        map_inner ! (self , this => RouterInner { path_router : this . path_router . with_state (state . clone ()) , fallback_router : this . fallback_router . with_state (state . clone ()) , default_fallback : this . default_fallback , catch_all_fallback : this . catch_all_fallback . with_state (state) , })
    }
    pub(crate) fn call_with_state(&self, req: Request, state: S) -> RouteFuture<Infallible> {
        let (req, state) = match self.inner.path_router.call_with_state(req, state) {
            Ok(future) => return future,
            Err((req, state)) => (req, state),
        };
        let (req, state) = match self.inner.fallback_router.call_with_state(req, state) {
            Ok(future) => return future,
            Err((req, state)) => (req, state),
        };
        self.inner
            .catch_all_fallback
            .clone()
            .call_with_state(req, state)
    }

    pub fn into_service<B>(self) -> RouterIntoService<B, S> {
        RouterIntoService {
            router: self,
            _marker: PhantomData,
        }
    }
}
impl Router {
    pub fn into_make_service(self) -> IntoMakeService<Self> {
        IntoMakeService::new(self.with_state(()))
    }
    pub fn into_make_service_with_connect_info<C>(self) -> IntoMakeServiceWithConnectInfo<Self, C> {
        IntoMakeServiceWithConnectInfo::new(self.with_state(()))
    }
}
const _: () = {
    use crate::axum::serve::IncomingStream;
    impl Service<IncomingStream<'_>> for Router<()> {
        type Response = Self;
        type Error = Infallible;
        type Future = std::future::Ready<Result<Self::Response, Self::Error>>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, _req: IncomingStream<'_>) -> Self::Future {
            std::future::ready(Ok(self.clone().with_state(())))
        }
    }
};
impl<B> Service<Request<B>> for Router<()>
where
    B: HttpBody<Data = bytes::Bytes> + Send + 'static,
    B::Error: Into<crate::axum_core::BoxError>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = RouteFuture<Infallible>;
    #[inline]
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let req = req.map(Body::new);
        self.call_with_state(req, ())
    }
}
pub struct RouterAsService<'a, B, S = ()> {
    router: &'a mut Router<S>,
    _marker: PhantomData<B>,
}
impl<B> Service<Request<B>> for RouterAsService<'_, B, ()>
where
    B: HttpBody<Data = bytes::Bytes> + Send + 'static,
    B::Error: Into<crate::axum_core::BoxError>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = RouteFuture<Infallible>;
    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Router as Service<Request<B>>>::poll_ready(self.router, cx)
    }
    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        self.router.call(req)
    }
}
impl<B, S> fmt::Debug for RouterAsService<'_, B, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouterAsService")
            .field("router", &self.router)
            .finish()
    }
}
pub struct RouterIntoService<B, S = ()> {
    router: Router<S>,
    _marker: PhantomData<B>,
}
impl<B, S> Clone for RouterIntoService<B, S>
where
    Router<S>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
            _marker: PhantomData,
        }
    }
}
impl<B> Service<Request<B>> for RouterIntoService<B, ()>
where
    B: HttpBody<Data = bytes::Bytes> + Send + 'static,
    B::Error: Into<crate::axum_core::BoxError>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = RouteFuture<Infallible>;
    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Router as Service<Request<B>>>::poll_ready(&mut self.router, cx)
    }
    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        self.router.call(req)
    }
}
impl<B, S> fmt::Debug for RouterIntoService<B, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouterIntoService")
            .field("router", &self.router)
            .finish()
    }
}
enum Fallback<S, E = Infallible> {
    Default(Route<E>),
    Service(Route<E>),
    BoxedHandler(BoxedIntoRoute<S, E>),
}
impl<S, E> Fallback<S, E>
where
    S: Clone,
{
    fn merge(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Default(_), pick @ Self::Default(_)) => Some(pick),
            (Self::Default(_), pick) | (pick, Self::Default(_)) => Some(pick),
            _ => None,
        }
    }
    fn map<F, E2>(self, f: F) -> Fallback<S, E2>
    where
        S: 'static,
        E: 'static,
        F: FnOnce(Route<E>) -> Route<E2> + Clone + Send + 'static,
        E2: 'static,
    {
        match self {
            Self::Default(route) => Fallback::Default(f(route)),
            Self::Service(route) => Fallback::Service(f(route)),
            Self::BoxedHandler(handler) => Fallback::BoxedHandler(handler.map(f)),
        }
    }
    fn with_state<S2>(self, state: S) -> Fallback<S2, E> {
        match self {
            Fallback::Default(route) => Fallback::Default(route),
            Fallback::Service(route) => Fallback::Service(route),
            Fallback::BoxedHandler(handler) => Fallback::Service(handler.into_route(state)),
        }
    }
    fn call_with_state(self, req: Request, state: S) -> RouteFuture<E> {
        match self {
            Fallback::Default(route) | Fallback::Service(route) => {
                RouteFuture::from_future(route.oneshot_inner_owned(req))
            }
            Fallback::BoxedHandler(handler) => {
                let route = handler.clone().into_route(state);
                RouteFuture::from_future(route.oneshot_inner_owned(req))
            }
        }
    }
}
impl<S, E> Clone for Fallback<S, E> {
    fn clone(&self) -> Self {
        match self {
            Self::Default(inner) => Self::Default(inner.clone()),
            Self::Service(inner) => Self::Service(inner.clone()),
            Self::BoxedHandler(inner) => Self::BoxedHandler(inner.clone()),
        }
    }
}
impl<S, E> fmt::Debug for Fallback<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default(inner) => f.debug_tuple("Default").field(inner).finish(),
            Self::Service(inner) => f.debug_tuple("Service").field(inner).finish(),
            Self::BoxedHandler(_) => f.debug_tuple("BoxedHandler").finish(),
        }
    }
}
enum Endpoint<S> {
    MethodRouter(MethodRouter<S>),
    Route(Route),
}
impl<S> Endpoint<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn layer<L>(self, layer: L) -> Endpoint<S>
    where
        L: Layer<Route> + Clone + Send + 'static,
        L::Service: Service<Request> + Clone + Send + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        match self {
            Endpoint::MethodRouter(method_router) => {
                Endpoint::MethodRouter(method_router.layer(layer))
            }
            Endpoint::Route(route) => Endpoint::Route(route.layer(layer)),
        }
    }
}
impl<S> Clone for Endpoint<S> {
    fn clone(&self) -> Self {
        match self {
            Self::MethodRouter(inner) => Self::MethodRouter(inner.clone()),
            Self::Route(inner) => Self::Route(inner.clone()),
        }
    }
}
impl<S> fmt::Debug for Endpoint<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MethodRouter(method_router) => {
                f.debug_tuple("MethodRouter").field(method_router).finish()
            }
            Self::Route(route) => f.debug_tuple("Route").field(route).finish(),
        }
    }
}
