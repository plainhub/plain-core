use crate::hyper::body::{Body, Incoming as IncomingBody};
use crate::hyper::proto;
use crate::hyper::rt::bounds::Http2ServerConnExec;
use crate::hyper::rt::{Read, Write};
use crate::hyper::service::HttpService;
use crate::hyper::{common::time::Time, rt::Timer};
use futures_core::ready;
use pin_project_lite::pin_project;
use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
pin_project! { #[must_use = "futures do nothing unless polled"] pub struct Connection < T , S , E > where S : HttpService < IncomingBody >, { conn : proto::h2::Server < T , S , S::ResBody , E >, } }
#[derive(Clone, Debug)]
pub struct Builder<E> {
    exec: E,
    timer: Time,
    h2_builder: proto::h2::server::Config,
}
impl<I, S, E> fmt::Debug for Connection<I, S, E>
where
    S: HttpService<IncomingBody>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish()
    }
}
impl<I, B, S, E> Connection<I, S, E>
where
    S: HttpService<IncomingBody, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: Http2ServerConnExec<S::Future, B>,
{
    pub fn graceful_shutdown(mut self: Pin<&mut Self>) {
        self.conn.graceful_shutdown();
    }
}
impl<I, B, S, E> Future for Connection<I, S, E>
where
    S: HttpService<IncomingBody, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: Http2ServerConnExec<S::Future, B>,
{
    type Output = crate::hyper::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(Pin::new(&mut self.conn).poll(cx)) {
            Ok(_done) => Poll::Ready(Ok(())),
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}
impl<E> Builder<E> {
    pub fn new(exec: E) -> Self {
        Self {
            exec,
            timer: Time::Empty,
            h2_builder: proto::h2::server::Config::default(),
        }
    }
    pub fn max_pending_accept_reset_streams(&mut self, max: impl Into<Option<usize>>) -> &mut Self {
        self.h2_builder.max_pending_accept_reset_streams = max.into();
        self
    }
    pub fn max_local_error_reset_streams(&mut self, max: impl Into<Option<usize>>) -> &mut Self {
        self.h2_builder.max_local_error_reset_streams = max.into();
        self
    }
    pub fn initial_stream_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        if let Some(sz) = sz.into() {
            self.h2_builder.adaptive_window = false;
            self.h2_builder.initial_stream_window_size = sz;
        }
        self
    }
    pub fn initial_connection_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        if let Some(sz) = sz.into() {
            self.h2_builder.adaptive_window = false;
            self.h2_builder.initial_conn_window_size = sz;
        }
        self
    }
    pub fn adaptive_window(&mut self, enabled: bool) -> &mut Self {
        use proto::h2::SPEC_WINDOW_SIZE;
        self.h2_builder.adaptive_window = enabled;
        if enabled {
            self.h2_builder.initial_conn_window_size = SPEC_WINDOW_SIZE;
            self.h2_builder.initial_stream_window_size = SPEC_WINDOW_SIZE;
        }
        self
    }
    pub fn max_frame_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        if let Some(sz) = sz.into() {
            self.h2_builder.max_frame_size = sz;
        }
        self
    }
    pub fn max_concurrent_streams(&mut self, max: impl Into<Option<u32>>) -> &mut Self {
        self.h2_builder.max_concurrent_streams = max.into();
        self
    }
    pub fn keep_alive_interval(&mut self, interval: impl Into<Option<Duration>>) -> &mut Self {
        self.h2_builder.keep_alive_interval = interval.into();
        self
    }
    pub fn keep_alive_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.h2_builder.keep_alive_timeout = timeout;
        self
    }
    pub fn max_send_buf_size(&mut self, max: usize) -> &mut Self {
        assert!(max <= u32::MAX as usize);
        self.h2_builder.max_send_buffer_size = max;
        self
    }
    pub fn enable_connect_protocol(&mut self) -> &mut Self {
        self.h2_builder.enable_connect_protocol = true;
        self
    }
    pub fn header_table_size(&mut self, size: impl Into<Option<u32>>) -> &mut Self {
        self.h2_builder.header_table_size = size.into();
        self
    }
    pub fn max_header_list_size(&mut self, max: u32) -> &mut Self {
        self.h2_builder.max_header_list_size = max;
        self
    }
    pub fn timer<M>(&mut self, timer: M) -> &mut Self
    where
        M: Timer + Send + Sync + 'static,
    {
        self.timer = Time::Timer(Arc::new(timer));
        self
    }
    pub fn auto_date_header(&mut self, enabled: bool) -> &mut Self {
        self.h2_builder.date_header = enabled;
        self
    }
    pub fn serve_connection<S, I, Bd>(&self, io: I, service: S) -> Connection<I, S, E>
    where
        S: HttpService<IncomingBody, ResBody = Bd>,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        Bd: Body + 'static,
        Bd::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin,
        E: Http2ServerConnExec<S::Future, Bd>,
    {
        let proto = proto::h2::Server::new(
            io,
            service,
            &self.h2_builder,
            self.exec.clone(),
            self.timer.clone(),
        );
        Connection { conn: proto }
    }
}
