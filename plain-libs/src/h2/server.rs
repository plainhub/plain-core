use crate::h2::codec::{Codec, UserError};
use crate::h2::frame::{self, Pseudo, PushPromiseHeaderError, Reason, Settings, StreamId};
use crate::h2::proto::{self, Config, Error, Prioritized};
use crate::h2::{FlowControl, PingPong, RecvStream, SendStream};
use crate::http::{HeaderMap, Method, Request, Response};
use bytes::{Buf, Bytes};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use std::{fmt, io};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tracing::instrument::{Instrument, Instrumented};
#[must_use = "futures do nothing unless polled"]
pub struct Handshake<T, B: Buf = Bytes> {
    builder: Builder,
    state: Handshaking<T, B>,
    span: tracing::Span,
}
#[must_use = "streams do nothing unless polled"]
pub struct Connection<T, B: Buf> {
    connection: proto::Connection<T, Peer, B>,
}
#[derive(Clone, Debug)]
pub struct Builder {
    reset_stream_duration: Duration,
    reset_stream_max: usize,
    pending_accept_reset_stream_max: usize,
    settings: Settings,
    initial_target_connection_window_size: Option<u32>,
    max_send_buffer_size: usize,
    local_max_error_reset_streams: Option<usize>,
    data_frame_budget: proto::DataFrameBudget,
}
#[derive(Debug)]
pub struct SendResponse<B: Buf> {
    inner: proto::StreamRef<B>,
}
pub struct SendPushedResponse<B: Buf> {
    inner: SendResponse<B>,
}
impl<B: Buf + fmt::Debug> fmt::Debug for SendPushedResponse<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SendPushedResponse {{ {:?} }}", self.inner)
    }
}
enum Handshaking<T, B: Buf> {
    Flushing(Instrumented<Flush<T, Prioritized<B>>>),
    ReadingPreface(Instrumented<ReadPreface<T, Prioritized<B>>>),
    Done,
}
struct Flush<T, B> {
    codec: Option<Codec<T, B>>,
}
struct ReadPreface<T, B> {
    codec: Option<Codec<T, B>>,
    pos: usize,
}
#[derive(Debug)]
pub(crate) struct Peer;
const PREFACE: [u8; 24] = *b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
pub fn handshake<T>(io: T) -> Handshake<T, Bytes>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    Builder::new().handshake(io)
}
impl<T, B> Connection<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin,
    B: Buf,
{
    fn handshake2(io: T, builder: Builder) -> Handshake<T, B> {
        let span = tracing::trace_span!("server_handshake");
        let entered = span.enter();
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
        let state =
            Handshaking::Flushing(Flush::new(codec).instrument(tracing::trace_span!("flush")));
        drop(entered);
        Handshake {
            builder,
            state,
            span,
        }
    }
    pub async fn accept(
        &mut self,
    ) -> Option<Result<(Request<RecvStream>, SendResponse<B>), crate::h2::Error>> {
        std::future::poll_fn(move |cx| self.poll_accept(cx)).await
    }
    pub fn poll_accept(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<(Request<RecvStream>, SendResponse<B>), crate::h2::Error>>> {
        if self.poll_closed(cx)?.is_ready() {
            return Poll::Ready(None);
        }
        if let Some(inner) = self.connection.next_incoming() {
            tracing::trace!("received incoming");
            let (head, _) = inner.take_request().into_parts();
            let body = RecvStream::new(FlowControl::new(inner.clone_to_opaque()));
            let request = Request::from_parts(head, body);
            let respond = SendResponse { inner };
            return Poll::Ready(Some(Ok((request, respond))));
        }
        Poll::Pending
    }
    pub fn set_target_window_size(&mut self, size: u32) {
        assert!(size <= proto::MAX_WINDOW_SIZE);
        self.connection.set_target_window_size(size);
    }
    pub fn set_initial_window_size(&mut self, size: u32) -> Result<(), crate::h2::Error> {
        assert!(size <= proto::MAX_WINDOW_SIZE);
        self.connection.set_initial_window_size(size)?;
        Ok(())
    }
    pub fn enable_connect_protocol(&mut self) -> Result<(), crate::h2::Error> {
        self.connection.set_enable_connect_protocol()?;
        Ok(())
    }
    pub fn poll_closed(&mut self, cx: &mut Context) -> Poll<Result<(), crate::h2::Error>> {
        self.connection.poll(cx).map_err(Into::into)
    }
    pub fn abrupt_shutdown(&mut self, reason: Reason) {
        self.connection.go_away_from_user(reason);
    }
    pub fn graceful_shutdown(&mut self) {
        self.connection.go_away_gracefully();
    }
    pub fn ping_pong(&mut self) -> Option<PingPong> {
        self.connection.take_user_pings().map(PingPong::new)
    }
    pub fn has_streams(&self) -> bool {
        self.connection.has_streams()
    }
    pub fn max_concurrent_send_streams(&self) -> usize {
        self.connection.max_send_streams()
    }
    pub fn max_concurrent_recv_streams(&self) -> usize {
        self.connection.max_recv_streams()
    }
}
impl<T, B> fmt::Debug for Connection<T, B>
where
    T: fmt::Debug,
    B: fmt::Debug + Buf,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Connection")
            .field("connection", &self.connection)
            .finish()
    }
}
impl Builder {
    pub fn new() -> Builder {
        Builder {
            reset_stream_duration: Duration::from_secs(proto::DEFAULT_RESET_STREAM_SECS),
            reset_stream_max: proto::DEFAULT_RESET_STREAM_MAX,
            pending_accept_reset_stream_max: proto::DEFAULT_REMOTE_RESET_STREAM_MAX,
            settings: Settings::default(),
            initial_target_connection_window_size: None,
            max_send_buffer_size: proto::DEFAULT_MAX_SEND_BUFFER_SIZE,
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
    pub fn header_table_size(&mut self, size: u32) -> &mut Self {
        self.settings.set_header_table_size(Some(size));
        self
    }
    pub fn max_concurrent_streams(&mut self, max: u32) -> &mut Self {
        self.settings.set_max_concurrent_streams(Some(max));
        self
    }
    pub fn max_concurrent_reset_streams(&mut self, max: usize) -> &mut Self {
        self.reset_stream_max = max;
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
    pub fn reset_stream_duration(&mut self, dur: Duration) -> &mut Self {
        self.reset_stream_duration = dur;
        self
    }
    pub fn enable_connect_protocol(&mut self) -> &mut Self {
        self.settings.set_enable_connect_protocol(Some(1));
        self
    }
    pub fn data_frame_budget(&mut self, budget: usize) -> &mut Self {
        self.data_frame_budget = proto::DataFrameBudget::Configured(budget);
        self
    }
    pub fn handshake<T, B>(&self, io: T) -> Handshake<T, B>
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
impl<B: Buf> SendResponse<B> {
    pub fn send_response(
        &mut self,
        response: Response<()>,
        end_of_stream: bool,
    ) -> Result<SendStream<B>, crate::h2::Error> {
        self.inner
            .send_response(response, end_of_stream)
            .map(|_| SendStream::new(self.inner.clone()))
            .map_err(Into::into)
    }

    pub fn send_reset(&mut self, reason: Reason) {
        self.inner.send_reset(reason)
    }
    pub fn poll_reset(&mut self, cx: &mut Context) -> Poll<Result<Reason, crate::h2::Error>> {
        self.inner.poll_reset(cx, proto::PollReset::AwaitingHeaders)
    }
    pub fn stream_id(&self) -> crate::h2::StreamId {
        crate::h2::StreamId::from_internal(self.inner.stream_id())
    }
}
impl<B: Buf> SendPushedResponse<B> {
    pub fn send_response(
        &mut self,
        response: Response<()>,
        end_of_stream: bool,
    ) -> Result<SendStream<B>, crate::h2::Error> {
        self.inner.send_response(response, end_of_stream)
    }
    pub fn send_reset(&mut self, reason: Reason) {
        self.inner.send_reset(reason)
    }
    pub fn poll_reset(&mut self, cx: &mut Context) -> Poll<Result<Reason, crate::h2::Error>> {
        self.inner.poll_reset(cx)
    }
    pub fn stream_id(&self) -> crate::h2::StreamId {
        self.inner.stream_id()
    }
}
impl<T, B: Buf> Flush<T, B> {
    fn new(codec: Codec<T, B>) -> Self {
        Flush { codec: Some(codec) }
    }
}
impl<T, B> Future for Flush<T, B>
where
    T: AsyncWrite + Unpin,
    B: Buf,
{
    type Output = Result<Codec<T, B>, crate::h2::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        ready!(self.codec.as_mut().unwrap().flush(cx)).map_err(crate::h2::Error::from_io)?;
        Poll::Ready(Ok(self.codec.take().unwrap()))
    }
}
impl<T, B: Buf> ReadPreface<T, B> {
    fn new(codec: Codec<T, B>) -> Self {
        ReadPreface {
            codec: Some(codec),
            pos: 0,
        }
    }
    fn inner_mut(&mut self) -> &mut T {
        self.codec.as_mut().unwrap().get_mut()
    }
}
impl<T, B> Future for ReadPreface<T, B>
where
    T: AsyncRead + Unpin,
    B: Buf,
{
    type Output = Result<Codec<T, B>, crate::h2::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut buf = [0; 24];
        let mut rem = PREFACE.len() - self.pos;
        while rem > 0 {
            let mut buf = ReadBuf::new(&mut buf[..rem]);
            ready!(Pin::new(self.inner_mut()).poll_read(cx, &mut buf))
                .map_err(crate::h2::Error::from_io)?;
            let n = buf.filled().len();
            if n == 0 {
                return Poll::Ready(Err(crate::h2::Error::from_io(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed before reading preface",
                ))));
            }
            if &PREFACE[self.pos..self.pos + n] != buf.filled() {
                proto_err ! (conn : "read_preface: invalid preface");
                return Poll::Ready(Err(Error::library_go_away(Reason::PROTOCOL_ERROR).into()));
            }
            self.pos += n;
            rem -= n;
        }
        Poll::Ready(Ok(self.codec.take().unwrap()))
    }
}
impl<T, B: Buf> Future for Handshake<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin,
    B: Buf,
{
    type Output = Result<Connection<T, B>, crate::h2::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let span = self.span.clone();
        let _e = span.enter();
        tracing::trace ! (state = ? self . state);
        loop {
            match &mut self.state {
                Handshaking::Flushing(flush) => {
                    let codec = match Pin::new(flush).poll(cx)? {
                        Poll::Pending => {
                            tracing::trace ! (flush . poll = % "Pending");
                            return Poll::Pending;
                        }
                        Poll::Ready(flushed) => {
                            tracing::trace ! (flush . poll = % "Ready");
                            flushed
                        }
                    };
                    self.state = Handshaking::ReadingPreface(
                        ReadPreface::new(codec).instrument(tracing::trace_span!("read_preface")),
                    );
                }
                Handshaking::ReadingPreface(read) => {
                    let codec = ready!(Pin::new(read).poll(cx)?);
                    self.state = Handshaking::Done;
                    let connection = proto::Connection::new(
                        codec,
                        Config {
                            next_stream_id: 2.into(),
                            initial_max_send_streams: 0,
                            max_send_buffer_size: self.builder.max_send_buffer_size,
                            reset_stream_duration: self.builder.reset_stream_duration,
                            reset_stream_max: self.builder.reset_stream_max,
                            remote_reset_stream_max: self.builder.pending_accept_reset_stream_max,
                            local_error_reset_streams_max: self
                                .builder
                                .local_max_error_reset_streams,
                            settings: self.builder.settings.clone(),
                            data_frame_budget: self
                                .builder
                                .data_frame_budget
                                .resolve(self.builder.initial_target_connection_window_size),
                        },
                    );
                    tracing::trace!("connection established!");
                    let mut c = Connection { connection };
                    if let Some(sz) = self.builder.initial_target_connection_window_size {
                        c.set_target_window_size(sz);
                    }
                    return Poll::Ready(Ok(c));
                }
                Handshaking::Done => {
                    panic!("Handshaking::poll() called again after handshaking was complete")
                }
            }
        }
    }
}
impl<T, B> fmt::Debug for Handshake<T, B>
where
    T: AsyncRead + AsyncWrite + fmt::Debug,
    B: fmt::Debug + Buf,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "server::Handshake")
    }
}
impl Peer {
    pub fn convert_send_message(
        id: StreamId,
        response: Response<()>,
        end_of_stream: bool,
    ) -> frame::Headers {
        use crate::http::response::Parts;
        let (
            Parts {
                status, headers, ..
            },
            _,
        ) = response.into_parts();
        let pseudo = Pseudo::response(status);
        let mut frame = frame::Headers::new(id, pseudo, headers);
        if end_of_stream {
            frame.set_end_stream()
        }
        frame
    }
    pub fn convert_push_message(
        stream_id: StreamId,
        promised_id: StreamId,
        request: Request<()>,
    ) -> Result<frame::PushPromise, UserError> {
        use crate::http::request::Parts;
        if let Err(e) = frame::PushPromise::validate_request(&request) {
            use PushPromiseHeaderError::*;
            match e {
                NotSafeAndCacheable => tracing::debug!(
                    ?promised_id,
                    "convert_push_message: method {} is not safe and cacheable",
                    request.method(),
                ),
                InvalidContentLength(e) => tracing::debug!(
                    ?promised_id,
                    "convert_push_message; promised request has invalid content-length {:?}",
                    e,
                ),
            }
            return Err(UserError::MalformedHeaders);
        }
        let (
            Parts {
                method,
                uri,
                headers,
                ..
            },
            _,
        ) = request.into_parts();
        let pseudo = Pseudo::request(method, uri, None);
        Ok(frame::PushPromise::new(
            stream_id,
            promised_id,
            pseudo,
            headers,
        ))
    }
}
impl proto::Peer for Peer {
    type Poll = Request<()>;
    const NAME: &'static str = "Server";
    fn r#dyn() -> proto::DynPeer {
        proto::DynPeer::Server
    }
    fn convert_poll_message(
        pseudo: Pseudo,
        fields: HeaderMap,
        stream_id: StreamId,
    ) -> Result<Self::Poll, Error> {
        use crate::http::{Version, uri};
        let mut b = Request::builder();
        macro_rules! malformed { ($($arg : tt) *) => { { tracing::debug ! ($($arg) *) ; return Err (Error::library_reset (stream_id , Reason::PROTOCOL_ERROR)) ; } } }
        b = b.version(Version::HTTP_2);
        let is_connect;
        if let Some(method) = pseudo.method {
            is_connect = method == Method::CONNECT;
            b = b.method(method);
        } else {
            malformed!("malformed headers: missing method");
        }
        let has_protocol = pseudo.protocol.is_some();
        if has_protocol {
            if is_connect {
                b = b.extension::<crate::h2::ext::Protocol>(pseudo.protocol.unwrap());
            } else {
                malformed!("malformed headers: :protocol on non-CONNECT request");
            }
        }
        if pseudo.status.is_some() {
            malformed!("malformed headers: :status field on request");
        }
        let mut parts = uri::Parts::default();
        if let Some(authority) = pseudo.authority {
            let maybe_authority = uri::Authority::from_maybe_shared(authority.clone().into_inner());
            parts.authority = Some(maybe_authority.or_else(|why| {
                malformed!(
                    "malformed headers: malformed authority ({:?}): {}",
                    authority,
                    why,
                )
            })?);
        }
        if let Some(scheme) = pseudo.scheme {
            if is_connect && !has_protocol {
                malformed!("malformed headers: :scheme in CONNECT");
            }
            let maybe_scheme = scheme.parse();
            let scheme = maybe_scheme.or_else(|why| {
                malformed!(
                    "malformed headers: malformed scheme ({:?}): {}",
                    scheme,
                    why,
                )
            })?;
            if parts.authority.is_some() {
                parts.scheme = Some(scheme);
            }
        } else if !is_connect || has_protocol {
            malformed!("malformed headers: missing scheme");
        }
        if let Some(path) = pseudo.path {
            if is_connect && !has_protocol {
                malformed!("malformed headers: :path in CONNECT");
            }
            if path.is_empty() {
                malformed!("malformed headers: missing path");
            }
            let maybe_path = uri::PathAndQuery::from_maybe_shared(path.clone().into_inner());
            parts.path_and_query = Some(maybe_path.or_else(|why| {
                malformed!("malformed headers: malformed path ({:?}): {}", path, why,)
            })?);
        } else if is_connect && has_protocol {
            malformed!("malformed headers: missing path in extended CONNECT");
        }
        b = b.uri(parts);
        let mut request = match b.body(()) {
            Ok(request) => request,
            Err(e) => {
                proto_err ! (stream : "error building request: {}; stream={:?}" , e , stream_id);
                return Err(Error::library_reset(stream_id, Reason::PROTOCOL_ERROR));
            }
        };
        *request.headers_mut() = fields;
        Ok(request)
    }
}
impl<T, B> fmt::Debug for Handshaking<T, B>
where
    B: Buf,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            Handshaking::Flushing(_) => f.write_str("Flushing(_)"),
            Handshaking::ReadingPreface(_) => f.write_str("ReadingPreface(_)"),
            Handshaking::Done => f.write_str("Done"),
        }
    }
}
