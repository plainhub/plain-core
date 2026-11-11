use crate::httparse::ParserConfig;
use crate::hyper::body::{Body, Incoming as IncomingBody};
use crate::hyper::proto;
use crate::hyper::rt::{Read, Write};
use crate::hyper::service::HttpService;
use crate::hyper::upgrade::Upgraded;
use crate::hyper::{
    common::time::{Dur, Time},
    rt::Timer,
};
use bytes::Bytes;
use futures_core::ready;
use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
type Http1Dispatcher<T, B, S> = proto::h1::Dispatcher<
    proto::h1::dispatch::Server<S, IncomingBody>,
    B,
    T,
    proto::ServerTransaction,
>;
pin_project_lite::pin_project! { #[must_use = "futures do nothing unless polled"] pub struct Connection < T , S > where S : HttpService < IncomingBody >, { conn : Http1Dispatcher < T , S::ResBody , S >, } }
#[derive(Clone, Debug)]
pub struct Builder {
    h1_parser_config: ParserConfig,
    timer: Time,
    h1_half_close: bool,
    h1_keep_alive: bool,
    h1_title_case_headers: bool,
    h1_preserve_header_case: bool,
    h1_max_headers: Option<usize>,
    h1_header_read_timeout: Dur,
    h1_writev: Option<bool>,
    max_buf_size: Option<usize>,
    pipeline_flush: bool,
    date_header: bool,
}
#[derive(Debug)]
#[non_exhaustive]
pub struct Parts<T, S> {
    pub io: T,
    pub read_buf: Bytes,
    pub service: S,
}
impl<I, S> fmt::Debug for Connection<I, S>
where
    S: HttpService<IncomingBody>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish()
    }
}
impl<I, B, S> Connection<I, S>
where
    S: HttpService<IncomingBody, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub fn graceful_shutdown(mut self: Pin<&mut Self>) {
        self.conn.disable_keep_alive();
    }
    pub fn into_parts(self) -> Parts<I, S> {
        let (io, read_buf, dispatch) = self.conn.into_inner();
        Parts {
            io,
            read_buf,
            service: dispatch.into_service(),
        }
    }
    pub fn poll_without_shutdown(&mut self, cx: &mut Context<'_>) -> Poll<crate::hyper::Result<()>>
    where
        S: Unpin,
        S::Future: Unpin,
    {
        self.conn.poll_without_shutdown(cx)
    }
    pub fn without_shutdown(self) -> impl Future<Output = crate::hyper::Result<Parts<I, S>>> {
        let mut zelf = Some(self);
        crate::hyper::common::future::poll_fn(move |cx| {
            ready!(
                zelf.as_mut()
                    .expect("server connection polled after completion")
                    .conn
                    .poll_without_shutdown(cx)
            )?;
            Poll::Ready(Ok(zelf
                .take()
                .expect("server connection missing before completion")
                .into_parts()))
        })
    }
    pub fn with_upgrades(self) -> UpgradeableConnection<I, S>
    where
        I: Send,
    {
        UpgradeableConnection { inner: Some(self) }
    }
}
impl<I, B, S> Future for Connection<I, S>
where
    S: HttpService<IncomingBody, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Output = crate::hyper::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(Pin::new(&mut self.conn).poll(cx)) {
            Ok(done) => {
                match done {
                    proto::Dispatched::Shutdown => {}
                    proto::Dispatched::Upgrade(pending) => {
                        pending.manual();
                    }
                }
                Poll::Ready(Ok(()))
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}
impl Builder {
    pub fn new() -> Self {
        Self {
            h1_parser_config: ParserConfig::default(),
            timer: Time::Empty,
            h1_half_close: false,
            h1_keep_alive: true,
            h1_title_case_headers: false,
            h1_preserve_header_case: false,
            h1_max_headers: None,
            h1_header_read_timeout: Dur::Default(Some(Duration::from_secs(30))),
            h1_writev: None,
            max_buf_size: None,
            pipeline_flush: false,
            date_header: true,
        }
    }
    pub fn half_close(&mut self, val: bool) -> &mut Self {
        self.h1_half_close = val;
        self
    }
    pub fn keep_alive(&mut self, val: bool) -> &mut Self {
        self.h1_keep_alive = val;
        self
    }
    pub fn title_case_headers(&mut self, enabled: bool) -> &mut Self {
        self.h1_title_case_headers = enabled;
        self
    }
    pub fn allow_multiple_spaces_in_request_line_delimiters(&mut self, enabled: bool) -> &mut Self {
        self.h1_parser_config
            .allow_multiple_spaces_in_request_line_delimiters(enabled);
        self
    }
    pub fn ignore_invalid_headers(&mut self, enabled: bool) -> &mut Builder {
        self.h1_parser_config
            .ignore_invalid_headers_in_requests(enabled);
        self
    }
    pub fn preserve_header_case(&mut self, enabled: bool) -> &mut Self {
        self.h1_preserve_header_case = enabled;
        self
    }
    pub fn max_headers(&mut self, val: usize) -> &mut Self {
        self.h1_max_headers = Some(val);
        self
    }
    pub fn header_read_timeout(&mut self, read_timeout: impl Into<Option<Duration>>) -> &mut Self {
        self.h1_header_read_timeout = Dur::Configured(read_timeout.into());
        self
    }
    pub fn writev(&mut self, val: bool) -> &mut Self {
        self.h1_writev = Some(val);
        self
    }
    pub fn max_buf_size(&mut self, max: usize) -> &mut Self {
        assert!(
            max >= proto::h1::MINIMUM_MAX_BUFFER_SIZE,
            "the max_buf_size cannot be smaller than the minimum that h1 specifies."
        );
        self.max_buf_size = Some(max);
        self
    }
    pub fn auto_date_header(&mut self, enabled: bool) -> &mut Self {
        self.date_header = enabled;
        self
    }
    pub fn pipeline_flush(&mut self, enabled: bool) -> &mut Self {
        self.pipeline_flush = enabled;
        self
    }
    pub fn timer<M>(&mut self, timer: M) -> &mut Self
    where
        M: Timer + Send + Sync + 'static,
    {
        self.timer = Time::Timer(Arc::new(timer));
        self
    }
    pub fn serve_connection<I, S>(&self, io: I, service: S) -> Connection<I, S>
    where
        S: HttpService<IncomingBody>,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        S::ResBody: 'static,
        <S::ResBody as Body>::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin,
    {
        let mut conn = proto::Conn::new(io);
        conn.set_h1_parser_config(self.h1_parser_config.clone());
        conn.set_timer(self.timer.clone());
        if !self.h1_keep_alive {
            conn.disable_keep_alive();
        }
        if self.h1_half_close {
            conn.set_allow_half_close();
        }
        if self.h1_title_case_headers {
            conn.set_title_case_headers();
        }
        if self.h1_preserve_header_case {
            conn.set_preserve_header_case();
        }
        if let Some(max_headers) = self.h1_max_headers {
            conn.set_http1_max_headers(max_headers);
        }
        if let Some(dur) = self
            .timer
            .check(self.h1_header_read_timeout, "header_read_timeout")
        {
            conn.set_http1_header_read_timeout(dur);
        }
        if let Some(writev) = self.h1_writev {
            if writev {
                conn.set_write_strategy_queue();
            } else {
                conn.set_write_strategy_flatten();
            }
        }
        conn.set_flush_pipeline(self.pipeline_flush);
        if let Some(max) = self.max_buf_size {
            conn.set_max_buf_size(max);
        }
        if !self.date_header {
            conn.disable_date_header();
        }
        let sd = proto::h1::dispatch::Server::new(service);
        let proto = proto::h1::Dispatcher::new(sd, conn);
        Connection { conn: proto }
    }
}
#[must_use = "futures do nothing unless polled"]
pub struct UpgradeableConnection<T, S>
where
    S: HttpService<IncomingBody>,
{
    pub(super) inner: Option<Connection<T, S>>,
}
impl<I, B, S> UpgradeableConnection<I, S>
where
    S: HttpService<IncomingBody, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub fn graceful_shutdown(mut self: Pin<&mut Self>) {
        if let Some(conn) = self.inner.as_mut() {
            Pin::new(conn).graceful_shutdown();
        }
    }
    pub fn into_parts(self) -> Option<Parts<I, S>> {
        self.inner.map(|conn| conn.into_parts())
    }
}
impl<I, B, S> Future for UpgradeableConnection<I, S>
where
    S: HttpService<IncomingBody, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin + Send + 'static,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Output = crate::hyper::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(conn) = self.inner.as_mut() {
            match ready!(Pin::new(&mut conn.conn).poll(cx)) {
                Ok(proto::Dispatched::Shutdown) => Poll::Ready(Ok(())),
                Ok(proto::Dispatched::Upgrade(pending)) => {
                    let (io, buf, _) = self
                        .inner
                        .take()
                        .expect("upgradeable server connection missing after upgrade")
                        .conn
                        .into_inner();
                    pending.fulfill(Upgraded::new(io, buf));
                    Poll::Ready(Ok(()))
                }
                Err(e) => Poll::Ready(Err(e)),
            }
        } else {
            Poll::Ready(Ok(()))
        }
    }
}
