use super::super::dispatch::{self, TrySendError};
use crate::http::{Request, Response};
use crate::httparse::ParserConfig;
use crate::hyper::body::{Body, Incoming as IncomingBody};
use crate::hyper::proto;
use crate::hyper::rt::{Read, Write};
use bytes::Bytes;
use futures_core::ready;
use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
type Dispatcher<T, B> =
    proto::dispatch::Dispatcher<proto::dispatch::Client<B>, B, T, proto::h1::ClientTransaction>;
pub struct SendRequest<B> {
    dispatch: dispatch::Sender<Request<B>, Response<IncomingBody>>,
}
#[derive(Debug)]
#[non_exhaustive]
pub struct Parts<T> {
    pub io: T,
    pub read_buf: Bytes,
}
#[must_use = "futures do nothing unless polled"]
pub struct Connection<T, B>
where
    T: Read + Write,
    B: Body + 'static,
{
    inner: Dispatcher<T, B>,
}
impl<T, B> Connection<T, B>
where
    T: Read + Write + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub fn into_parts(self) -> Parts<T> {
        let (io, read_buf, _) = self.inner.into_inner();
        Parts { io, read_buf }
    }
    pub fn poll_without_shutdown(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<crate::hyper::Result<()>> {
        self.inner.poll_without_shutdown(cx)
    }
    pub async fn without_shutdown(self) -> crate::hyper::Result<Parts<T>> {
        let mut conn = Some(self);
        crate::hyper::common::future::poll_fn(move |cx| -> Poll<crate::hyper::Result<Parts<T>>> {
            ready!(
                conn.as_mut()
                    .expect("client connection polled after completion")
                    .poll_without_shutdown(cx)
            )?;
            Poll::Ready(Ok(conn
                .take()
                .expect("client connection missing before completion")
                .into_parts()))
        })
        .await
    }
}
#[derive(Clone, Debug)]
pub struct Builder {
    h09_responses: bool,
    h1_parser_config: ParserConfig,
    h1_writev: Option<bool>,
    h1_title_case_headers: bool,
    h1_preserve_header_case: bool,
    h1_max_headers: Option<usize>,
    h1_read_buf_exact_size: Option<usize>,
    h1_max_buf_size: Option<usize>,
}
pub async fn handshake<T, B>(io: T) -> crate::hyper::Result<(SendRequest<B>, Connection<T, B>)>
where
    T: Read + Write + Unpin,
    B: Body + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    Builder::new().handshake(io).await
}
impl<B> SendRequest<B> {
    pub fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<crate::hyper::Result<()>> {
        self.dispatch.poll_ready(cx)
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
impl<T, B> Connection<T, B>
where
    T: Read + Write + Unpin + Send,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub fn with_upgrades(self) -> upgrades::UpgradeableConnection<T, B> {
        upgrades::UpgradeableConnection { inner: Some(self) }
    }
}
impl<T, B> fmt::Debug for Connection<T, B>
where
    T: Read + Write + fmt::Debug,
    B: Body + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish()
    }
}
impl<T, B> Future for Connection<T, B>
where
    T: Read + Write + Unpin,
    B: Body + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Output = crate::hyper::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(Pin::new(&mut self.inner).poll(cx))? {
            proto::Dispatched::Shutdown => Poll::Ready(Ok(())),
            proto::Dispatched::Upgrade(pending) => {
                pending.manual();
                Poll::Ready(Ok(()))
            }
        }
    }
}
impl Builder {
    #[inline]
    pub fn new() -> Builder {
        Builder {
            h09_responses: false,
            h1_writev: None,
            h1_read_buf_exact_size: None,
            h1_parser_config: ParserConfig::default(),
            h1_title_case_headers: false,
            h1_preserve_header_case: false,
            h1_max_headers: None,
            h1_max_buf_size: None,
        }
    }
    pub fn http09_responses(&mut self, enabled: bool) -> &mut Builder {
        self.h09_responses = enabled;
        self
    }
    pub fn allow_spaces_after_header_name_in_responses(&mut self, enabled: bool) -> &mut Builder {
        self.h1_parser_config
            .allow_spaces_after_header_name_in_responses(enabled);
        self
    }
    pub fn allow_obsolete_multiline_headers_in_responses(&mut self, enabled: bool) -> &mut Builder {
        self.h1_parser_config
            .allow_obsolete_multiline_headers_in_responses(enabled);
        self
    }
    pub fn ignore_invalid_headers_in_responses(&mut self, enabled: bool) -> &mut Builder {
        self.h1_parser_config
            .ignore_invalid_headers_in_responses(enabled);
        self
    }
    pub fn writev(&mut self, enabled: bool) -> &mut Builder {
        self.h1_writev = Some(enabled);
        self
    }
    pub fn title_case_headers(&mut self, enabled: bool) -> &mut Builder {
        self.h1_title_case_headers = enabled;
        self
    }
    pub fn preserve_header_case(&mut self, enabled: bool) -> &mut Builder {
        self.h1_preserve_header_case = enabled;
        self
    }
    pub fn max_headers(&mut self, val: usize) -> &mut Self {
        self.h1_max_headers = Some(val);
        self
    }
    pub fn read_buf_exact_size(&mut self, sz: Option<usize>) -> &mut Builder {
        self.h1_read_buf_exact_size = sz;
        self.h1_max_buf_size = None;
        self
    }
    pub fn max_buf_size(&mut self, max: usize) -> &mut Self {
        assert!(
            max >= proto::h1::MINIMUM_MAX_BUFFER_SIZE,
            "the max_buf_size cannot be smaller than the minimum that h1 specifies."
        );
        self.h1_max_buf_size = Some(max);
        self.h1_read_buf_exact_size = None;
        self
    }
    pub fn handshake<T, B>(
        &self,
        io: T,
    ) -> impl Future<Output = crate::hyper::Result<(SendRequest<B>, Connection<T, B>)>>
    where
        T: Read + Write + Unpin,
        B: Body + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        let opts = self.clone();
        async move {
            trace!("client handshake HTTP/1");
            let (tx, rx) = dispatch::channel();
            let mut conn = proto::Conn::new(io);
            conn.set_h1_parser_config(opts.h1_parser_config);
            if let Some(writev) = opts.h1_writev {
                if writev {
                    conn.set_write_strategy_queue();
                } else {
                    conn.set_write_strategy_flatten();
                }
            }
            if opts.h1_title_case_headers {
                conn.set_title_case_headers();
            }
            if opts.h1_preserve_header_case {
                conn.set_preserve_header_case();
            }
            if let Some(max_headers) = opts.h1_max_headers {
                conn.set_http1_max_headers(max_headers);
            }
            if opts.h09_responses {
                conn.set_h09_responses();
            }
            if let Some(sz) = opts.h1_read_buf_exact_size {
                conn.set_read_buf_exact_size(sz);
            }
            if let Some(max) = opts.h1_max_buf_size {
                conn.set_max_buf_size(max);
            }
            let cd = proto::h1::dispatch::Client::new(rx);
            let proto = proto::h1::Dispatcher::new(cd, conn);
            Ok((SendRequest { dispatch: tx }, Connection { inner: proto }))
        }
    }
}
mod upgrades {
    use super::{Connection, Context, Future, Parts, Pin, Poll, Read, StdError, Write};
    use crate::hyper::body::Body;
    use crate::hyper::proto::Dispatched;
    use crate::hyper::upgrade::Upgraded;
    use futures_core::ready;
    #[must_use = "futures do nothing unless polled"]
    pub struct UpgradeableConnection<T, B>
    where
        T: Read + Write + Unpin + Send + 'static,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        pub(super) inner: Option<Connection<T, B>>,
    }
    impl<I, B> Future for UpgradeableConnection<I, B>
    where
        I: Read + Write + Unpin + Send + 'static,
        B: Body + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        type Output = crate::hyper::Result<()>;
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            match ready!(
                Pin::new(
                    &mut self
                        .inner
                        .as_mut()
                        .expect("upgradeable client connection polled after upgrade")
                        .inner,
                )
                .poll(cx)
            ) {
                Ok(Dispatched::Shutdown) => Poll::Ready(Ok(())),
                Ok(Dispatched::Upgrade(pending)) => {
                    let Parts { io, read_buf } = self
                        .inner
                        .take()
                        .expect("upgradeable client connection missing after upgrade")
                        .into_parts();
                    pending.fulfill(Upgraded::new(io, read_buf));
                    Poll::Ready(Ok(()))
                }
                Err(e) => Poll::Ready(Err(e)),
            }
        }
    }
}
