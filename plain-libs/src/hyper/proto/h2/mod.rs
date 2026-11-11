use crate::h2::SendStream;
use crate::http::HeaderMap;
use crate::http::header::{CONNECTION, HeaderName, TRANSFER_ENCODING, UPGRADE};
use crate::hyper::body::Body;
use bytes::Buf;
use futures_core::ready;
use pin_project_lite::pin_project;
use std::error::Error as StdError;
use std::future::Future;
use std::io::{Cursor, IoSlice};
use std::pin::Pin;
use std::task::{Context, Poll};
pub(crate) mod ping;
pub(crate) mod upgrade;
cfg_client! { pub (crate) mod client ; pub (crate) use self::client::ClientTask ; }
cfg_server! { pub (crate) mod server ; pub (crate) use self::server::Server ; }
pub(crate) const SPEC_WINDOW_SIZE: u32 = 65_535;
static CONNECTION_HEADERS: [HeaderName; 4] = [
    HeaderName::from_static("keep-alive"),
    HeaderName::from_static("proxy-connection"),
    TRANSFER_ENCODING,
    UPGRADE,
];
enum MessageKind {
    Request,
    Response,
}
fn strip_connection_headers(headers: &mut HeaderMap, kind: MessageKind) {
    for header in &CONNECTION_HEADERS {
        if headers.remove(header).is_some() {
            warn!("Connection header illegal in HTTP/2: {}", header.as_str());
        }
    }
    if matches!(kind, MessageKind::Request) {
        if headers
            .get(crate::http::header::TE)
            .map_or(false, |te_header| te_header != "trailers")
        {
            warn!("TE headers not set to \"trailers\" are illegal in HTTP/2 requests");
            headers.remove(crate::http::header::TE);
        }
    } else if headers.remove(crate::http::header::TE).is_some() {
        warn!("TE headers illegal in HTTP/2 responses");
    }
    if let Some(header) = headers.remove(CONNECTION) {
        warn!(
            "Connection header illegal in HTTP/2: {}",
            CONNECTION.as_str()
        );
        if let Ok(header_contents) = header.to_str() {
            for name in header_contents.split(',') {
                let name = name.trim();
                headers.remove(name);
            }
        }
    }
}
pin_project! { pub (crate) struct PipeToSendStream < S > where S : Body , { body_tx : SendStream < SendBuf < S::Data >>, data_done : bool , buffered_data : Option < Peeked < S::Data >>, #[pin] stream : S , } }
struct Peeked<D> {
    data: D,
    is_eos: bool,
}
impl<S> PipeToSendStream<S>
where
    S: Body,
{
    fn new(stream: S, tx: SendStream<SendBuf<S::Data>>) -> PipeToSendStream<S> {
        PipeToSendStream {
            body_tx: tx,
            data_done: false,
            buffered_data: None,
            stream,
        }
    }
    fn send_reset(self: Pin<&mut Self>, reason: crate::h2::Reason) {
        self.project().body_tx.send_reset(reason);
    }
}
impl<S> Future for PipeToSendStream<S>
where
    S: Body,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Output = crate::hyper::Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut me = self.project();
        loop {
            if let Poll::Ready(reason) = me
                .body_tx
                .poll_reset(cx)
                .map_err(crate::hyper::Error::new_body_write)?
            {
                debug!("stream received RST_STREAM: {:?}", reason);
                return Poll::Ready(Err(crate::hyper::Error::new_body_write(
                    crate::h2::Error::from(reason),
                )));
            }
            if me.buffered_data.is_some() {
                while me.body_tx.capacity() == 0 {
                    match ready!(me.body_tx.poll_capacity(cx)) {
                        Some(Ok(0)) => {}
                        Some(Ok(_)) => break,
                        Some(Err(e)) => {
                            return Poll::Ready(Err(crate::hyper::Error::new_body_write(e)));
                        }
                        None => {
                            return Poll::Ready(Err(crate::hyper::Error::new_body_write(
                                "send stream capacity unexpectedly closed",
                            )));
                        }
                    }
                }
                let peeked = me.buffered_data.take().expect("checked is_some above");
                let buf = SendBuf::Buf(peeked.data);
                me.body_tx
                    .send_data(buf, peeked.is_eos)
                    .map_err(crate::hyper::Error::new_body_write)?;
                if peeked.is_eos {
                    return Poll::Ready(Ok(()));
                }
                continue;
            }
            match ready!(me.stream.as_mut().poll_frame(cx)) {
                Some(Ok(frame)) => {
                    if frame.is_data() {
                        let chunk = frame.into_data().unwrap_or_else(|_| unreachable!());
                        let is_eos = me.stream.is_end_stream();
                        let len = chunk.remaining();
                        trace!("send body chunk: {} bytes, eos={}", len, is_eos);
                        if len == 0 {
                            let buf = SendBuf::Buf(chunk);
                            me.body_tx
                                .send_data(buf, is_eos)
                                .map_err(crate::hyper::Error::new_body_write)?;
                            if is_eos {
                                return Poll::Ready(Ok(()));
                            }
                            continue;
                        }
                        me.body_tx.reserve_capacity(1);
                        *me.buffered_data = Some(Peeked {
                            data: chunk,
                            is_eos,
                        });
                    } else if frame.is_trailers() {
                        me.body_tx.reserve_capacity(0);
                        me.body_tx
                            .send_trailers(frame.into_trailers().unwrap_or_else(|_| unreachable!()))
                            .map_err(crate::hyper::Error::new_body_write)?;
                        return Poll::Ready(Ok(()));
                    } else {
                        trace!("discarding unknown frame");
                    }
                }
                Some(Err(e)) => return Poll::Ready(Err(me.body_tx.on_user_err(e))),
                None => {
                    return Poll::Ready(me.body_tx.send_eos_frame());
                }
            }
        }
    }
}
trait SendStreamExt {
    fn on_user_err<E>(&mut self, err: E) -> crate::hyper::Error
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>;
    fn send_eos_frame(&mut self) -> crate::hyper::Result<()>;
}
impl<B: Buf> SendStreamExt for SendStream<SendBuf<B>> {
    fn on_user_err<E>(&mut self, err: E) -> crate::hyper::Error
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let err = crate::hyper::Error::new_user_body(err);
        debug!("send body user stream error: {}", err);
        self.send_reset(err.h2_reason());
        err
    }
    fn send_eos_frame(&mut self) -> crate::hyper::Result<()> {
        trace!("send body eos");
        self.send_data(SendBuf::None, true)
            .map_err(crate::hyper::Error::new_body_write)
    }
}
#[repr(usize)]
enum SendBuf<B> {
    Buf(B),
    Cursor(Cursor<Box<[u8]>>),
    None,
}
impl<B: Buf> Buf for SendBuf<B> {
    #[inline]
    fn remaining(&self) -> usize {
        match self {
            Self::Buf(b) => b.remaining(),
            Self::Cursor(c) => Buf::remaining(c),
            Self::None => 0,
        }
    }
    #[inline]
    fn chunk(&self) -> &[u8] {
        match self {
            Self::Buf(b) => b.chunk(),
            Self::Cursor(c) => c.chunk(),
            Self::None => &[],
        }
    }
    #[inline]
    fn advance(&mut self, cnt: usize) {
        match self {
            Self::Buf(b) => b.advance(cnt),
            Self::Cursor(c) => c.advance(cnt),
            Self::None => {}
        }
    }
    fn chunks_vectored<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
        match self {
            Self::Buf(b) => b.chunks_vectored(dst),
            Self::Cursor(c) => c.chunks_vectored(dst),
            Self::None => 0,
        }
    }
}
