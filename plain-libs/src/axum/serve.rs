use crate::axum_core::{body::Body, extract::Request, response::Response};
use crate::hyper::body::Incoming;
use crate::hyper_util::rt::{TokioExecutor, TokioIo};
use crate::hyper_util::{server::conn::auto::Builder, service::TowerToHyperService};
use crate::tower::ServiceExt as _;
use futures_util::{FutureExt, pin_mut};
use std::{
    convert::Infallible,
    fmt::Debug,
    future::{Future, IntoFuture, poll_fn},
    io,
    marker::PhantomData,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::watch,
};
use tower_service::Service;
pub fn serve<M, S>(tcp_listener: TcpListener, make_service: M) -> Serve<M, S>
where
    M: for<'a> Service<IncomingStream<'a>, Error = Infallible, Response = S>,
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    Serve {
        tcp_listener,
        make_service,
        tcp_nodelay: None,
        _marker: PhantomData,
    }
}
#[must_use = "futures must be awaited or polled"]
pub struct Serve<M, S> {
    tcp_listener: TcpListener,
    make_service: M,
    tcp_nodelay: Option<bool>,
    _marker: PhantomData<S>,
}
impl<M, S> Serve<M, S> {
    pub fn with_graceful_shutdown<F>(self, signal: F) -> WithGracefulShutdown<M, S, F>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        WithGracefulShutdown {
            tcp_listener: self.tcp_listener,
            make_service: self.make_service,
            signal,
            tcp_nodelay: self.tcp_nodelay,
            _marker: PhantomData,
        }
    }
    pub fn tcp_nodelay(self, nodelay: bool) -> Self {
        Self {
            tcp_nodelay: Some(nodelay),
            ..self
        }
    }
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp_listener.local_addr()
    }
}
impl<M, S> Debug for Serve<M, S>
where
    M: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            tcp_listener,
            make_service,
            tcp_nodelay,
            _marker: _,
        } = self;
        f.debug_struct("Serve")
            .field("tcp_listener", tcp_listener)
            .field("make_service", make_service)
            .field("tcp_nodelay", tcp_nodelay)
            .finish()
    }
}
impl<M, S> IntoFuture for Serve<M, S>
where
    M: for<'a> Service<IncomingStream<'a>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a>>>::Future: Send,
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    type Output = io::Result<()>;
    type IntoFuture = private::ServeFuture;
    fn into_future(self) -> Self::IntoFuture {
        private::ServeFuture(Box::pin(async move {
            let Self {
                tcp_listener,
                mut make_service,
                tcp_nodelay,
                _marker: _,
            } = self;
            loop {
                let (tcp_stream, remote_addr) = match tcp_accept(&tcp_listener).await {
                    Some(conn) => conn,
                    None => continue,
                };
                if let Some(nodelay) = tcp_nodelay {
                    if let Err(err) = tcp_stream.set_nodelay(nodelay) {
                        trace!("failed to set TCP_NODELAY on incoming connection: {err:#}");
                    }
                }
                let tcp_stream = TokioIo::new(tcp_stream);
                poll_fn(|cx| make_service.poll_ready(cx))
                    .await
                    .unwrap_or_else(|err| match err {});
                let tower_service = make_service
                    .call(IncomingStream {
                        tcp_stream: &tcp_stream,
                        remote_addr,
                    })
                    .await
                    .unwrap_or_else(|err| match err {})
                    .map_request(|req: Request<Incoming>| req.map(Body::new));
                let hyper_service = TowerToHyperService::new(tower_service);
                tokio::spawn(async move {
                    match Builder::new(TokioExecutor::new())
                        .serve_connection_with_upgrades(tcp_stream, hyper_service)
                        .await
                    {
                        Ok(()) => {}
                        Err(_err) => {}
                    }
                });
            }
        }))
    }
}
#[must_use = "futures must be awaited or polled"]
pub struct WithGracefulShutdown<M, S, F> {
    tcp_listener: TcpListener,
    make_service: M,
    signal: F,
    tcp_nodelay: Option<bool>,
    _marker: PhantomData<S>,
}
impl<M, S, F> WithGracefulShutdown<M, S, F> {
    pub fn tcp_nodelay(self, nodelay: bool) -> Self {
        Self {
            tcp_nodelay: Some(nodelay),
            ..self
        }
    }
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp_listener.local_addr()
    }
}
impl<M, S, F> Debug for WithGracefulShutdown<M, S, F>
where
    M: Debug,
    S: Debug,
    F: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            tcp_listener,
            make_service,
            signal,
            tcp_nodelay,
            _marker: _,
        } = self;
        f.debug_struct("WithGracefulShutdown")
            .field("tcp_listener", tcp_listener)
            .field("make_service", make_service)
            .field("signal", signal)
            .field("tcp_nodelay", tcp_nodelay)
            .finish()
    }
}
impl<M, S, F> IntoFuture for WithGracefulShutdown<M, S, F>
where
    M: for<'a> Service<IncomingStream<'a>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a>>>::Future: Send,
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    F: Future<Output = ()> + Send + 'static,
{
    type Output = io::Result<()>;
    type IntoFuture = private::ServeFuture;
    fn into_future(self) -> Self::IntoFuture {
        let Self {
            tcp_listener,
            mut make_service,
            signal,
            tcp_nodelay,
            _marker: _,
        } = self;
        let (signal_tx, signal_rx) = watch::channel(());
        let signal_tx = Arc::new(signal_tx);
        tokio::spawn(async move {
            signal.await;
            trace!("received graceful shutdown signal. Telling tasks to shutdown");
            drop(signal_rx);
        });
        let (close_tx, close_rx) = watch::channel(());
        private::ServeFuture(Box::pin(async move {
            loop {
                let (tcp_stream, remote_addr) = tokio::select! { conn = tcp_accept (& tcp_listener) => { match conn { Some (conn) => conn , None => continue , } } _ = signal_tx . closed () => { trace ! ("signal received, not accepting new connections") ; break ; } };
                if let Some(nodelay) = tcp_nodelay {
                    if let Err(err) = tcp_stream.set_nodelay(nodelay) {
                        trace!("failed to set TCP_NODELAY on incoming connection: {err:#}");
                    }
                }
                let tcp_stream = TokioIo::new(tcp_stream);
                trace!("connection {remote_addr} accepted");
                poll_fn(|cx| make_service.poll_ready(cx))
                    .await
                    .unwrap_or_else(|err| match err {});
                let tower_service = make_service
                    .call(IncomingStream {
                        tcp_stream: &tcp_stream,
                        remote_addr,
                    })
                    .await
                    .unwrap_or_else(|err| match err {})
                    .map_request(|req: Request<Incoming>| req.map(Body::new));
                let hyper_service = TowerToHyperService::new(tower_service);
                let signal_tx = Arc::clone(&signal_tx);
                let close_rx = close_rx.clone();
                tokio::spawn(async move {
                    let builder = Builder::new(TokioExecutor::new());
                    let conn = builder.serve_connection_with_upgrades(tcp_stream, hyper_service);
                    pin_mut!(conn);
                    let signal_closed = signal_tx.closed().fuse();
                    pin_mut!(signal_closed);
                    loop {
                        tokio::select! { result = conn . as_mut () => { if let Err (_err) = result { trace ! ("failed to serve connection: {_err:#}") ; } break ; } _ = & mut signal_closed => { trace ! ("signal received in task, starting graceful shutdown") ; conn . as_mut () . graceful_shutdown () ; } }
                    }
                    trace!("connection {remote_addr} closed");
                    drop(close_rx);
                });
            }
            drop(close_rx);
            drop(tcp_listener);
            trace!(
                "waiting for {} task(s) to finish",
                close_tx.receiver_count()
            );
            close_tx.closed().await;
            Ok(())
        }))
    }
}
fn is_connection_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}
async fn tcp_accept(listener: &TcpListener) -> Option<(TcpStream, SocketAddr)> {
    match listener.accept().await {
        Ok(conn) => Some(conn),
        Err(e) => {
            if is_connection_error(&e) {
                return None;
            }
            error!("accept error: {e}");
            tokio::time::sleep(Duration::from_secs(1)).await;
            None
        }
    }
}
mod private {
    use std::{
        future::Future,
        io,
        pin::Pin,
        task::{Context, Poll},
    };
    pub struct ServeFuture(pub(super) futures_util::future::BoxFuture<'static, io::Result<()>>);
    impl Future for ServeFuture {
        type Output = io::Result<()>;
        #[inline]
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.0.as_mut().poll(cx)
        }
    }
    impl std::fmt::Debug for ServeFuture {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("ServeFuture").finish_non_exhaustive()
        }
    }
}
#[derive(Debug)]
pub struct IncomingStream<'a> {
    tcp_stream: &'a TokioIo<TcpStream>,
    remote_addr: SocketAddr,
}
impl IncomingStream<'_> {
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.tcp_stream.inner().local_addr()
    }
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
}
