use crate::h2::codec::UserError;
use crate::h2::frame::DEFAULT_INITIAL_WINDOW_SIZE;
use crate::h2::frame::{Reason, StreamId};
use crate::h2::proto::*;
use crate::h2::{client, server};
use bytes::Bytes;
use futures_core::Stream;
use std::io;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::AsyncRead;
#[derive(Debug)]
pub(crate) struct Connection<T, P, B: Buf = Bytes>
where
    P: Peer,
{
    codec: Codec<T, Prioritized<B>>,
    inner: ConnectionInner<P, B>,
}
#[derive(Debug)]
struct ConnectionInner<P, B: Buf = Bytes>
where
    P: Peer,
{
    state: State,
    error: Option<frame::GoAway>,
    go_away: GoAway,
    ping_pong: PingPong,
    settings: Settings,
    streams: Streams<B, P>,
    span: tracing::Span,
    _phantom: PhantomData<P>,
}
struct DynConnection<'a, B: Buf = Bytes> {
    state: &'a mut State,
    go_away: &'a mut GoAway,
    streams: DynStreams<'a, B>,
    error: &'a mut Option<frame::GoAway>,
    ping_pong: &'a mut PingPong,
}
#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub next_stream_id: StreamId,
    pub initial_max_send_streams: usize,
    pub max_send_buffer_size: usize,
    pub reset_stream_duration: Duration,
    pub reset_stream_max: usize,
    pub remote_reset_stream_max: usize,
    pub local_error_reset_streams_max: Option<usize>,
    pub settings: frame::Settings,
    pub data_frame_budget: usize,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum DataFrameBudget {
    Auto,
    Configured(usize),
}
impl DataFrameBudget {
    pub(crate) fn resolve(self, connection_window: Option<WindowSize>) -> usize {
        match self {
            Self::Configured(budget) => budget,
            Self::Auto => {
                let window = connection_window.unwrap_or(DEFAULT_INITIAL_WINDOW_SIZE);
                let budget = window as usize / 2;
                budget.max(DEFAULT_DATA_FRAME_BUDGET)
            }
        }
    }
}
#[derive(Debug)]
enum State {
    Open,
    Closing(Reason, Initiator),
    Closed(Reason, Initiator),
}
impl<T, P, B> Connection<T, P, B>
where
    T: AsyncRead + AsyncWrite + Unpin,
    P: Peer,
    B: Buf,
{
    pub fn new(codec: Codec<T, Prioritized<B>>, config: Config) -> Connection<T, P, B> {
        fn streams_config(config: &Config) -> streams::Config {
            streams::Config {
                initial_max_send_streams: config.initial_max_send_streams,
                local_max_buffer_size: config.max_send_buffer_size,
                local_next_stream_id: config.next_stream_id,
                local_push_enabled: config.settings.is_push_enabled().unwrap_or(true),
                extended_connect_protocol_enabled: config
                    .settings
                    .is_extended_connect_protocol_enabled()
                    .unwrap_or(false),
                local_reset_duration: config.reset_stream_duration,
                local_reset_max: config.reset_stream_max,
                remote_reset_max: config.remote_reset_stream_max,
                remote_init_window_sz: DEFAULT_INITIAL_WINDOW_SIZE,
                remote_max_initiated: config
                    .settings
                    .max_concurrent_streams()
                    .map(|max| max as usize),
                local_max_error_reset_streams: config.local_error_reset_streams_max,
                data_frame_budget: config.data_frame_budget,
            }
        }
        let streams = Streams::new(streams_config(&config));
        let span = tracing::debug_span ! (parent : None , "Connection" , peer = % P::NAME);
        span.follows_from(tracing::Span::current());
        Connection {
            codec,
            inner: ConnectionInner {
                state: State::Open,
                error: None,
                go_away: GoAway::new(),
                ping_pong: PingPong::new(),
                settings: Settings::new(config.settings),
                streams,
                span,
                _phantom: PhantomData,
            },
        }
    }
    pub(crate) fn set_target_window_size(&mut self, size: WindowSize) {
        let _res = self.inner.streams.set_target_connection_window_size(size);
        debug_assert!(_res.is_ok());
    }
    pub(crate) fn set_initial_window_size(&mut self, size: WindowSize) -> Result<(), UserError> {
        let mut settings = frame::Settings::default();
        settings.set_initial_window_size(Some(size));
        self.inner.settings.send_settings(settings)
    }
    pub(crate) fn set_enable_connect_protocol(&mut self) -> Result<(), UserError> {
        let mut settings = frame::Settings::default();
        settings.set_enable_connect_protocol(Some(1));
        self.inner.settings.send_settings(settings)
    }
    pub(crate) fn max_send_streams(&self) -> usize {
        self.inner.streams.max_send_streams()
    }
    pub(crate) fn max_recv_streams(&self) -> usize {
        self.inner.streams.max_recv_streams()
    }
    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Error>> {
        let _e = self.inner.span.enter();
        let span = tracing::trace_span!("poll_ready");
        let _e = span.enter();
        ready!(self.inner.ping_pong.send_pending_pong(cx, &mut self.codec))?;
        ready!(self.inner.ping_pong.send_pending_ping(cx, &mut self.codec))?;
        ready!(
            self.inner
                .settings
                .poll_send(cx, &mut self.codec, &mut self.inner.streams)
        )?;
        ready!(self.inner.streams.send_pending_refusal(cx, &mut self.codec))?;
        Poll::Ready(Ok(()))
    }
    fn poll_go_away(&mut self, cx: &mut Context) -> Poll<Option<io::Result<Reason>>> {
        self.inner.go_away.send_pending_go_away(cx, &mut self.codec)
    }
    pub fn go_away_from_user(&mut self, e: Reason) {
        self.inner.as_dyn().go_away_from_user(e)
    }
    fn take_error(&mut self, ours: Reason, initiator: Initiator) -> Result<(), Error> {
        let (debug_data, theirs) = self
            .inner
            .error
            .take()
            .as_ref()
            .map_or((Bytes::new(), Reason::NO_ERROR), |frame| {
                (frame.debug_data().clone(), frame.reason())
            });
        match (ours, theirs) {
            (Reason::NO_ERROR, Reason::NO_ERROR) => Ok(()),
            (ours, Reason::NO_ERROR) => Err(Error::GoAway(Bytes::new(), ours, initiator)),
            (_, theirs) => Err(Error::remote_go_away(debug_data, theirs)),
        }
    }
    pub fn maybe_close_connection_if_no_streams(&mut self) {
        if !self.inner.streams.has_streams_or_other_references() {
            self.inner.as_dyn().go_away_now(Reason::NO_ERROR);
        }
    }
    pub fn has_streams(&self) -> bool {
        self.inner.streams.has_streams()
    }
    pub fn has_streams_or_other_references(&self) -> bool {
        self.inner.streams.has_streams_or_other_references()
    }
    pub(crate) fn take_user_pings(&mut self) -> Option<UserPings> {
        self.inner.ping_pong.take_user_pings()
    }
    pub fn poll(&mut self, cx: &mut Context) -> Poll<Result<(), Error>> {
        let span = self.inner.span.clone();
        let _e = span.enter();
        let span = tracing::trace_span!("poll");
        let _e = span.enter();
        loop {
            tracing::trace ! (connection . state = ? self . inner . state);
            match self.inner.state {
                State::Open => {
                    let result = match self.poll2(cx) {
                        Poll::Ready(result) => result,
                        Poll::Pending => {
                            ready!(self.inner.streams.poll_complete(cx, &mut self.codec))?;
                            if (self.inner.error.is_some()
                                || self.inner.go_away.should_close_on_idle())
                                && !self.inner.streams.has_streams()
                            {
                                self.inner.as_dyn().go_away_now(Reason::NO_ERROR);
                                continue;
                            }
                            return Poll::Pending;
                        }
                    };
                    self.inner.as_dyn().handle_poll2_result(result)?
                }
                State::Closing(reason, initiator) => {
                    tracing::trace!("connection closing after flush");
                    ready!(self.codec.shutdown(cx))?;
                    self.inner.state = State::Closed(reason, initiator);
                }
                State::Closed(reason, initiator) => {
                    return Poll::Ready(self.take_error(reason, initiator));
                }
            }
        }
    }
    fn poll2(&mut self, cx: &mut Context) -> Poll<Result<(), Error>> {
        self.clear_expired_reset_streams();
        loop {
            if let Some(reason) = ready!(self.poll_go_away(cx)?) {
                if self.inner.go_away.should_close_now() {
                    if self.inner.go_away.is_user_initiated() {
                        return Poll::Ready(Ok(()));
                    } else {
                        return Poll::Ready(Err(Error::library_go_away(reason)));
                    }
                }
                debug_assert_eq!(
                    reason,
                    Reason::NO_ERROR,
                    "graceful GOAWAY should be NO_ERROR"
                );
            }
            ready!(self.poll_ready(cx))?;
            match self
                .inner
                .as_dyn()
                .recv_frame(ready!(Pin::new(&mut self.codec).poll_next(cx)?))?
            {
                ReceivedFrame::Settings(frame) => {
                    self.inner.settings.recv_settings(
                        frame,
                        &mut self.codec,
                        &mut self.inner.streams,
                    )?;
                }
                ReceivedFrame::Continue => (),
                ReceivedFrame::Done => {
                    return Poll::Ready(Ok(()));
                }
            }
        }
    }
    fn clear_expired_reset_streams(&mut self) {
        self.inner.streams.clear_expired_reset_streams();
    }
}
impl<P, B> ConnectionInner<P, B>
where
    P: Peer,
    B: Buf,
{
    fn as_dyn(&mut self) -> DynConnection<'_, B> {
        let ConnectionInner {
            state,
            go_away,
            streams,
            error,
            ping_pong,
            ..
        } = self;
        let streams = streams.as_dyn();
        DynConnection {
            state,
            go_away,
            streams,
            error,
            ping_pong,
        }
    }
}
impl<B> DynConnection<'_, B>
where
    B: Buf,
{
    fn go_away(&mut self, id: StreamId, e: Reason) {
        let frame = frame::GoAway::new(id, e);
        self.streams.send_go_away(id);
        self.go_away.go_away(frame);
    }
    fn go_away_now(&mut self, e: Reason) {
        let last_processed_id = self.streams.last_processed_id();
        let frame = frame::GoAway::new(last_processed_id, e);
        self.go_away.go_away_now(frame);
    }
    fn go_away_now_data(&mut self, e: Reason, data: Bytes) {
        let last_processed_id = self.streams.last_processed_id();
        let frame = frame::GoAway::with_debug_data(last_processed_id, e, data);
        self.go_away.go_away_now(frame);
    }
    fn go_away_from_user(&mut self, e: Reason) {
        let last_processed_id = self.streams.last_processed_id();
        let frame = frame::GoAway::new(last_processed_id, e);
        self.go_away.go_away_from_user(frame);
        self.streams.handle_error(Error::user_go_away(e));
    }
    fn handle_poll2_result(&mut self, result: Result<(), Error>) -> Result<(), Error> {
        match result {
            Ok(()) => {
                *self.state = State::Closing(Reason::NO_ERROR, Initiator::Library);
                Ok(())
            }
            Err(Error::GoAway(debug_data, reason, initiator)) => {
                self.handle_go_away(reason, debug_data, initiator);
                Ok(())
            }
            Err(Error::Reset(id, reason, initiator)) => {
                if initiator == Initiator::Remote {
                    tracing::trace!(?id, ?reason, ?initiator, "stream reset");
                    return Ok(());
                }
                debug_assert_eq!(initiator, Initiator::Library);
                tracing::trace!(?id, ?reason, ?initiator, "stream error");
                match self.streams.send_reset(id, reason) {
                    Ok(()) => (),
                    Err(crate::h2::proto::error::GoAway { debug_data, reason }) => {
                        self.handle_go_away(reason, debug_data, Initiator::Library);
                    }
                }
                Ok(())
            }
            Err(Error::Io(kind, inner)) => {
                tracing::debug ! (error = ? kind , "Connection::poll; IO error");
                let e = Error::Io(kind, inner);
                self.streams.handle_error(e.clone());
                if self.streams.is_buffer_empty()
                    && matches!(kind, io::ErrorKind::UnexpectedEof)
                    && (self.streams.is_server()
                        || self.error.as_ref().map(|f| f.reason() == Reason::NO_ERROR)
                            == Some(true))
                {
                    *self.state = State::Closed(Reason::NO_ERROR, Initiator::Library);
                    return Ok(());
                }
                Err(e)
            }
        }
    }
    fn handle_go_away(&mut self, reason: Reason, debug_data: Bytes, initiator: Initiator) {
        let e = Error::GoAway(debug_data.clone(), reason, initiator);
        tracing::debug ! (error = ? e , "Connection::poll; connection error");
        if self
            .go_away
            .going_away()
            .map_or(false, |frame| frame.reason() == reason)
        {
            tracing::trace!("    -> already going away");
            *self.state = State::Closing(reason, initiator);
            return;
        }
        self.streams.handle_error(e);
        self.go_away_now_data(reason, debug_data);
    }
    fn recv_frame(&mut self, frame: Option<Frame>) -> Result<ReceivedFrame, Error> {
        use crate::h2::frame::Frame::*;
        match frame {
            Some(Headers(frame)) => {
                tracing::trace!(?frame, "recv HEADERS");
                self.streams.recv_headers(frame)?;
            }
            Some(Data(frame)) => {
                tracing::trace!(?frame, "recv DATA");
                self.streams.recv_data(frame)?;
            }
            Some(Reset(frame)) => {
                tracing::trace!(?frame, "recv RST_STREAM");
                self.streams.recv_reset(frame)?;
            }
            Some(PushPromise(frame)) => {
                tracing::trace!(?frame, "recv PUSH_PROMISE");
                self.streams.recv_push_promise(frame)?;
            }
            Some(Settings(frame)) => {
                tracing::trace!(?frame, "recv SETTINGS");
                return Ok(ReceivedFrame::Settings(frame));
            }
            Some(GoAway(frame)) => {
                tracing::trace!(?frame, "recv GOAWAY");
                self.streams.recv_go_away(&frame)?;
                *self.error = Some(frame);
            }
            Some(Ping(frame)) => {
                tracing::trace!(?frame, "recv PING");
                let status = self.ping_pong.recv_ping(frame);
                if status.is_shutdown() {
                    assert!(
                        self.go_away.is_going_away(),
                        "received unexpected shutdown ping"
                    );
                    let last_processed_id = self.streams.last_processed_id();
                    self.go_away(last_processed_id, Reason::NO_ERROR);
                }
            }
            Some(WindowUpdate(frame)) => {
                tracing::trace!(?frame, "recv WINDOW_UPDATE");
                self.streams.recv_window_update(frame)?;
            }
            Some(Priority(frame)) => {
                tracing::trace!(?frame, "recv PRIORITY");
            }
            None => {
                tracing::trace!("codec closed");
                self.streams.recv_eof(false).expect("mutex poisoned");
                return Ok(ReceivedFrame::Done);
            }
        }
        Ok(ReceivedFrame::Continue)
    }
}
enum ReceivedFrame {
    Settings(frame::Settings),
    Continue,
    Done,
}
impl<T, B> Connection<T, client::Peer, B>
where
    T: AsyncRead + AsyncWrite,
    B: Buf,
{
    pub(crate) fn streams(&self) -> &Streams<B, client::Peer> {
        &self.inner.streams
    }
}
impl<T, B> Connection<T, server::Peer, B>
where
    T: AsyncRead + AsyncWrite + Unpin,
    B: Buf,
{
    pub fn next_incoming(&mut self) -> Option<StreamRef<B>> {
        self.inner.streams.next_incoming()
    }
    pub fn go_away_gracefully(&mut self) {
        if self.inner.go_away.is_going_away() {
            return;
        }
        self.inner.as_dyn().go_away(StreamId::MAX, Reason::NO_ERROR);
        self.inner.ping_pong.ping_shutdown();
    }
}
impl<T, P, B> Drop for Connection<T, P, B>
where
    P: Peer,
    B: Buf,
{
    fn drop(&mut self) {
        let _ = self.inner.streams.recv_eof(true);
    }
}
