use crate::h2::codec::{Codec, SendError, UserError};
use crate::h2::ext::Protocol;
use crate::h2::frame::{Headers, Pseudo, Reason, Settings, StreamId};
use crate::h2::proto::{self, Error};
use crate::h2::{FlowControl, PingPong, RecvStream, SendStream};
use crate::http::{HeaderMap, Method, Request, Response, Version, uri};
use bytes::{Buf, Bytes};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tracing::Instrument;
pub struct SendRequest<B: Buf> {
    inner: proto::Streams<B, Peer>,
    pending: Option<proto::OpaqueStreamRef>,
}
#[derive(Debug)]
pub struct ReadySendRequest<B: Buf> {
    inner: Option<SendRequest<B>>,
}
#[must_use = "futures do nothing unless polled"]
pub struct Connection<T, B: Buf = Bytes> {
    inner: proto::Connection<T, Peer, B>,
}
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct ResponseFuture {
    inner: proto::OpaqueStreamRef,
    push_promise_consumed: bool,
}
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct PushedResponseFuture {
    inner: ResponseFuture,
}
#[derive(Debug)]
pub struct PushPromise {
    request: Request<()>,
    response: PushedResponseFuture,
}
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct PushPromises {
    inner: proto::OpaqueStreamRef,
}
#[derive(Clone, Debug)]
pub struct Builder {
    reset_stream_duration: Duration,
    initial_max_send_streams: usize,
    initial_target_connection_window_size: Option<u32>,
    max_send_buffer_size: usize,
    reset_stream_max: usize,
    pending_accept_reset_stream_max: usize,
    settings: Settings,
    stream_id: StreamId,
    local_max_error_reset_streams: Option<usize>,
    data_frame_budget: proto::DataFrameBudget,
}
#[derive(Debug)]
pub(crate) struct Peer;
impl<B> SendRequest<B>
where
    B: Buf,
{
    pub fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), crate::h2::Error>> {
        ready!(self.inner.poll_pending_open(cx, self.pending.as_ref()))?;
        self.pending = None;
        Poll::Ready(Ok(()))
    }
    pub fn ready(self) -> ReadySendRequest<B> {
        ReadySendRequest { inner: Some(self) }
    }
    pub fn send_request(
        &mut self,
        request: Request<()>,
        end_of_stream: bool,
    ) -> Result<(ResponseFuture, SendStream<B>), crate::h2::Error> {
        self.inner
            .send_request(request, end_of_stream, self.pending.as_ref())
            .map_err(Into::into)
            .map(|(stream, is_full)| {
                if stream.is_pending_open() && is_full {
                    self.pending = Some(stream.clone_to_opaque());
                }
                let response = ResponseFuture {
                    inner: stream.clone_to_opaque(),
                    push_promise_consumed: false,
                };
                let stream = SendStream::new(stream);
                (response, stream)
            })
    }
    pub fn is_extended_connect_protocol_enabled(&self) -> bool {
        self.inner.is_extended_connect_protocol_enabled()
    }
    pub fn current_max_send_streams(&self) -> usize {
        self.inner.current_max_send_streams()
    }
    pub fn current_max_recv_streams(&self) -> usize {
        self.inner.current_max_recv_streams()
    }
}
impl<B> fmt::Debug for SendRequest<B>
where
    B: Buf,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SendRequest").finish()
    }
}
impl<B> Clone for SendRequest<B>
where
    B: Buf,
{
    fn clone(&self) -> Self {
        SendRequest {
            inner: self.inner.clone(),
            pending: None,
        }
    }
}
impl<B> Future for ReadySendRequest<B>
where
    B: Buf,
{
    type Output = Result<SendRequest<B>, crate::h2::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut self.inner {
            Some(send_request) => {
                ready!(send_request.poll_ready(cx))?;
            }
            None => panic!("called `poll` after future completed"),
        }
        Poll::Ready(Ok(self.inner.take().unwrap()))
    }
}
impl Builder {
    pub fn new() -> Builder {
        Builder {
            max_send_buffer_size: proto::DEFAULT_MAX_SEND_BUFFER_SIZE,
            reset_stream_duration: Duration::from_secs(proto::DEFAULT_RESET_STREAM_SECS),
            reset_stream_max: proto::DEFAULT_RESET_STREAM_MAX,
            pending_accept_reset_stream_max: proto::DEFAULT_REMOTE_RESET_STREAM_MAX,
            initial_target_connection_window_size: None,
            initial_max_send_streams: usize::MAX,
            settings: Default::default(),
            stream_id: 1.into(),
            local_max_error_reset_streams: Some(proto::DEFAULT_LOCAL_RESET_COUNT_MAX),
            data_frame_budget: proto::DataFrameBudget::Auto,
        }
    }
    pub fn initial_window_size(&mut self, size: u32) -> &mut Self {
        self.settings.set_initial_window_size(Some(size));
        self
    }
    pub fn initial_connection_window_size(&mut self, size: u32) -> &mut Self {
        self.initial_target_connection_window_size = Some(size);
        self
    }
    pub fn max_frame_size(&mut self, max: u32) -> &mut Self {
        self.settings.set_max_frame_size(Some(max));
        self
    }
    pub fn max_header_list_size(&mut self, max: u32) -> &mut Self {
        self.settings.set_max_header_list_size(Some(max));
        self
    }
    pub fn max_concurrent_streams(&mut self, max: u32) -> &mut Self {
        self.settings.set_max_concurrent_streams(Some(max));
        self
    }
    pub fn initial_max_send_streams(&mut self, initial: usize) -> &mut Self {
        self.initial_max_send_streams = initial;
        self
    }
    pub fn max_concurrent_reset_streams(&mut self, max: usize) -> &mut Self {
        self.reset_stream_max = max;
        self
    }
    pub fn reset_stream_duration(&mut self, dur: Duration) -> &mut Self {
        self.reset_stream_duration = dur;
        self
    }
    pub fn max_local_error_reset_streams(&mut self, max: Option<usize>) -> &mut Self {
        self.local_max_error_reset_streams = max;
        self
    }
    pub fn max_pending_accept_reset_streams(&mut self, max: usize) -> &mut Self {
        self.pending_accept_reset_stream_max = max;
        self
    }
    pub fn max_send_buffer_size(&mut self, max: usize) -> &mut Self {
        assert!(max <= u32::MAX as usize);
        self.max_send_buffer_size = max;
        self
    }
    pub fn enable_push(&mut self, enabled: bool) -> &mut Self {
        self.settings.set_enable_push(enabled);
        self
    }
    pub fn header_table_size(&mut self, size: u32) -> &mut Self {
        self.settings.set_header_table_size(Some(size));
        self
    }
    pub fn data_frame_budget(&mut self, budget: usize) -> &mut Self {
        self.data_frame_budget = proto::DataFrameBudget::Configured(budget);
        self
    }
    pub fn handshake<T, B>(
        &self,
        io: T,
    ) -> impl Future<Output = Result<(SendRequest<B>, Connection<T, B>), crate::h2::Error>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
        B: Buf,
    {
        Connection::handshake2(io, self.clone())
    }
}
impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}
pub async fn handshake<T>(
    io: T,
) -> Result<(SendRequest<Bytes>, Connection<T, Bytes>), crate::h2::Error>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let builder = Builder::new();
    builder
        .handshake(io)
        .instrument(tracing::trace_span!("client_handshake"))
        .await
}
async fn bind_connection<T>(io: &mut T) -> Result<(), crate::h2::Error>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    tracing::debug!("binding client connection");
    let msg: &'static [u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
    io.write_all(msg).await.map_err(crate::h2::Error::from_io)?;
    tracing::debug!("client connection bound");
    Ok(())
}
impl<T, B> Connection<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin,
    B: Buf,
{
    async fn handshake2(
        mut io: T,
        builder: Builder,
    ) -> Result<(SendRequest<B>, Connection<T, B>), crate::h2::Error> {
        bind_connection(&mut io).await?;
        let mut codec = Codec::new(io);
        if let Some(max) = builder.settings.max_frame_size() {
            codec.set_max_recv_frame_size(max as usize);
        }
        if let Some(max) = builder.settings.max_header_list_size() {
            codec.set_max_recv_header_list_size(max as usize);
        }
        codec
            .buffer(builder.settings.clone().into())
            .expect("invalid SETTINGS frame");
        let inner = proto::Connection::new(
            codec,
            proto::Config {
                next_stream_id: builder.stream_id,
                initial_max_send_streams: builder.initial_max_send_streams,
                max_send_buffer_size: builder.max_send_buffer_size,
                reset_stream_duration: builder.reset_stream_duration,
                reset_stream_max: builder.reset_stream_max,
                remote_reset_stream_max: builder.pending_accept_reset_stream_max,
                local_error_reset_streams_max: builder.local_max_error_reset_streams,
                settings: builder.settings,
                data_frame_budget: builder
                    .data_frame_budget
                    .resolve(builder.initial_target_connection_window_size),
            },
        );
        let send_request = SendRequest {
            inner: inner.streams().clone(),
            pending: None,
        };
        let mut connection = Connection { inner };
        if let Some(sz) = builder.initial_target_connection_window_size {
            connection.set_target_window_size(sz);
        }
        Ok((send_request, connection))
    }
    pub fn set_target_window_size(&mut self, size: u32) {
        assert!(size <= proto::MAX_WINDOW_SIZE);
        self.inner.set_target_window_size(size);
    }
    pub fn set_initial_window_size(&mut self, size: u32) -> Result<(), crate::h2::Error> {
        assert!(size <= proto::MAX_WINDOW_SIZE);
        self.inner.set_initial_window_size(size)?;
        Ok(())
    }
    pub fn ping_pong(&mut self) -> Option<PingPong> {
        self.inner.take_user_pings().map(PingPong::new)
    }
    pub fn max_concurrent_send_streams(&self) -> usize {
        self.inner.max_send_streams()
    }
    pub fn max_concurrent_recv_streams(&self) -> usize {
        self.inner.max_recv_streams()
    }
}
impl<T, B> Future for Connection<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin,
    B: Buf,
{
    type Output = Result<(), crate::h2::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.maybe_close_connection_if_no_streams();
        let had_streams_or_refs = self.inner.has_streams_or_other_references();
        let result = self.inner.poll(cx).map_err(Into::into);
        if result.is_pending()
            && had_streams_or_refs
            && !self.inner.has_streams_or_other_references()
        {
            tracing::trace!("last stream closed during poll, wake again");
            cx.waker().wake_by_ref();
        }
        result
    }
}
impl<T, B> fmt::Debug for Connection<T, B>
where
    T: AsyncRead + AsyncWrite,
    T: fmt::Debug,
    B: fmt::Debug + Buf,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, fmt)
    }
}
impl Future for ResponseFuture {
    type Output = Result<Response<RecvStream>, crate::h2::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (parts, _) = ready!(self.inner.poll_response(cx))?.into_parts();
        let body = RecvStream::new(FlowControl::new(self.inner.clone()));
        Poll::Ready(Ok(Response::from_parts(parts, body)))
    }
}
impl ResponseFuture {
    pub fn stream_id(&self) -> crate::h2::StreamId {
        crate::h2::StreamId::from_internal(self.inner.stream_id())
    }
    pub fn poll_informational(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Response<()>, crate::h2::Error>>> {
        self.inner.poll_informational(cx).map_err(Into::into)
    }
}
impl PushPromises {
    pub fn poll_push_promise(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<PushPromise, crate::h2::Error>>> {
        match self.inner.poll_pushed(cx) {
            Poll::Ready(Some(Ok((request, response)))) => {
                let response = PushedResponseFuture {
                    inner: ResponseFuture {
                        inner: response,
                        push_promise_consumed: false,
                    },
                };
                Poll::Ready(Some(Ok(PushPromise { request, response })))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e.into()))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
impl PushPromise {
    pub fn request(&self) -> &Request<()> {
        &self.request
    }

    pub fn into_parts(self) -> (Request<()>, PushedResponseFuture) {
        (self.request, self.response)
    }
}
impl Future for PushedResponseFuture {
    type Output = Result<Response<RecvStream>, crate::h2::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner).poll(cx)
    }
}
impl PushedResponseFuture {
    pub fn stream_id(&self) -> crate::h2::StreamId {
        self.inner.stream_id()
    }
}
impl Peer {
    pub fn convert_send_message(
        id: StreamId,
        request: Request<()>,
        protocol: Option<Protocol>,
        end_of_stream: bool,
    ) -> Result<Headers, SendError> {
        use crate::http::request::Parts;
        let (
            Parts {
                method,
                uri,
                headers,
                version,
                ..
            },
            _,
        ) = request.into_parts();
        let is_connect = method == Method::CONNECT;
        let mut pseudo = Pseudo::request(method, uri, protocol);
        if pseudo.scheme.is_none() {
            if pseudo.authority.is_none() {
                if version == Version::HTTP_2 {
                    return Err(UserError::MissingUriSchemeAndAuthority.into());
                } else {
                    pseudo.set_scheme(uri::Scheme::HTTP);
                }
            } else if !is_connect {
            }
        }
        let mut frame = Headers::new(id, pseudo, headers);
        if end_of_stream {
            frame.set_end_stream()
        }
        Ok(frame)
    }
}
impl proto::Peer for Peer {
    type Poll = Response<()>;
    const NAME: &'static str = "Client";
    fn r#dyn() -> proto::DynPeer {
        proto::DynPeer::Client
    }
    fn convert_poll_message(
        pseudo: Pseudo,
        fields: HeaderMap,
        stream_id: StreamId,
    ) -> Result<Self::Poll, Error> {
        let mut b = Response::builder();
        b = b.version(Version::HTTP_2);
        if let Some(status) = pseudo.status {
            b = b.status(status);
        }
        let mut response = match b.body(()) {
            Ok(response) => response,
            Err(_) => {
                return Err(Error::library_reset(stream_id, Reason::PROTOCOL_ERROR));
            }
        };
        *response.headers_mut() = fields;
        Ok(response)
    }
}
