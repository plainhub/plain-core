use super::{IntoMakeService, future::InfallibleRouteFuture};
use crate::axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use crate::axum::{
    body::{Body, Bytes, HttpBody},
    boxed::BoxedIntoRoute,
    error_handling::{HandleError, HandleErrorLayer},
    handler::Handler,
    http::{Method, StatusCode},
    response::Response,
    routing::{Fallback, MethodFilter, Route, future::RouteFuture},
};
use crate::axum_core::{BoxError, extract::Request, response::IntoResponse};
use crate::tower::{service_fn, util::MapResponseLayer};
use bytes::BytesMut;
use std::{
    convert::Infallible,
    fmt,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;
macro_rules! top_level_service_fn {
    ($name : ident , $method : ident) => {
        pub fn $name<T, S>(svc: T) -> MethodRouter<S, T::Error>
        where
            T: Service<Request> + Clone + Send + 'static,
            T::Response: IntoResponse + 'static,
            T::Future: Send + 'static,
            S: Clone,
        {
            on_service(MethodFilter::$method, svc)
        }
    };
}
macro_rules! top_level_handler_fn {
    ($name : ident , $method : ident) => {
        pub fn $name<H, T, S>(handler: H) -> MethodRouter<S, Infallible>
        where
            H: Handler<T, S>,
            T: 'static,
            S: Clone + Send + Sync + 'static,
        {
            on(MethodFilter::$method, handler)
        }
    };
}
macro_rules! chained_service_fn {
    ($name : ident , $method : ident) => {
        #[track_caller]
        pub fn $name<T>(self, svc: T) -> Self
        where
            T: Service<Request, Error = E> + Clone + Send + 'static,
            T::Response: IntoResponse + 'static,
            T::Future: Send + 'static,
        {
            self.on_service(MethodFilter::$method, svc)
        }
    };
}
macro_rules! chained_handler_fn {
    ($name : ident , $method : ident) => {
        #[track_caller]
        pub fn $name<H, T>(self, handler: H) -> Self
        where
            H: Handler<T, S>,
            T: 'static,
            S: Send + Sync + 'static,
        {
            self.on(MethodFilter::$method, handler)
        }
    };
}
top_level_service_fn!(connect_service, CONNECT);
top_level_service_fn!(delete_service, DELETE);
top_level_service_fn!(get_service, GET);
top_level_service_fn!(head_service, HEAD);
top_level_service_fn!(options_service, OPTIONS);
top_level_service_fn!(patch_service, PATCH);
top_level_service_fn!(post_service, POST);
top_level_service_fn!(put_service, PUT);
top_level_service_fn!(trace_service, TRACE);
pub fn on_service<T, S>(filter: MethodFilter, svc: T) -> MethodRouter<S, T::Error>
where
    T: Service<Request> + Clone + Send + 'static,
    T::Response: IntoResponse + 'static,
    T::Future: Send + 'static,
    S: Clone,
{
    MethodRouter::new().on_service(filter, svc)
}
pub fn any_service<T, S>(svc: T) -> MethodRouter<S, T::Error>
where
    T: Service<Request> + Clone + Send + 'static,
    T::Response: IntoResponse + 'static,
    T::Future: Send + 'static,
    S: Clone,
{
    MethodRouter::new()
        .fallback_service(svc)
        .skip_allow_header()
}
top_level_handler_fn!(connect, CONNECT);
top_level_handler_fn!(delete, DELETE);
top_level_handler_fn!(get, GET);
top_level_handler_fn!(head, HEAD);
top_level_handler_fn!(options, OPTIONS);
top_level_handler_fn!(patch, PATCH);
top_level_handler_fn!(post, POST);
top_level_handler_fn!(put, PUT);
top_level_handler_fn!(trace, TRACE);
pub fn on<H, T, S>(filter: MethodFilter, handler: H) -> MethodRouter<S, Infallible>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().on(filter, handler)
}
pub fn any<H, T, S>(handler: H) -> MethodRouter<S, Infallible>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().fallback(handler).skip_allow_header()
}
#[must_use]
pub struct MethodRouter<S = (), E = Infallible> {
    get: MethodEndpoint<S, E>,
    head: MethodEndpoint<S, E>,
    delete: MethodEndpoint<S, E>,
    options: MethodEndpoint<S, E>,
    patch: MethodEndpoint<S, E>,
    post: MethodEndpoint<S, E>,
    put: MethodEndpoint<S, E>,
    trace: MethodEndpoint<S, E>,
    connect: MethodEndpoint<S, E>,
    fallback: Fallback<S, E>,
    allow_header: AllowHeader,
}
#[derive(Clone, Debug)]
enum AllowHeader {
    None,
    Skip,
    Bytes(BytesMut),
}
impl AllowHeader {
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (AllowHeader::Skip, _) | (_, AllowHeader::Skip) => AllowHeader::Skip,
            (AllowHeader::None, AllowHeader::None) => AllowHeader::None,
            (AllowHeader::None, AllowHeader::Bytes(pick)) => AllowHeader::Bytes(pick),
            (AllowHeader::Bytes(pick), AllowHeader::None) => AllowHeader::Bytes(pick),
            (AllowHeader::Bytes(mut a), AllowHeader::Bytes(b)) => {
                a.extend_from_slice(b",");
                a.extend_from_slice(&b);
                AllowHeader::Bytes(a)
            }
        }
    }
}
impl<S, E> fmt::Debug for MethodRouter<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MethodRouter")
            .field("get", &self.get)
            .field("head", &self.head)
            .field("delete", &self.delete)
            .field("options", &self.options)
            .field("patch", &self.patch)
            .field("post", &self.post)
            .field("put", &self.put)
            .field("trace", &self.trace)
            .field("connect", &self.connect)
            .field("fallback", &self.fallback)
            .field("allow_header", &self.allow_header)
            .finish()
    }
}
impl<S> MethodRouter<S, Infallible>
where
    S: Clone,
{
    #[track_caller]
    pub fn on<H, T>(self, filter: MethodFilter, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
        S: Send + Sync + 'static,
    {
        self.on_endpoint(
            filter,
            MethodEndpoint::BoxedHandler(BoxedIntoRoute::from_handler(handler)),
        )
    }
    chained_handler_fn!(connect, CONNECT);
    chained_handler_fn!(delete, DELETE);
    chained_handler_fn!(get, GET);
    chained_handler_fn!(head, HEAD);
    chained_handler_fn!(options, OPTIONS);
    chained_handler_fn!(patch, PATCH);
    chained_handler_fn!(post, POST);
    chained_handler_fn!(put, PUT);
    chained_handler_fn!(trace, TRACE);
    pub fn fallback<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
        S: Send + Sync + 'static,
    {
        self.fallback = Fallback::BoxedHandler(BoxedIntoRoute::from_handler(handler));
        self
    }
    pub(crate) fn default_fallback<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
        S: Send + Sync + 'static,
    {
        match self.fallback {
            Fallback::Default(_) => self.fallback(handler),
            _ => self,
        }
    }
}
impl MethodRouter<(), Infallible> {
    pub fn into_make_service(self) -> IntoMakeService<Self> {
        IntoMakeService::new(self.with_state(()))
    }
    pub fn into_make_service_with_connect_info<C>(self) -> IntoMakeServiceWithConnectInfo<Self, C> {
        IntoMakeServiceWithConnectInfo::new(self.with_state(()))
    }
}
impl<S, E> MethodRouter<S, E>
where
    S: Clone,
{
    pub fn new() -> Self {
        let fallback = Route::new(service_fn(|_: Request| async {
            Ok(StatusCode::METHOD_NOT_ALLOWED.into_response())
        }));
        Self {
            get: MethodEndpoint::None,
            head: MethodEndpoint::None,
            delete: MethodEndpoint::None,
            options: MethodEndpoint::None,
            patch: MethodEndpoint::None,
            post: MethodEndpoint::None,
            put: MethodEndpoint::None,
            trace: MethodEndpoint::None,
            connect: MethodEndpoint::None,
            allow_header: AllowHeader::None,
            fallback: Fallback::Default(fallback),
        }
    }
    pub fn with_state<S2>(self, state: S) -> MethodRouter<S2, E> {
        MethodRouter {
            get: self.get.with_state(&state),
            head: self.head.with_state(&state),
            delete: self.delete.with_state(&state),
            options: self.options.with_state(&state),
            patch: self.patch.with_state(&state),
            post: self.post.with_state(&state),
            put: self.put.with_state(&state),
            trace: self.trace.with_state(&state),
            connect: self.connect.with_state(&state),
            allow_header: self.allow_header,
            fallback: self.fallback.with_state(state),
        }
    }
    #[track_caller]
    pub fn on_service<T>(self, filter: MethodFilter, svc: T) -> Self
    where
        T: Service<Request, Error = E> + Clone + Send + 'static,
        T::Response: IntoResponse + 'static,
        T::Future: Send + 'static,
    {
        self.on_endpoint(filter, MethodEndpoint::Route(Route::new(svc)))
    }
    #[track_caller]
    fn on_endpoint(mut self, filter: MethodFilter, endpoint: MethodEndpoint<S, E>) -> Self {
        #[track_caller]
        fn set_endpoint<S, E>(
            method_name: &str,
            out: &mut MethodEndpoint<S, E>,
            endpoint: &MethodEndpoint<S, E>,
            endpoint_filter: MethodFilter,
            filter: MethodFilter,
            allow_header: &mut AllowHeader,
            methods: &[&'static str],
        ) where
            MethodEndpoint<S, E>: Clone,
            S: Clone,
        {
            if endpoint_filter.contains(filter) {
                if out.is_some() {
                    panic!(
                        "Overlapping method route. Cannot add two method routes that both handle \
                         `{method_name}`",
                    )
                }
                *out = endpoint.clone();
                for method in methods {
                    append_allow_header(allow_header, method);
                }
            }
        }
        set_endpoint(
            "GET",
            &mut self.get,
            &endpoint,
            filter,
            MethodFilter::GET,
            &mut self.allow_header,
            &["GET", "HEAD"],
        );
        set_endpoint(
            "HEAD",
            &mut self.head,
            &endpoint,
            filter,
            MethodFilter::HEAD,
            &mut self.allow_header,
            &["HEAD"],
        );
        set_endpoint(
            "TRACE",
            &mut self.trace,
            &endpoint,
            filter,
            MethodFilter::TRACE,
            &mut self.allow_header,
            &["TRACE"],
        );
        set_endpoint(
            "PUT",
            &mut self.put,
            &endpoint,
            filter,
            MethodFilter::PUT,
            &mut self.allow_header,
            &["PUT"],
        );
        set_endpoint(
            "POST",
            &mut self.post,
            &endpoint,
            filter,
            MethodFilter::POST,
            &mut self.allow_header,
            &["POST"],
        );
        set_endpoint(
            "PATCH",
            &mut self.patch,
            &endpoint,
            filter,
            MethodFilter::PATCH,
            &mut self.allow_header,
            &["PATCH"],
        );
        set_endpoint(
            "OPTIONS",
            &mut self.options,
            &endpoint,
            filter,
            MethodFilter::OPTIONS,
            &mut self.allow_header,
            &["OPTIONS"],
        );
        set_endpoint(
            "DELETE",
            &mut self.delete,
            &endpoint,
            filter,
            MethodFilter::DELETE,
            &mut self.allow_header,
            &["DELETE"],
        );
        set_endpoint(
            "CONNECT",
            &mut self.options,
            &endpoint,
            filter,
            MethodFilter::CONNECT,
            &mut self.allow_header,
            &["CONNECT"],
        );
        self
    }
    chained_service_fn!(connect_service, CONNECT);
    chained_service_fn!(delete_service, DELETE);
    chained_service_fn!(get_service, GET);
    chained_service_fn!(head_service, HEAD);
    chained_service_fn!(options_service, OPTIONS);
    chained_service_fn!(patch_service, PATCH);
    chained_service_fn!(post_service, POST);
    chained_service_fn!(put_service, PUT);
    chained_service_fn!(trace_service, TRACE);
    pub fn fallback_service<T>(mut self, svc: T) -> Self
    where
        T: Service<Request, Error = E> + Clone + Send + 'static,
        T::Response: IntoResponse + 'static,
        T::Future: Send + 'static,
    {
        self.fallback = Fallback::Service(Route::new(svc));
        self
    }
    pub fn layer<L, NewError>(self, layer: L) -> MethodRouter<S, NewError>
    where
        L: Layer<Route<E>> + Clone + Send + 'static,
        L::Service: Service<Request> + Clone + Send + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<NewError> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
        E: 'static,
        S: 'static,
        NewError: 'static,
    {
        let layer_fn = move |route: Route<E>| route.layer(layer.clone());
        MethodRouter {
            get: self.get.map(layer_fn.clone()),
            head: self.head.map(layer_fn.clone()),
            delete: self.delete.map(layer_fn.clone()),
            options: self.options.map(layer_fn.clone()),
            patch: self.patch.map(layer_fn.clone()),
            post: self.post.map(layer_fn.clone()),
            put: self.put.map(layer_fn.clone()),
            trace: self.trace.map(layer_fn.clone()),
            connect: self.connect.map(layer_fn.clone()),
            fallback: self.fallback.map(layer_fn),
            allow_header: self.allow_header,
        }
    }
    #[track_caller]
    pub fn route_layer<L>(mut self, layer: L) -> MethodRouter<S, E>
    where
        L: Layer<Route<E>> + Clone + Send + 'static,
        L::Service: Service<Request, Error = E> + Clone + Send + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
        E: 'static,
        S: 'static,
    {
        if self.get.is_none()
            && self.head.is_none()
            && self.delete.is_none()
            && self.options.is_none()
            && self.patch.is_none()
            && self.post.is_none()
            && self.put.is_none()
            && self.trace.is_none()
            && self.connect.is_none()
        {
            panic!(
                "Adding a route_layer before any routes is a no-op. \
                 Add the routes you want the layer to apply to first."
            );
        }
        let layer_fn = move |svc| {
            let svc = layer.layer(svc);
            let svc = MapResponseLayer::new(IntoResponse::into_response).layer(svc);
            Route::new(svc)
        };
        self.get = self.get.map(layer_fn.clone());
        self.head = self.head.map(layer_fn.clone());
        self.delete = self.delete.map(layer_fn.clone());
        self.options = self.options.map(layer_fn.clone());
        self.patch = self.patch.map(layer_fn.clone());
        self.post = self.post.map(layer_fn.clone());
        self.put = self.put.map(layer_fn.clone());
        self.trace = self.trace.map(layer_fn.clone());
        self.connect = self.connect.map(layer_fn);
        self
    }
    #[track_caller]
    pub(crate) fn merge_for_path(mut self, path: Option<&str>, other: MethodRouter<S, E>) -> Self {
        #[track_caller]
        fn merge_inner<S, E>(
            path: Option<&str>,
            name: &str,
            first: MethodEndpoint<S, E>,
            second: MethodEndpoint<S, E>,
        ) -> MethodEndpoint<S, E> {
            match (first, second) {
                (MethodEndpoint::None, MethodEndpoint::None) => MethodEndpoint::None,
                (pick, MethodEndpoint::None) | (MethodEndpoint::None, pick) => pick,
                _ => {
                    if let Some(path) = path {
                        panic!(
                            "Overlapping method route. Handler for `{name} {path}` already exists"
                        );
                    } else {
                        panic!(
                            "Overlapping method route. Cannot merge two method routes that both \
                             define `{name}`"
                        );
                    }
                }
            }
        }
        self.get = merge_inner(path, "GET", self.get, other.get);
        self.head = merge_inner(path, "HEAD", self.head, other.head);
        self.delete = merge_inner(path, "DELETE", self.delete, other.delete);
        self.options = merge_inner(path, "OPTIONS", self.options, other.options);
        self.patch = merge_inner(path, "PATCH", self.patch, other.patch);
        self.post = merge_inner(path, "POST", self.post, other.post);
        self.put = merge_inner(path, "PUT", self.put, other.put);
        self.trace = merge_inner(path, "TRACE", self.trace, other.trace);
        self.connect = merge_inner(path, "CONNECT", self.connect, other.connect);
        self.fallback = self
            .fallback
            .merge(other.fallback)
            .expect("Cannot merge two `MethodRouter`s that both have a fallback");
        self.allow_header = self.allow_header.merge(other.allow_header);
        self
    }
    #[track_caller]
    pub fn merge(self, other: MethodRouter<S, E>) -> Self {
        self.merge_for_path(None, other)
    }
    pub fn handle_error<F, T>(self, f: F) -> MethodRouter<S, Infallible>
    where
        F: Clone + Send + Sync + 'static,
        HandleError<Route<E>, F, T>: Service<Request, Error = Infallible>,
        <HandleError<Route<E>, F, T> as Service<Request>>::Future: Send,
        <HandleError<Route<E>, F, T> as Service<Request>>::Response: IntoResponse + Send,
        T: 'static,
        E: 'static,
        S: 'static,
    {
        self.layer(HandleErrorLayer::new(f))
    }
    fn skip_allow_header(mut self) -> Self {
        self.allow_header = AllowHeader::Skip;
        self
    }
    pub(crate) fn call_with_state(&self, req: Request, state: S) -> RouteFuture<E> {
        let method = req.method();
        let is_head = *method == Method::HEAD;
        macro_rules! call {
            ($method_variant : ident , $svc : expr) => {
                if *method == Method::$method_variant {
                    match $svc {
                        MethodEndpoint::None => {}
                        MethodEndpoint::Route(route) => {
                            return RouteFuture::from_future(
                                route.clone().oneshot_inner_owned(req),
                            )
                            .strip_body(is_head);
                        }
                        MethodEndpoint::BoxedHandler(handler) => {
                            let route = handler.clone().into_route(state);
                            return RouteFuture::from_future(
                                route.clone().oneshot_inner_owned(req),
                            )
                            .strip_body(is_head);
                        }
                    }
                }
            };
        }
        let Self {
            get,
            head,
            delete,
            options,
            patch,
            post,
            put,
            trace,
            connect,
            fallback,
            allow_header,
        } = self;
        call!(HEAD, head);
        call!(HEAD, get);
        call!(GET, get);
        call!(POST, post);
        call!(OPTIONS, options);
        call!(PATCH, patch);
        call!(PUT, put);
        call!(DELETE, delete);
        call!(TRACE, trace);
        call!(CONNECT, connect);
        let future = fallback.clone().call_with_state(req, state);
        match allow_header {
            AllowHeader::None => future.allow_header(Bytes::new()),
            AllowHeader::Skip => future,
            AllowHeader::Bytes(allow_header) => future.allow_header(allow_header.clone().freeze()),
        }
    }
}
fn append_allow_header(allow_header: &mut AllowHeader, method: &'static str) {
    match allow_header {
        AllowHeader::None => {
            *allow_header = AllowHeader::Bytes(BytesMut::from(method));
        }
        AllowHeader::Skip => {}
        AllowHeader::Bytes(allow_header) => {
            if let Ok(s) = std::str::from_utf8(allow_header) {
                if !s.contains(method) {
                    allow_header.extend_from_slice(b",");
                    allow_header.extend_from_slice(method.as_bytes());
                }
            } else {
                #[cfg(debug_assertions)]
                panic!("`allow_header` contained invalid uft-8. This should never happen")
            }
        }
    }
}
impl<S, E> Clone for MethodRouter<S, E> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            head: self.head.clone(),
            delete: self.delete.clone(),
            options: self.options.clone(),
            patch: self.patch.clone(),
            post: self.post.clone(),
            put: self.put.clone(),
            trace: self.trace.clone(),
            connect: self.connect.clone(),
            fallback: self.fallback.clone(),
            allow_header: self.allow_header.clone(),
        }
    }
}
impl<S, E> Default for MethodRouter<S, E>
where
    S: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}
