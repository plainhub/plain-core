use super::DecodedLength;
use crate::http::HeaderMap;
use crate::http_body::{Body, Frame, SizeHint};
use crate::hyper::common::watch;
use crate::hyper::proto::h2::ping;
use bytes::Bytes;
use futures_channel::{mpsc, oneshot};
use futures_core::ready;
use futures_core::{Stream, stream::FusedStream};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
type BodySender = mpsc::Sender<Result<Bytes, crate::hyper::Error>>;
type TrailersSender = oneshot::Sender<HeaderMap>;
#[must_use = "streams do nothing unless polled"]
pub struct Incoming {
    kind: Kind,
}
enum Kind {
    Empty,
    Chan {
        content_length: DecodedLength,
        want_tx: watch::Sender,
        data_rx: mpsc::Receiver<Result<Bytes, crate::hyper::Error>>,
        trailers_rx: oneshot::Receiver<HeaderMap>,
    },
    H2 {
        content_length: DecodedLength,
        data_done: bool,
        ping: ping::Recorder,
        recv: crate::h2::RecvStream,
    },
}
#[must_use = "Sender does nothing unless sent on"]
pub(crate) struct Sender {
    want_rx: watch::Receiver,
    data_tx: BodySender,
    trailers_tx: Option<TrailersSender>,
}
const WANT_PENDING: usize = 1;
const WANT_READY: usize = 2;
impl Incoming {
    pub(crate) fn new_channel(content_length: DecodedLength, wanter: bool) -> (Sender, Incoming) {
        let (data_tx, data_rx) = mpsc::channel(0);
        let (trailers_tx, trailers_rx) = oneshot::channel();
        let want = if wanter { WANT_PENDING } else { WANT_READY };
        let (want_tx, want_rx) = watch::channel(want);
        let tx = Sender {
            want_rx,
            data_tx,
            trailers_tx: Some(trailers_tx),
        };
        let rx = Incoming::new(Kind::Chan {
            content_length,
            want_tx,
            data_rx,
            trailers_rx,
        });
        (tx, rx)
    }
    fn new(kind: Kind) -> Incoming {
        Incoming { kind }
    }
    pub(crate) fn empty() -> Incoming {
        Incoming::new(Kind::Empty)
    }
    pub(crate) fn h2(
        recv: crate::h2::RecvStream,
        mut content_length: DecodedLength,
        ping: ping::Recorder,
    ) -> Self {
        if !content_length.is_exact() && recv.is_end_stream() {
            content_length = DecodedLength::ZERO;
        }
        Incoming::new(Kind::H2 {
            data_done: false,
            ping,
            content_length,
            recv,
        })
    }
}
impl Body for Incoming {
    type Data = Bytes;
    type Error = crate::hyper::Error;
    fn poll_frame(
        #[cfg_attr(not(all(any(all(), all()), any(all(), all()))), allow(unused_mut))]
        mut self: Pin<&mut Self>,
        #[cfg_attr(
            not(all(any(all(), all()), any(all(), all()))),
            allow(unused_variables)
        )]
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match &mut self.kind {
            Kind::Empty => Poll::Ready(None),
            Kind::Chan {
                content_length: len,
                data_rx,
                want_tx,
                trailers_rx,
            } => {
                want_tx.send(WANT_READY);
                if !data_rx.is_terminated() {
                    if let Some(chunk) = ready!(Pin::new(data_rx).poll_next(cx)?) {
                        len.sub_if(chunk.len() as u64);
                        return Poll::Ready(Some(Ok(Frame::data(chunk))));
                    }
                }
                match ready!(Pin::new(trailers_rx).poll(cx)) {
                    Ok(t) => Poll::Ready(Some(Ok(Frame::trailers(t)))),
                    Err(_) => Poll::Ready(None),
                }
            }
            Kind::H2 {
                data_done,
                ping,
                recv: h2,
                content_length: len,
            } => {
                if !*data_done {
                    match ready!(h2.poll_data(cx)) {
                        Some(Ok(bytes)) => {
                            let _ = h2.flow_control().release_capacity(bytes.len());
                            len.sub_if(bytes.len() as u64);
                            ping.record_data(bytes.len());
                            return Poll::Ready(Some(Ok(Frame::data(bytes))));
                        }
                        Some(Err(e)) => {
                            if let Some(crate::h2::Reason::NO_ERROR) = e.reason() {
                                return Poll::Ready(None);
                            } else {
                                return Poll::Ready(Some(Err(crate::hyper::Error::new_body(e))));
                            }
                        }
                        None => {
                            *data_done = true;
                        }
                    }
                }
                match ready!(h2.poll_trailers(cx)) {
                    Ok(t) => {
                        ping.record_non_data();
                        Poll::Ready(Ok(t.map(Frame::trailers)).transpose())
                    }
                    Err(e) => {
                        if let Some(crate::h2::Reason::NO_ERROR) = e.reason() {
                            Poll::Ready(None)
                        } else {
                            Poll::Ready(Some(Err(crate::hyper::Error::new_h2(e))))
                        }
                    }
                }
            }
        }
    }
    fn is_end_stream(&self) -> bool {
        match &self.kind {
            Kind::Empty => true,
            Kind::Chan { content_length, .. } => *content_length == DecodedLength::ZERO,
            Kind::H2 { recv: h2, .. } => h2.is_end_stream(),
        }
    }
    fn size_hint(&self) -> SizeHint {
        fn opt_len(decoded_length: DecodedLength) -> SizeHint {
            if let Some(content_length) = decoded_length.into_opt() {
                SizeHint::with_exact(content_length)
            } else {
                SizeHint::default()
            }
        }
        match self.kind {
            Kind::Empty => SizeHint::with_exact(0),
            Kind::Chan { content_length, .. } => opt_len(content_length),
            Kind::H2 { content_length, .. } => opt_len(content_length),
        }
    }
}
impl fmt::Debug for Incoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct Streaming;
        #[derive(Debug)]
        struct Empty;
        let mut builder = f.debug_tuple("Body");
        match self.kind {
            Kind::Empty => builder.field(&Empty),
            _ => builder.field(&Streaming),
        };
        builder.finish()
    }
}
impl Sender {
    pub(crate) fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<crate::hyper::Result<()>> {
        ready!(self.poll_want(cx)?);
        self.data_tx
            .poll_ready(cx)
            .map_err(|_| crate::hyper::Error::new_closed())
    }
    fn poll_want(&mut self, cx: &mut Context<'_>) -> Poll<crate::hyper::Result<()>> {
        match self.want_rx.load(cx) {
            WANT_READY => Poll::Ready(Ok(())),
            WANT_PENDING => Poll::Pending,
            watch::CLOSED => Poll::Ready(Err(crate::hyper::Error::new_closed())),
            unexpected => unreachable!("want_rx value: {}", unexpected),
        }
    }
    pub(crate) async fn send_trailers(&mut self, trailers: HeaderMap) -> crate::hyper::Result<()> {
        let tx = match self.trailers_tx.take() {
            Some(tx) => tx,
            None => return Err(crate::hyper::Error::new_closed()),
        };
        tx.send(trailers)
            .map_err(|_| crate::hyper::Error::new_closed())
    }
    pub(crate) fn try_send_data(&mut self, chunk: Bytes) -> Result<(), Bytes> {
        self.data_tx
            .try_send(Ok(chunk))
            .map_err(|err| err.into_inner().expect("just sent Ok"))
    }
    pub(crate) fn try_send_trailers(
        &mut self,
        trailers: HeaderMap,
    ) -> Result<(), Option<HeaderMap>> {
        let tx = match self.trailers_tx.take() {
            Some(tx) => tx,
            None => return Err(None),
        };
        tx.send(trailers).map_err(Some)
    }
    pub(crate) fn send_error(&mut self, err: crate::hyper::Error) {
        let _ = self.data_tx.clone().try_send(Err(err));
    }
}
impl fmt::Debug for Sender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct Open;
        #[derive(Debug)]
        struct Closed;
        let mut builder = f.debug_tuple("Sender");
        match self.want_rx.peek() {
            watch::CLOSED => builder.field(&Closed),
            _ => builder.field(&Open),
        };
        builder.finish()
    }
}
