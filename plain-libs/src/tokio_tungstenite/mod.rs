pub use crate::tungstenite;
mod compat;
mod connect;
mod handshake;
mod stream;
mod tls;
use crate::tungstenite::protocol::CloseFrame;
use crate::tungstenite::{
    client::IntoClientRequest,
    handshake::{
        HandshakeError,
        client::{ClientHandshake, Response},
        server::{Callback, NoCallback},
    },
};
use crate::tungstenite::{
    error::Error as WsError,
    protocol::{Message, Role, WebSocket, WebSocketConfig},
};
use compat::{AllowStd, ContextWaker, cvt};
pub use connect::connect_async_tls_with_config;
pub use connect::{connect_async, connect_async_with_config};
use futures_util::{
    sink::{Sink, SinkExt},
    stream::{FusedStream, Stream},
};
use log::*;
use std::io::{Read, Write};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
pub use stream::MaybeTlsStream;
pub use tls::Connector;
pub use tls::{client_async_tls, client_async_tls_with_config};
use tokio::io::{AsyncRead, AsyncWrite};
pub async fn client_async<'a, R, S>(
    request: R,
    stream: S,
) -> Result<(WebSocketStream<S>, Response), WsError>
where
    R: IntoClientRequest + Unpin,
    S: AsyncRead + AsyncWrite + Unpin,
{
    client_async_with_config(request, stream, None).await
}
pub async fn client_async_with_config<'a, R, S>(
    request: R,
    stream: S,
    config: Option<WebSocketConfig>,
) -> Result<(WebSocketStream<S>, Response), WsError>
where
    R: IntoClientRequest + Unpin,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let f = handshake::client_handshake(stream, move |allow_std| {
        let request = request.into_client_request()?;
        let cli_handshake = ClientHandshake::start(allow_std, request, config)?;
        cli_handshake.handshake()
    });
    f.await.map_err(|e| match e {
        HandshakeError::Failure(e) => e,
        e => WsError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        )),
    })
}
pub async fn accept_async<S>(stream: S) -> Result<WebSocketStream<S>, WsError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    accept_hdr_async(stream, NoCallback).await
}