enum MethodEndpoint<S, E> {
    None,
    Route(Route<E>),
    BoxedHandler(BoxedIntoRoute<S, E>),
}
impl<S, E> MethodEndpoint<S, E>
where
    S: Clone,
{
    fn is_some(&self) -> bool {
        matches!(self, Self::Route(_) | Self::BoxedHandler(_))
    }
    fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
    fn map<F, E2>(self, f: F) -> MethodEndpoint<S, E2>
    where
        S: 'static,
        E: 'static,
        F: FnOnce(Route<E>) -> Route<E2> + Clone + Send + 'static,
        E2: 'static,
    {
        match self {
            Self::None => MethodEndpoint::None,
            Self::Route(route) => MethodEndpoint::Route(f(route)),
            Self::BoxedHandler(handler) => MethodEndpoint::BoxedHandler(handler.map(f)),
        }
    }
    fn with_state<S2>(self, state: &S) -> MethodEndpoint<S2, E> {
        match self {
            MethodEndpoint::None => MethodEndpoint::None,
            MethodEndpoint::Route(route) => MethodEndpoint::Route(route),
            MethodEndpoint::BoxedHandler(handler) => {
                MethodEndpoint::Route(handler.into_route(state.clone()))
            }
        }
    }
}
impl<S, E> Clone for MethodEndpoint<S, E> {
    fn clone(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Route(inner) => Self::Route(inner.clone()),
            Self::BoxedHandler(inner) => Self::BoxedHandler(inner.clone()),
        }
    }
}
impl<S, E> fmt::Debug for MethodEndpoint<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.debug_tuple("None").finish(),
            Self::Route(inner) => inner.fmt(f),
            Self::BoxedHandler(_) => f.debug_tuple("BoxedHandler").finish(),
        }
    }
}
impl<B, E> Service<Request<B>> for MethodRouter<(), E>
where
    B: HttpBody<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    type Response = Response;
    type Error = E;
    type Future = RouteFuture<E>;
    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let req = req.map(Body::new);
        self.call_with_state(req, ())
    }
}
impl<S> Handler<(), S> for MethodRouter<S>
where
    S: Clone + 'static,
{
    type Future = InfallibleRouteFuture;
    fn call(self, req: Request, state: S) -> Self::Future {
        InfallibleRouteFuture::new(self.call_with_state(req, state))
    }
}
const _: () = {
    use crate::axum::serve::IncomingStream;
    impl Service<IncomingStream<'_>> for MethodRouter<()> {
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
