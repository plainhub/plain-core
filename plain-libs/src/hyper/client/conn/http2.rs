use super::super::dispatch::{self, TrySendError};
use crate::http::{Request, Response};
use crate::hyper::body::{Body, Incoming as IncomingBody};
use crate::hyper::common::time::Time;
use crate::hyper::proto;
use crate::hyper::rt::Timer;
use crate::hyper::rt::bounds::Http2ClientConnExec;
use crate::hyper::rt::{Read, Write};
use futures_core::ready;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
pub struct SendRequest<B> {
    dispatch: dispatch::UnboundedSender<Request<B>, Response<IncomingBody>>,
}
impl<B> Clone for SendRequest<B> {
    fn clone(&self) -> SendRequest<B> {
        SendRequest {
            dispatch: self.dispatch.clone(),
        }
    }
}
#[must_use = "futures do nothing unless polled"]
pub struct Connection<T, B, E>
where
    T: Read + Write + Unpin,
    B: Body + 'static,
    E: Http2ClientConnExec<B, T> + Unpin,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
{
    inner: (PhantomData<T>, proto::h2::ClientTask<B, E, T>),
}
#[derive(Clone, Debug)]
pub struct Builder<Ex> {
    pub(super) exec: Ex,
    pub(super) timer: Time,
    h2_builder: proto::h2::client::Config,
}
pub async fn handshake<E, T, B>(
    exec: E,
    io: T,
) -> crate::hyper::Result<(SendRequest<B>, Connection<T, B, E>)>
where
    T: Read + Write + Unpin,
    B: Body + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
    E: Http2ClientConnExec<B, T> + Unpin + Clone,
{
    Builder::new(exec).handshake(io).await
}
impl<B> SendRequest<B> {
    pub fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<crate::hyper::Result<()>> {
        if self.is_closed() {
            Poll::Ready(Err(crate::hyper::Error::new_closed()))
        } else {
            Poll::Ready(Ok(()))
        }
    }
    pub async fn ready(&mut self) -> crate::hyper::Result<()> {
        crate::hyper::common::future::poll_fn(|cx| self.poll_ready(cx)).await
    }
    pub fn is_ready(&self) -> bool {
        self.dispatch.is_ready()
    }
    pub fn is_closed(&self) -> bool {
        self.dispatch.is_closed()
    }
}
impl<B> SendRequest<B>
where
    B: Body + 'static,
{
    pub fn send_request(
        &mut self,
        req: Request<B>,
    ) -> impl Future<Output = crate::hyper::Result<Response<IncomingBody>>> {
        let sent = self.dispatch.send(req);
        async move {
            match sent {
                Ok(rx) => match rx.await {
                    Ok(Ok(resp)) => Ok(resp),
                    Ok(Err(err)) => Err(err),
                    Err(_canceled) => panic!("dispatch dropped without returning error"),
                },
                Err(_req) => {
                    debug!("connection was not ready");
                    Err(crate::hyper::Error::new_canceled().with("connection was not ready"))
                }
            }
        }
    }
    pub fn try_send_request(
        &mut self,
        req: Request<B>,
    ) -> impl Future<Output = Result<Response<IncomingBody>, TrySendError<Request<B>>>> {
        let sent = self.dispatch.try_send(req);
        async move {
            match sent {
                Ok(rx) => match rx.await {
                    Ok(Ok(res)) => Ok(res),
                    Ok(Err(err)) => Err(err),
                    Err(_) => panic!("dispatch dropped without returning error"),
                },
                Err(req) => {
                    debug!("connection was not ready");
                    let error =
                        crate::hyper::Error::new_canceled().with("connection was not ready");
                    Err(TrySendError {
                        error,
                        message: Some(req),
                    })
                }
            }
        }
    }
}
impl<B> fmt::Debug for SendRequest<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendRequest").finish()
    }
}
impl<T, B, E> Connection<T, B, E>
where
    T: Read + Write + Unpin + 'static,
    B: Body + Unpin + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
    E: Http2ClientConnExec<B, T> + Unpin,
{
    pub fn is_extended_connect_protocol_enabled(&self) -> bool {
        self.inner.1.is_extended_connect_protocol_enabled()
    }
    pub fn current_max_send_streams(&self) -> usize {
        self.inner.1.current_max_send_streams()
    }
    pub fn current_max_recv_streams(&self) -> usize {
        self.inner.1.current_max_recv_streams()
    }
}
impl<T, B, E> fmt::Debug for Connection<T, B, E>
where
    T: Read + Write + fmt::Debug + 'static + Unpin,
    B: Body + 'static,
    E: Http2ClientConnExec<B, T> + Unpin,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish()
    }
}
impl<T, B, E> Future for Connection<T, B, E>
where
    T: Read + Write + Unpin + 'static,
    B: Body + 'static + Unpin,
    B::Data: Send,
    E: Unpin,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
    E: Http2ClientConnExec<B, T> + Unpin,
{
    type Output = crate::hyper::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(Pin::new(&mut self.inner.1).poll(cx))? {
            proto::Dispatched::Shutdown => Poll::Ready(Ok(())),
            proto::Dispatched::Upgrade(_pending) => unreachable!("http2 cannot upgrade"),
        }
    }
}
impl<Ex> Builder<Ex>
where
    Ex: Clone,
{
    #[inline]
    pub fn new(exec: Ex) -> Builder<Ex> {
        Builder {
            exec,
            timer: Time::Empty,
            h2_builder: proto::h2::client::Config::default(),
        }
    }
    pub fn timer<M>(&mut self, timer: M) -> &mut Builder<Ex>
    where
        M: Timer + Send + Sync + 'static,
    {
        self.timer = Time::Timer(Arc::new(timer));
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
    pub fn initial_max_send_streams(&mut self, initial: impl Into<Option<usize>>) -> &mut Self {
        if let Some(initial) = initial.into() {
            self.h2_builder.initial_max_send_streams = initial;
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
        self.h2_builder.max_frame_size = sz.into();
        self
    }
    pub fn max_header_list_size(&mut self, max: u32) -> &mut Self {
        self.h2_builder.max_header_list_size = max;
        self
    }
    pub fn header_table_size(&mut self, size: impl Into<Option<u32>>) -> &mut Self {
        self.h2_builder.header_table_size = size.into();
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
    pub fn keep_alive_while_idle(&mut self, enabled: bool) -> &mut Self {
        self.h2_builder.keep_alive_while_idle = enabled;
        self
    }
    pub fn max_concurrent_reset_streams(&mut self, max: usize) -> &mut Self {
        self.h2_builder.max_concurrent_reset_streams = Some(max);
        self
    }
    pub fn max_send_buf_size(&mut self, max: usize) -> &mut Self {
        assert!(max <= u32::MAX as usize);
        self.h2_builder.max_send_buffer_size = max;
        self
    }
    pub fn max_pending_accept_reset_streams(&mut self, max: impl Into<Option<usize>>) -> &mut Self {
        self.h2_builder.max_pending_accept_reset_streams = max.into();
        self
    }
    pub fn max_local_error_reset_streams(&mut self, max: impl Into<Option<usize>>) -> &mut Self {
        self.h2_builder.max_local_error_reset_streams = max.into();
        self
    }
    pub fn reset_stream_duration(&mut self, dur: Duration) -> &mut Self {
        self.h2_builder.reset_stream_duration = Some(dur);
        self
    }
    pub fn handshake<T, B>(
        &self,
        io: T,
    ) -> impl Future<Output = crate::hyper::Result<(SendRequest<B>, Connection<T, B, Ex>)>>
    where
        T: Read + Write + Unpin,
        B: Body + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
        Ex: Http2ClientConnExec<B, T> + Unpin,
    {
        let opts = self.clone();
        async move {
            trace!("client handshake HTTP/2");
            let (tx, rx) = dispatch::channel();
            let h2 = proto::h2::client::handshake(io, rx, &opts.h2_builder, opts.exec, opts.timer)
                .await?;
            Ok((
                SendRequest {
                    dispatch: tx.unbound(),
                },
                Connection {
                    inner: (PhantomData, h2),
                },
            ))
        }
    }
}