pub async fn accept_hdr_async<S, C>(stream: S, callback: C) -> Result<WebSocketStream<S>, WsError>
where
    S: AsyncRead + AsyncWrite + Unpin,
    C: Callback + Unpin,
{
    accept_hdr_async_with_config(stream, callback, None).await
}
pub async fn accept_hdr_async_with_config<S, C>(
    stream: S,
    callback: C,
    config: Option<WebSocketConfig>,
) -> Result<WebSocketStream<S>, WsError>
where
    S: AsyncRead + AsyncWrite + Unpin,
    C: Callback + Unpin,
{
    let f = handshake::server_handshake(stream, move |allow_std| {
        crate::tungstenite::accept_hdr_with_config(allow_std, callback, config)
    });
    f.await.map_err(|e| match e {
        HandshakeError::Failure(e) => e,
        e => WsError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        )),
    })
}
#[derive(Debug)]
pub struct WebSocketStream<S> {
    inner: WebSocket<AllowStd<S>>,
    closing: bool,
    ended: bool,
    ready: bool,
}
impl<S> WebSocketStream<S> {
    pub async fn from_raw_socket(stream: S, role: Role, config: Option<WebSocketConfig>) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        handshake::without_handshake(stream, move |allow_std| {
            WebSocket::from_raw_socket(allow_std, role, config)
        })
        .await
    }
    pub async fn from_partially_read(
        stream: S,
        part: Vec<u8>,
        role: Role,
        config: Option<WebSocketConfig>,
    ) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        handshake::without_handshake(stream, move |allow_std| {
            WebSocket::from_partially_read(allow_std, part, role, config)
        })
        .await
    }
    pub(crate) fn new(ws: WebSocket<AllowStd<S>>) -> Self {
        Self {
            inner: ws,
            closing: false,
            ended: false,
            ready: true,
        }
    }
    fn with_context<F, R>(&mut self, ctx: Option<(ContextWaker, &mut Context<'_>)>, f: F) -> R
    where
        S: Unpin,
        F: FnOnce(&mut WebSocket<AllowStd<S>>) -> R,
        AllowStd<S>: Read + Write,
    {
        trace!("{}:{} WebSocketStream.with_context", file!(), line!());
        if let Some((kind, ctx)) = ctx {
            self.inner.get_mut().set_waker(kind, ctx.waker());
        }
        f(&mut self.inner)
    }
    pub fn into_inner(self) -> S {
        self.inner.into_inner().into_inner()
    }
    pub fn get_ref(&self) -> &S
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        self.inner.get_ref().get_ref()
    }
    pub fn get_mut(&mut self) -> &mut S
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        self.inner.get_mut().get_mut()
    }
    pub fn get_config(&self) -> &WebSocketConfig {
        self.inner.get_config()
    }
    pub async fn close(&mut self, msg: Option<CloseFrame>) -> Result<(), WsError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        self.send(Message::Close(msg)).await
    }
}
impl<T> Stream for WebSocketStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = Result<Message, WsError>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        trace!("{}:{} Stream.poll_next", file!(), line!());
        if self.ended {
            return Poll::Ready(None);
        }
        match futures_util::ready!(self.with_context(Some((ContextWaker::Read, cx)), |s| {
            trace!(
                "{}:{} Stream.with_context poll_next -> read()",
                file!(),
                line!()
            );
            cvt(s.read())
        })) {
            Ok(v) => Poll::Ready(Some(Ok(v))),
            Err(e) => {
                self.ended = true;
                if matches!(e, WsError::AlreadyClosed | WsError::ConnectionClosed) {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Err(e)))
                }
            }
        }
    }
}
impl<T> FusedStream for WebSocketStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn is_terminated(&self) -> bool {
        self.ended
    }
}
impl<T> Sink<Message> for WebSocketStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Error = WsError;
    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.ready {
            Poll::Ready(Ok(()))
        } else {
            (*self)
                .with_context(Some((ContextWaker::Write, cx)), |s| cvt(s.flush()))
                .map(|r| {
                    self.ready = true;
                    r
                })
        }
    }
    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        match (*self).with_context(None, |s| s.write(item)) {
            Ok(()) => {
                self.ready = true;
                Ok(())
            }
            Err(WsError::Io(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {
                self.ready = false;
                Ok(())
            }
            Err(e) => {
                self.ready = true;
                debug!("websocket start_send error: {}", e);
                Err(e)
            }
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        (*self)
            .with_context(Some((ContextWaker::Write, cx)), |s| cvt(s.flush()))
            .map(|r| {
                self.ready = true;
                match r {
                    Err(WsError::ConnectionClosed) => Ok(()),
                    other => other,
                }
            })
    }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.ready = true;
        let res = if self.closing {
            (*self).with_context(Some((ContextWaker::Write, cx)), |s| s.flush())
        } else {
            (*self).with_context(Some((ContextWaker::Write, cx)), |s| s.close(None))
        };
        match res {
            Ok(()) => Poll::Ready(Ok(())),
            Err(WsError::ConnectionClosed) => Poll::Ready(Ok(())),
            Err(WsError::Io(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {
                trace!("WouldBlock");
                self.closing = true;
                Poll::Pending
            }
            Err(err) => {
                debug!("websocket close error: {}", err);
                Poll::Ready(Err(err))
            }
        }
    }
}
#[inline]
fn domain(request: &crate::tungstenite::handshake::client::Request) -> Result<String, WsError> {
    match request.uri().host() {
        Some(d) if d.starts_with('[') && d.ends_with(']') => Ok(d[1..d.len() - 1].to_string()),
        Some(d) => Ok(d.to_string()),
        None => Err(WsError::Url(
            crate::tungstenite::error::UrlError::NoHostName,
        )),
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/tokio_tungstenite.rs"]
mod tests;
