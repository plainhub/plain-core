use super::recv::RecvHeaderBlockError;
use super::store::{self, Entry, Resolve, Store};
use super::{Buffer, BufferStatus, Config, Counts, Prioritized, Recv, Send, Stream, StreamId};
use crate::h2::codec::{Codec, SendError, UserError};
use crate::h2::ext::Protocol;
use crate::h2::frame::{self, Frame, Reason};
use crate::h2::proto::{Error, Initiator, Open, Peer, WindowSize, peer};
use crate::h2::{client, proto, server};
use crate::http::{HeaderMap, Request, Response};
use bytes::{Buf, Bytes};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::{fmt, io};
use tokio::io::AsyncWrite;
#[derive(Debug)]
pub(crate) struct Streams<B, P>
where
    P: Peer,
{
    inner: Arc<Mutex<Inner>>,
    send_buffer: Arc<SendBuffer<B>>,
    _p: ::std::marker::PhantomData<P>,
}
#[derive(Debug)]
pub(crate) struct DynStreams<'a, B> {
    inner: &'a Mutex<Inner>,
    send_buffer: &'a SendBuffer<B>,
    peer: peer::Dyn,
}
#[derive(Debug)]
pub(crate) struct StreamRef<B> {
    opaque: OpaqueStreamRef,
    send_buffer: Arc<SendBuffer<B>>,
}
pub(crate) struct OpaqueStreamRef {
    inner: Arc<Mutex<Inner>>,
    key: store::Key,
}
#[derive(Debug)]
struct Inner {
    counts: Counts,
    actions: Actions,
    store: Store,
    refs: usize,
}
#[derive(Debug)]
struct Actions {
    recv: Recv,
    send: Send,
    task: Option<Waker>,
    conn_error: Option<proto::Error>,
}
#[derive(Debug)]
struct SendBuffer<B> {
    inner: Mutex<Buffer<Frame<B>>>,
}
impl<B, P> Streams<B, P>
where
    B: Buf,
    P: Peer,
{
    pub fn new(config: Config) -> Self {
        let peer = P::r#dyn();
        Streams {
            inner: Inner::new(peer, config),
            send_buffer: Arc::new(SendBuffer::new()),
            _p: ::std::marker::PhantomData,
        }
    }
    pub fn set_target_connection_window_size(&mut self, size: WindowSize) -> Result<(), Reason> {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        me.actions
            .recv
            .set_target_connection_window(size, &mut me.actions.task)
    }
    pub fn next_incoming(&mut self) -> Option<StreamRef<B>> {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        me.actions.recv.next_incoming(&mut me.store).map(|key| {
            let stream = &mut me.store.resolve(key);
            tracing::trace!(
                "next_incoming; id={:?}, state={:?}",
                stream.id,
                stream.state
            );
            me.refs += 1;
            if stream.state.is_remote_reset() {
                me.counts.dec_num_remote_reset_streams();
            }
            StreamRef {
                opaque: OpaqueStreamRef::new(self.inner.clone(), stream),
                send_buffer: self.send_buffer.clone(),
            }
        })
    }
    pub fn send_pending_refusal<T>(
        &mut self,
        cx: &mut Context,
        dst: &mut Codec<T, Prioritized<B>>,
    ) -> Poll<io::Result<()>>
    where
        T: AsyncWrite + Unpin,
    {
        loop {
            let status = {
                let mut me = self.inner.lock().unwrap();
                let me = &mut *me;
                me.actions.recv.send_pending_refusal(dst)?
            };
            match status {
                BufferStatus::Complete => return Poll::Ready(Ok(())),
                BufferStatus::CodecFull => ready!(dst.poll_ready(cx))?,
            }
        }
    }
    pub fn clear_expired_reset_streams(&mut self) {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        me.actions
            .recv
            .clear_expired_reset_streams(&mut me.store, &mut me.counts);
    }
    pub fn poll_complete<T>(
        &mut self,
        cx: &mut Context,
        dst: &mut Codec<T, Prioritized<B>>,
    ) -> Poll<io::Result<()>>
    where
        T: AsyncWrite + Unpin,
    {
        loop {
            ready!(dst.poll_ready(cx))?;
            let status = {
                let mut me = self.inner.lock().unwrap();
                let status = me.buffer_pending(&self.send_buffer, dst)?;
                if status == BufferStatus::Complete {
                    me.actions.task = Some(cx.waker().clone());
                }
                status
            };
            match status {
                BufferStatus::Complete => {}
                BufferStatus::CodecFull => continue,
            }
            ready!(dst.flush(cx))?;
            let reclaimed = {
                let mut me = self.inner.lock().unwrap();
                me.reclaim_written_frame(&self.send_buffer, dst)
            };
            if !reclaimed {
                return Poll::Ready(Ok(()));
            }
        }
    }
    pub fn apply_remote_settings(
        &mut self,
        frame: &frame::Settings,
        is_initial: bool,
    ) -> Result<(), Error> {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        let mut send_buffer = self.send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        me.counts.apply_remote_settings(frame, is_initial);
        me.actions.send.apply_remote_settings(
            frame,
            send_buffer,
            &mut me.store,
            &mut me.counts,
            &mut me.actions.task,
        )
    }
    pub fn apply_local_settings(&mut self, frame: &frame::Settings) -> Result<(), Error> {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        me.actions.recv.apply_local_settings(frame, &mut me.store)
    }
    pub fn send_request(
        &mut self,
        mut request: Request<()>,
        end_of_stream: bool,
        pending: Option<&OpaqueStreamRef>,
    ) -> Result<(StreamRef<B>, bool), SendError> {
        use super::stream::ContentLength;
        use crate::http::Method;
        let protocol = request.extensions_mut().remove::<Protocol>();
        request.extensions_mut().clear();
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        let mut send_buffer = self.send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        me.actions.ensure_no_conn_error()?;
        me.actions.send.ensure_next_stream_id()?;
        if let Some(stream) = pending {
            if me.store.resolve(stream.key).is_pending_open {
                return Err(UserError::Rejected.into());
            }
        }
        if me.counts.peer().is_server() {
            return Err(UserError::UnexpectedFrameType.into());
        }
        let stream_id = me.actions.send.open()?;
        let mut stream = Stream::new(
            stream_id,
            me.actions.send.init_window_sz(),
            me.actions.recv.init_window_sz(),
        );
        if *request.method() == Method::HEAD {
            stream.content_length = ContentLength::Head;
        }
        let headers =
            client::Peer::convert_send_message(stream_id, request, protocol, end_of_stream)?;
        let mut stream = me.store.insert(stream.id, stream);
        let sent = me.actions.send.send_headers(
            headers,
            send_buffer,
            &mut stream,
            &mut me.counts,
            &mut me.actions.task,
        );
        if let Err(err) = sent {
            stream.unlink();
            stream.remove();
            return Err(err.into());
        }
        debug_assert!(!stream.state.is_closed());
        me.refs += 1;
        let is_full = me.counts.next_send_stream_will_reach_capacity();
        Ok((
            StreamRef {
                opaque: OpaqueStreamRef::new(self.inner.clone(), &mut stream),
                send_buffer: self.send_buffer.clone(),
            },
            is_full,
        ))
    }
    pub(crate) fn is_extended_connect_protocol_enabled(&self) -> bool {
        self.inner
            .lock()
            .unwrap()
            .actions
            .send
            .is_extended_connect_protocol_enabled()
    }
    pub fn current_max_send_streams(&self) -> usize {
        let me = self.inner.lock().unwrap();
        me.counts.max_send_streams()
    }
    pub fn current_max_recv_streams(&self) -> usize {
        let me = self.inner.lock().unwrap();
        me.counts.max_recv_streams()
    }
}
impl<B> DynStreams<'_, B> {
    pub fn is_buffer_empty(&self) -> bool {
        self.send_buffer.is_empty()
    }
    pub fn is_server(&self) -> bool {
        self.peer.is_server()
    }
    pub fn recv_headers(&mut self, frame: frame::Headers) -> Result<(), Error> {
        let mut me = self.inner.lock().unwrap();
        me.recv_headers(self.peer, self.send_buffer, frame)
    }
    pub fn recv_data(&mut self, frame: frame::Data) -> Result<(), Error> {
        let mut me = self.inner.lock().unwrap();
        me.recv_data(self.peer, self.send_buffer, frame)
    }
    pub fn recv_reset(&mut self, frame: frame::Reset) -> Result<(), Error> {
        let mut me = self.inner.lock().unwrap();
        me.recv_reset(self.send_buffer, frame)
    }
    pub fn handle_error(&mut self, err: proto::Error) -> StreamId {
        let mut me = self.inner.lock().unwrap();
        me.handle_error(self.send_buffer, err)
    }
    pub fn recv_go_away(&mut self, frame: &frame::GoAway) -> Result<(), Error> {
        let mut me = self.inner.lock().unwrap();
        me.recv_go_away(self.send_buffer, frame)
    }
    pub fn last_processed_id(&self) -> StreamId {
        self.inner.lock().unwrap().actions.recv.last_processed_id()
    }
    pub fn recv_window_update(&mut self, frame: frame::WindowUpdate) -> Result<(), Error> {
        let mut me = self.inner.lock().unwrap();
        me.recv_window_update(self.send_buffer, frame)
    }
    pub fn recv_push_promise(&mut self, frame: frame::PushPromise) -> Result<(), Error> {
        let mut me = self.inner.lock().unwrap();
        me.recv_push_promise(self.send_buffer, frame)
    }
    pub fn recv_eof(&mut self, clear_pending_accept: bool) -> Result<(), ()> {
        let mut me = self.inner.lock().map_err(|_| ())?;
        me.recv_eof(self.send_buffer, clear_pending_accept)
    }
    pub fn send_reset(
        &mut self,
        id: StreamId,
        reason: Reason,
    ) -> Result<(), crate::h2::proto::error::GoAway> {
        let mut me = self.inner.lock().unwrap();
        me.send_reset(self.send_buffer, id, reason)
    }
    pub fn send_go_away(&mut self, last_processed_id: StreamId) {
        let mut me = self.inner.lock().unwrap();
        me.actions.recv.go_away(last_processed_id);
    }
}
impl Inner {
    fn new(peer: peer::Dyn, config: Config) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Inner {
            counts: Counts::new(peer, &config),
            actions: Actions {
                recv: Recv::new(peer, &config),
                send: Send::new(&config),
                task: None,
                conn_error: None,
            },
            store: Store::new(),
            refs: 1,
        }))
    }
    fn recv_headers<B>(
        &mut self,
        peer: peer::Dyn,
        send_buffer: &SendBuffer<B>,
        frame: frame::Headers,
    ) -> Result<(), Error> {
        let id = frame.stream_id();
        if id > self.actions.recv.max_stream_id() {
            tracing::trace!(
                "id ({:?}) > max_stream_id ({:?}), ignoring HEADERS",
                id,
                self.actions.recv.max_stream_id()
            );
            return Ok(());
        }
        let key = match self.store.find_entry(id) {
            Entry::Occupied(e) => e.key(),
            Entry::Vacant(e) => {
                if !peer.is_server() {
                    if self.actions.may_have_forgotten_stream(peer, id) {
                        tracing::debug!(
                            "recv_headers for old stream={:?}, sending STREAM_CLOSED",
                            id,
                        );
                        return Err(Error::library_reset(id, Reason::STREAM_CLOSED));
                    }
                }
                match self
                    .actions
                    .recv
                    .open(id, Open::Headers, &mut self.counts)?
                {
                    Some(stream_id) => {
                        let stream = Stream::new(
                            stream_id,
                            self.actions.send.init_window_sz(),
                            self.actions.recv.init_window_sz(),
                        );
                        e.insert(stream)
                    }
                    None => return Ok(()),
                }
            }
        };
        let stream = self.store.resolve(key);
        if stream.is_pending_open {
            proto_err ! (conn : "recv_headers: received frame on idle stream {:?}" , id);
            return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
        }
        if stream.state.is_local_error() {
            tracing::trace!("recv_headers; ignoring trailers on {:?}", stream.id);
            return Ok(());
        }
        let actions = &mut self.actions;
        let mut send_buffer = send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        self . counts . transition (stream , | counts , stream | { tracing::trace ! ("recv_headers; stream={:?}; state={:?}" , stream . id , stream . state) ; let res = if stream . state . is_recv_headers () { match actions . recv . recv_headers (frame , stream , counts) { Ok (()) => Ok (()) , Err (RecvHeaderBlockError::Oversize (resp)) => { if let Some (resp) = resp { let sent = actions . send . send_headers (resp , send_buffer , stream , counts , & mut actions . task) ; debug_assert ! (sent . is_ok () , "oversize response should not fail") ; actions . send . schedule_implicit_reset (stream , Reason::PROTOCOL_ERROR , counts , & mut actions . task) ; actions . recv . enqueue_reset_expiration (stream , counts) ; Ok (()) } else { Err (Error::library_reset (stream . id , Reason::PROTOCOL_ERROR)) } } , Err (RecvHeaderBlockError::State (err)) => Err (err) , } } else { if ! frame . is_end_stream () { proto_err ! (stream : "recv_headers: trailers frame was not EOS; stream={:?}" , stream . id) ; return Err (Error::library_reset (stream . id , Reason::PROTOCOL_ERROR)) ; } actions . recv . recv_trailers (frame , stream) } ; actions . reset_on_recv_stream_err (send_buffer , stream , counts , res) })
    }
    fn recv_data<B>(
        &mut self,
        peer: peer::Dyn,
        send_buffer: &SendBuffer<B>,
        frame: frame::Data,
    ) -> Result<(), Error> {
        let id = frame.stream_id();
        let stream = match self.store.find_mut(&id) {
            Some(stream) => stream,
            None => {
                if id > self.actions.recv.max_stream_id() {
                    tracing::trace!(
                        "id ({:?}) > max_stream_id ({:?}), ignoring DATA",
                        id,
                        self.actions.recv.max_stream_id()
                    );
                    let sz = frame.flow_controlled_len();
                    assert!(sz <= super::MAX_WINDOW_SIZE as usize);
                    let sz = sz as WindowSize;
                    self.actions.recv.ignore_data(sz)?;
                    return Ok(());
                }
                if self.actions.may_have_forgotten_stream(peer, id) {
                    tracing::debug!("recv_data for old stream={:?}, sending STREAM_CLOSED", id,);
                    let sz = frame.flow_controlled_len();
                    assert!(sz <= super::MAX_WINDOW_SIZE as usize);
                    let sz = sz as WindowSize;
                    self.actions.recv.ignore_data(sz)?;
                    return Err(Error::library_reset(id, Reason::STREAM_CLOSED));
                }
                proto_err ! (conn : "recv_data: stream not found; id={:?}" , id);
                return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
            }
        };
        let actions = &mut self.actions;
        let mut send_buffer = send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        self.counts.transition(stream, |counts, stream| {
            let sz = frame.flow_controlled_len();
            let is_end_stream = frame.is_end_stream();
            let payload_len = frame.payload().len();
            let mut res = actions.recv.recv_data(frame, stream);
            if res.is_ok() && !is_end_stream {
                res = counts.record_data_frame(payload_len).map_err(|_| {
                    tracing::debug!("too many small DATA frames");
                    Error::library_go_away_data(Reason::ENHANCE_YOUR_CALM, "too_many_data_frames")
                });
            }
            if let Err(Error::Reset(..)) = res {
                actions
                    .recv
                    .release_connection_capacity(sz as WindowSize, &mut None);
            }
            actions.reset_on_recv_stream_err(send_buffer, stream, counts, res)
        })
    }
    fn recv_reset<B>(
        &mut self,
        send_buffer: &SendBuffer<B>,
        frame: frame::Reset,
    ) -> Result<(), Error> {
        let id = frame.stream_id();
        if id.is_zero() {
            proto_err ! (conn : "recv_reset: invalid stream ID 0");
            return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
        }
        if id > self.actions.recv.max_stream_id() {
            tracing::trace!(
                "id ({:?}) > max_stream_id ({:?}), ignoring RST_STREAM",
                id,
                self.actions.recv.max_stream_id()
            );
            return Ok(());
        }
        let stream = match self.store.find_mut(&id) {
            Some(stream) => stream,
            None => {
                self.actions
                    .ensure_not_idle(self.counts.peer(), id)
                    .map_err(Error::library_go_away)?;
                return Ok(());
            }
        };
        if stream.is_pending_open {
            proto_err ! (conn : "recv_reset: received frame on idle stream {:?}" , id);
            return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
        }
        let mut send_buffer = send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        let actions = &mut self.actions;
        self.counts.transition(stream, |counts, stream| {
            actions.recv.recv_reset(frame, stream, counts)?;
            actions.send.handle_error(send_buffer, stream, counts);
            assert!(stream.state.is_closed());
            Ok(())
        })
    }
    fn recv_window_update<B>(
        &mut self,
        send_buffer: &SendBuffer<B>,
        frame: frame::WindowUpdate,
    ) -> Result<(), Error> {
        let id = frame.stream_id();
        let mut send_buffer = send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        if id.is_zero() {
            self.actions
                .send
                .recv_connection_window_update(frame, &mut self.store, &mut self.counts)
                .map_err(Error::library_go_away)?;
        } else {
            if let Some(mut stream) = self.store.find_mut(&id) {
                if stream.is_pending_open {
                    proto_err ! (conn : "recv_window_update: received frame on idle stream {:?}" , id);
                    return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
                }
                let res = self
                    .actions
                    .send
                    .recv_stream_window_update(
                        frame.size_increment(),
                        send_buffer,
                        &mut stream,
                        &mut self.counts,
                        &mut self.actions.task,
                    )
                    .map_err(|reason| Error::library_reset(id, reason));
                return self.actions.reset_on_recv_stream_err(
                    send_buffer,
                    &mut stream,
                    &mut self.counts,
                    res,
                );
            } else {
                self.actions
                    .ensure_not_idle(self.counts.peer(), id)
                    .map_err(Error::library_go_away)?;
            }
        }
        Ok(())
    }
    fn handle_error<B>(&mut self, send_buffer: &SendBuffer<B>, err: proto::Error) -> StreamId {
        let actions = &mut self.actions;
        let counts = &mut self.counts;
        let mut send_buffer = send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        let last_processed_id = actions.recv.last_processed_id();
        self.store.for_each(|stream| {
            counts.transition(stream, |counts, stream| {
                actions.recv.handle_error(&err, &mut *stream);
                actions.send.handle_error(send_buffer, stream, counts);
            })
        });
        actions.conn_error = Some(err);
        last_processed_id
    }
    fn recv_go_away<B>(
        &mut self,
        send_buffer: &SendBuffer<B>,
        frame: &frame::GoAway,
    ) -> Result<(), Error> {
        let actions = &mut self.actions;
        let counts = &mut self.counts;
        let mut send_buffer = send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        let last_stream_id = frame.last_stream_id();
        actions.send.recv_go_away(last_stream_id)?;
        let err = Error::remote_go_away(frame.debug_data().clone(), frame.reason());
        let peer = counts.peer();
        self.store.for_each(|stream| {
            if stream.id > last_stream_id && peer.is_local_init(stream.id) {
                counts.transition(stream, |counts, stream| {
                    actions.recv.handle_error(&err, &mut *stream);
                    actions.send.handle_error(send_buffer, stream, counts);
                })
            }
        });
        actions.conn_error = Some(err);
        Ok(())
    }
    fn recv_push_promise<B>(
        &mut self,
        send_buffer: &SendBuffer<B>,
        frame: frame::PushPromise,
    ) -> Result<(), Error> {
        let id = frame.stream_id();
        let promised_id = frame.promised_id();
        let parent_key = match self.store.find_mut(&id) {
            Some(stream) => {
                if id > self.actions.recv.max_stream_id() {
                    tracing::trace!(
                        "id ({:?}) > max_stream_id ({:?}), ignoring PUSH_PROMISE",
                        id,
                        self.actions.recv.max_stream_id()
                    );
                    return Ok(());
                }
                if !stream.state.ensure_recv_open()? {
                    proto_err ! (conn : "recv_push_promise: initiating stream is not opened");
                    return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
                }
                stream.key()
            }
            None => {
                proto_err ! (conn : "recv_push_promise: initiating stream is in an invalid state");
                return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
            }
        };
        self.actions.recv.ensure_can_reserve()?;
        if self
            .actions
            .recv
            .open(promised_id, Open::PushPromise, &mut self.counts)?
            .is_none()
        {
            return Ok(());
        }
        let child_key: Option<store::Key> = {
            let stream = self.store.insert(promised_id, {
                Stream::new(
                    promised_id,
                    self.actions.send.init_window_sz(),
                    self.actions.recv.init_window_sz(),
                )
            });
            let actions = &mut self.actions;
            self.counts.transition(stream, |counts, stream| {
                let stream_valid = actions.recv.recv_push_promise(frame, stream);
                match stream_valid {
                    Ok(()) => Ok(Some(stream.key())),
                    _ => {
                        let mut send_buffer = send_buffer.inner.lock().unwrap();
                        actions
                            .reset_on_recv_stream_err(
                                &mut *send_buffer,
                                stream,
                                counts,
                                stream_valid,
                            )
                            .map(|()| None)
                    }
                }
            })?
        };
        if let Some(child) = child_key {
            let mut ppp = self.store[parent_key].pending_push_promises.take();
            ppp.push(&mut self.store.resolve(child));
            let parent = &mut self.store.resolve(parent_key);
            parent.pending_push_promises = ppp;
            parent.notify_push();
        };
        Ok(())
    }
    fn recv_eof<B>(
        &mut self,
        send_buffer: &SendBuffer<B>,
        clear_pending_accept: bool,
    ) -> Result<(), ()> {
        let actions = &mut self.actions;
        let counts = &mut self.counts;
        let mut send_buffer = send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        if actions.conn_error.is_none() {
            actions.conn_error = Some(
                io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "connection closed because of a broken pipe",
                )
                .into(),
            );
        }
        tracing::trace!("Streams::recv_eof");
        self.store.for_each(|stream| {
            counts.transition(stream, |counts, stream| {
                actions.recv.recv_eof(stream);
                actions.send.handle_error(send_buffer, stream, counts);
            })
        });
        actions.clear_queues(clear_pending_accept, &mut self.store, counts);
        Ok(())
    }
    fn buffer_pending<T, B>(
        &mut self,
        send_buffer: &SendBuffer<B>,
        dst: &mut Codec<T, Prioritized<B>>,
    ) -> io::Result<BufferStatus>
    where
        T: AsyncWrite + Unpin,
        B: Buf,
    {
        let mut send_buffer = send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        if self
            .actions
            .recv
            .buffer_pending(&mut self.store, &mut self.counts, dst)?
            == BufferStatus::CodecFull
        {
            return Ok(BufferStatus::CodecFull);
        }
        if self
            .actions
            .send
            .buffer_pending(send_buffer, &mut self.store, &mut self.counts, dst)?
            == BufferStatus::CodecFull
        {
            return Ok(BufferStatus::CodecFull);
        }
        Ok(BufferStatus::Complete)
    }
    fn reclaim_written_frame<T, B>(
        &mut self,
        send_buffer: &SendBuffer<B>,
        dst: &mut Codec<T, Prioritized<B>>,
    ) -> bool
    where
        B: Buf,
    {
        let mut send_buffer = send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        self.actions
            .send
            .reclaim_written_frame(send_buffer, &mut self.store, dst)
    }
    fn send_reset<B>(
        &mut self,
        send_buffer: &SendBuffer<B>,
        id: StreamId,
        reason: Reason,
    ) -> Result<(), crate::h2::proto::error::GoAway> {
        let key = match self.store.find_entry(id) {
            Entry::Occupied(e) => e.key(),
            Entry::Vacant(e) => {
                if self.counts.peer().is_local_init(id) {
                    self.actions.send.maybe_reset_next_stream_id(id);
                } else {
                    self.actions.recv.maybe_reset_next_stream_id(id);
                }
                let stream = Stream::new(id, 0, 0);
                e.insert(stream)
            }
        };
        let stream = self.store.resolve(key);
        let mut send_buffer = send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        self.actions.send_reset(
            stream,
            reason,
            Initiator::Library,
            &mut self.counts,
            send_buffer,
        )
    }
}
impl<B> Streams<B, client::Peer>
where
    B: Buf,
{
    pub fn poll_pending_open(
        &mut self,
        cx: &Context,
        pending: Option<&OpaqueStreamRef>,
    ) -> Poll<Result<(), crate::h2::Error>> {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        me.actions.ensure_no_conn_error()?;
        me.actions.send.ensure_next_stream_id()?;
        if let Some(pending) = pending {
            let mut stream = me.store.resolve(pending.key);
            tracing::trace!("poll_pending_open; stream = {:?}", stream.is_pending_open);
            if stream.is_pending_open {
                stream.wait_send(cx);
                return Poll::Pending;
            }
        }
        Poll::Ready(Ok(()))
    }
}
impl<B, P> Streams<B, P>
where
    P: Peer,
{
    pub fn as_dyn(&self) -> DynStreams<'_, B> {
        let Self {
            inner,
            send_buffer,
            _p,
        } = self;
        DynStreams {
            inner,
            send_buffer,
            peer: P::r#dyn(),
        }
    }
    pub fn recv_eof(&mut self, clear_pending_accept: bool) -> Result<(), ()> {
        self.as_dyn().recv_eof(clear_pending_accept)
    }
    pub(crate) fn max_send_streams(&self) -> usize {
        self.inner.lock().unwrap().counts.max_send_streams()
    }
    pub(crate) fn max_recv_streams(&self) -> usize {
        self.inner.lock().unwrap().counts.max_recv_streams()
    }
    pub fn has_streams(&self) -> bool {
        let me = self.inner.lock().unwrap();
        me.counts.has_streams()
    }
    pub fn has_streams_or_other_references(&self) -> bool {
        let me = self.inner.lock().unwrap();
        me.counts.has_streams() || me.refs > 1
    }
}
impl<B, P> Clone for Streams<B, P>
where
    P: Peer,
{
    fn clone(&self) -> Self {
        self.inner.lock().unwrap().refs += 1;
        Streams {
            inner: self.inner.clone(),
            send_buffer: self.send_buffer.clone(),
            _p: ::std::marker::PhantomData,
        }
    }
}
impl<B, P> Drop for Streams<B, P>
where
    P: Peer,
{
    fn drop(&mut self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.refs -= 1;
            if inner.refs == 1 {
                if let Some(task) = inner.actions.task.take() {
                    task.wake();
                }
            }
        }
    }
}
impl<B> StreamRef<B> {
    pub fn send_data(&mut self, data: B, end_stream: bool) -> Result<(), UserError>
    where
        B: Buf,
    {
        let mut me = self.opaque.inner.lock().unwrap();
        let me = &mut *me;
        let stream = me.store.resolve(self.opaque.key);
        let actions = &mut me.actions;
        let mut send_buffer = self.send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        me.counts.transition(stream, |counts, stream| {
            let mut frame = frame::Data::new(stream.id, data);
            frame.set_end_stream(end_stream);
            actions
                .send
                .send_data(frame, send_buffer, stream, counts, &mut actions.task)
        })
    }
    pub fn send_trailers(&mut self, trailers: HeaderMap) -> Result<(), UserError> {
        let mut me = self.opaque.inner.lock().unwrap();
        let me = &mut *me;
        let stream = me.store.resolve(self.opaque.key);
        let actions = &mut me.actions;
        let mut send_buffer = self.send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        me.counts.transition(stream, |counts, stream| {
            let frame = frame::Headers::trailers(stream.id, trailers);
            actions
                .send
                .send_trailers(frame, send_buffer, stream, counts, &mut actions.task)
        })
    }
    pub fn send_reset(&mut self, reason: Reason) {
        let mut me = self.opaque.inner.lock().unwrap();
        let me = &mut *me;
        let stream = me.store.resolve(self.opaque.key);
        let mut send_buffer = self.send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        match me
            .actions
            .send_reset(stream, reason, Initiator::User, &mut me.counts, send_buffer)
        {
            Ok(()) => (),
            Err(crate::h2::proto::error::GoAway { .. }) => {
                unreachable!("Initiator::User should not error sending reset");
            }
        }
    }
    pub fn send_informational_headers(&mut self, frame: frame::Headers) -> Result<(), UserError> {
        let mut me = self.opaque.inner.lock().unwrap();
        let me = &mut *me;
        let stream = me.store.resolve(self.opaque.key);
        let actions = &mut me.actions;
        let mut send_buffer = self.send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        me.counts.transition(stream, |counts, stream| {
            debug_assert!(
                frame.is_informational(),
                "Frame must be informational after conversion from informational response"
            );
            if frame.is_end_stream() {
                return Err(UserError::UnexpectedFrameType);
            }
            actions.send.send_interim_informational_headers(
                frame,
                send_buffer,
                stream,
                counts,
                &mut actions.task,
            )
        })
    }
    pub fn send_response(
        &mut self,
        mut response: Response<()>,
        end_of_stream: bool,
    ) -> Result<(), UserError> {
        response.extensions_mut().clear();
        let mut me = self.opaque.inner.lock().unwrap();
        let me = &mut *me;
        let stream = me.store.resolve(self.opaque.key);
        let actions = &mut me.actions;
        let mut send_buffer = self.send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        me.counts.transition(stream, |counts, stream| {
            let frame = server::Peer::convert_send_message(stream.id, response, end_of_stream);
            actions
                .send
                .send_headers(frame, send_buffer, stream, counts, &mut actions.task)
        })
    }
    pub fn send_push_promise(
        &mut self,
        mut request: Request<()>,
    ) -> Result<StreamRef<B>, UserError> {
        request.extensions_mut().clear();
        let mut me = self.opaque.inner.lock().unwrap();
        let me = &mut *me;
        let mut send_buffer = self.send_buffer.inner.lock().unwrap();
        let send_buffer = &mut *send_buffer;
        let actions = &mut me.actions;
        let promised_id = actions.send.reserve_local()?;
        let child_key = {
            let mut child_stream = me.store.insert(
                promised_id,
                Stream::new(
                    promised_id,
                    actions.send.init_window_sz(),
                    actions.recv.init_window_sz(),
                ),
            );
            child_stream.state.reserve_local()?;
            child_stream.is_pending_push = true;
            child_stream.key()
        };
        let pushed = {
            let mut stream = me.store.resolve(self.opaque.key);
            let frame =
                crate::h2::server::Peer::convert_push_message(stream.id, promised_id, request)?;
            actions
                .send
                .send_push_promise(frame, send_buffer, &mut stream, &mut actions.task)
        };
        if let Err(err) = pushed {
            let mut child_stream = me.store.resolve(child_key);
            child_stream.unlink();
            child_stream.remove();
            return Err(err);
        }
        me.refs += 1;
        let opaque =
            OpaqueStreamRef::new(self.opaque.inner.clone(), &mut me.store.resolve(child_key));
        Ok(StreamRef {
            opaque,
            send_buffer: self.send_buffer.clone(),
        })
    }
    pub fn take_request(&self) -> Request<()> {
        let mut me = self.opaque.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.opaque.key);
        me.actions.recv.take_request(&mut stream)
    }
    pub fn is_pending_open(&self) -> bool {
        let mut me = self.opaque.inner.lock().unwrap();
        me.store.resolve(self.opaque.key).is_pending_open
    }
    pub fn reserve_capacity(&mut self, capacity: WindowSize) {
        let mut me = self.opaque.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.opaque.key);
        me.actions
            .send
            .reserve_capacity(capacity, &mut stream, &mut me.counts)
    }
    pub fn capacity(&self) -> WindowSize {
        let mut me = self.opaque.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.opaque.key);
        me.actions.send.capacity(&mut stream)
    }
    pub fn poll_capacity(&mut self, cx: &Context) -> Poll<Option<Result<WindowSize, UserError>>> {
        let mut me = self.opaque.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.opaque.key);
        me.actions.send.poll_capacity(cx, &mut stream)
    }
    pub(crate) fn poll_reset(
        &mut self,
        cx: &Context,
        mode: proto::PollReset,
    ) -> Poll<Result<Reason, crate::h2::Error>> {
        let mut me = self.opaque.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.opaque.key);
        me.actions.send.poll_reset(cx, &mut stream, mode)
    }
    pub fn clone_to_opaque(&self) -> OpaqueStreamRef {
        self.opaque.clone()
    }
    pub fn stream_id(&self) -> StreamId {
        self.opaque.stream_id()
    }
}
impl<B> Clone for StreamRef<B> {
    fn clone(&self) -> Self {
        StreamRef {
            opaque: self.opaque.clone(),
            send_buffer: self.send_buffer.clone(),
        }
    }
}
impl OpaqueStreamRef {
    fn new(inner: Arc<Mutex<Inner>>, stream: &mut store::Ptr) -> OpaqueStreamRef {
        stream.ref_inc();
        OpaqueStreamRef {
            inner,
            key: stream.key(),
        }
    }
    pub fn poll_response(&mut self, cx: &Context) -> Poll<Result<Response<()>, proto::Error>> {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.key);
        me.actions.recv.poll_response(cx, &mut stream)
    }
    pub fn poll_informational(
        &mut self,
        cx: &Context,
    ) -> Poll<Option<Result<Response<()>, proto::Error>>> {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.key);
        me.actions.recv.poll_informational(cx, &mut stream)
    }
    pub fn poll_pushed(
        &mut self,
        cx: &Context,
    ) -> Poll<Option<Result<(Request<()>, OpaqueStreamRef), proto::Error>>> {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.key);
        me.actions
            .recv
            .poll_pushed(cx, &mut stream)
            .map_ok(|(h, key)| {
                me.refs += 1;
                let opaque_ref =
                    OpaqueStreamRef::new(self.inner.clone(), &mut me.store.resolve(key));
                (h, opaque_ref)
            })
    }
    pub fn is_end_stream(&self) -> bool {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        let stream = me.store.resolve(self.key);
        me.actions.recv.is_end_stream(&stream)
    }
    pub fn poll_data(&mut self, cx: &Context) -> Poll<Option<Result<Bytes, proto::Error>>> {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.key);
        me.actions
            .recv
            .poll_data(cx, &mut stream)
            .map(|result| match result {
                Some(Ok(data)) => {
                    if data.is_budgeted {
                        me.counts.release_data_frame(data.payload.len());
                    }
                    Some(Ok(data.payload))
                }
                Some(Err(err)) => Some(Err(err)),
                None => None,
            })
    }
    pub fn poll_trailers(&mut self, cx: &Context) -> Poll<Option<Result<HeaderMap, proto::Error>>> {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.key);
        me.actions.recv.poll_trailers(cx, &mut stream)
    }
    pub(crate) fn available_recv_capacity(&self) -> isize {
        let me = self.inner.lock().unwrap();
        let me = &*me;
        let stream = &me.store[self.key];
        stream.recv_flow.available().into()
    }
    pub(crate) fn used_recv_capacity(&self) -> WindowSize {
        let me = self.inner.lock().unwrap();
        let me = &*me;
        let stream = &me.store[self.key];
        stream.in_flight_recv_data
    }
    pub fn release_capacity(&mut self, capacity: WindowSize) -> Result<(), UserError> {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.key);
        me.actions
            .recv
            .release_capacity(capacity, &mut stream, &mut me.actions.task)
    }
    pub(crate) fn clear_recv_buffer(&mut self) {
        let mut me = self.inner.lock().unwrap();
        let me = &mut *me;
        let mut stream = me.store.resolve(self.key);
        stream.is_recv = false;
        me.actions
            .recv
            .clear_recv_buffer(&mut stream, &mut me.actions.task, &mut me.counts);
    }
    pub fn stream_id(&self) -> StreamId {
        self.inner.lock().unwrap().store[self.key].id
    }
}
impl fmt::Debug for OpaqueStreamRef {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use std::sync::TryLockError::*;
        match self.inner.try_lock() {
            Ok(me) => {
                let stream = &me.store[self.key];
                fmt.debug_struct("OpaqueStreamRef")
                    .field("stream_id", &stream.id)
                    .field("ref_count", &stream.ref_count)
                    .finish()
            }
            Err(Poisoned(_)) => fmt
                .debug_struct("OpaqueStreamRef")
                .field("inner", &"<Poisoned>")
                .finish(),
            Err(WouldBlock) => fmt
                .debug_struct("OpaqueStreamRef")
                .field("inner", &"<Locked>")
                .finish(),
        }
    }
}
impl Clone for OpaqueStreamRef {
    fn clone(&self) -> Self {
        let mut inner = self.inner.lock().unwrap();
        inner.store.resolve(self.key).ref_inc();
        inner.refs += 1;
        OpaqueStreamRef {
            inner: self.inner.clone(),
            key: self.key,
        }
    }
}
impl Drop for OpaqueStreamRef {
    fn drop(&mut self) {
        drop_stream_ref(&self.inner, self.key);
    }
}
fn drop_stream_ref(inner: &Mutex<Inner>, key: store::Key) {
    let mut me = match inner.lock() {
        Ok(inner) => inner,
        Err(_) => {
            if ::std::thread::panicking() {
                tracing::trace!("StreamRef::drop; mutex poisoned");
                return;
            } else {
                panic!("StreamRef::drop; mutex poisoned");
            }
        }
    };
    let me = &mut *me;
    me.refs -= 1;
    let mut stream = me.store.resolve(key);
    tracing::trace!("drop_stream_ref; stream={:?}", stream);
    stream.ref_dec();
    let actions = &mut me.actions;
    if stream.ref_count == 0 && stream.is_closed() {
        if let Some(task) = actions.task.take() {
            task.wake();
        }
    }
    me.counts.transition(stream, |counts, stream| {
        maybe_cancel(stream, actions, counts);
        if stream.ref_count == 0 {
            actions
                .recv
                .release_closed_capacity(stream, &mut actions.task, counts);
            let mut ppp = stream.pending_push_promises.take();
            while let Some(promise) = ppp.pop(stream.store_mut()) {
                counts.transition(promise, |counts, stream| {
                    maybe_cancel(stream, actions, counts);
                });
            }
        }
    });
}
fn maybe_cancel(stream: &mut store::Ptr, actions: &mut Actions, counts: &mut Counts) {
    if stream.is_canceled_interest() {
        let reason = if counts.peer().is_server()
            && stream.state.is_send_closed()
            && stream.state.is_recv_streaming()
        {
            Reason::NO_ERROR
        } else {
            Reason::CANCEL
        };
        actions
            .send
            .schedule_implicit_reset(stream, reason, counts, &mut actions.task);
        actions.recv.enqueue_reset_expiration(stream, counts);
    }
}
impl<B> SendBuffer<B> {
    fn new() -> Self {
        let inner = Mutex::new(Buffer::new());
        SendBuffer { inner }
    }
    pub fn is_empty(&self) -> bool {
        let buf = self.inner.lock().unwrap();
        buf.is_empty()
    }
}
impl Actions {
    fn send_reset<B>(
        &mut self,
        stream: store::Ptr,
        reason: Reason,
        initiator: Initiator,
        counts: &mut Counts,
        send_buffer: &mut Buffer<Frame<B>>,
    ) -> Result<(), crate::h2::proto::error::GoAway> {
        counts.transition(stream, |counts, stream| {
            if initiator.is_library() {
                if counts.can_inc_num_local_error_resets() {
                    counts.inc_num_local_error_resets();
                } else {
                    tracing::warn!(
                        "locally-reset streams reached limit ({:?})",
                        counts.max_local_error_resets().unwrap(),
                    );
                    return Err(crate::h2::proto::error::GoAway {
                        reason: Reason::ENHANCE_YOUR_CALM,
                        debug_data: "too_many_internal_resets".into(),
                    });
                }
            }
            self.send.send_reset(
                reason,
                initiator,
                send_buffer,
                stream,
                counts,
                &mut self.task,
            );
            self.recv.enqueue_reset_expiration(stream, counts);
            stream.notify_recv();
            Ok(())
        })
    }
    fn reset_on_recv_stream_err<B>(
        &mut self,
        buffer: &mut Buffer<Frame<B>>,
        stream: &mut store::Ptr,
        counts: &mut Counts,
        res: Result<(), Error>,
    ) -> Result<(), Error> {
        if let Err(Error::Reset(stream_id, reason, initiator)) = res {
            debug_assert_eq!(stream_id, stream.id);
            if counts.can_inc_num_local_error_resets() {
                counts.inc_num_local_error_resets();
                self.send
                    .send_reset(reason, initiator, buffer, stream, counts, &mut self.task);
                self.recv.enqueue_reset_expiration(stream, counts);
                stream.notify_recv();
                Ok(())
            } else {
                tracing::warn!(
                    "reset_on_recv_stream_err; locally-reset streams reached limit ({:?})",
                    counts.max_local_error_resets().unwrap(),
                );
                Err(Error::library_go_away_data(
                    Reason::ENHANCE_YOUR_CALM,
                    "too_many_internal_resets",
                ))
            }
        } else {
            res
        }
    }
    fn ensure_not_idle(&mut self, peer: peer::Dyn, id: StreamId) -> Result<(), Reason> {
        if peer.is_local_init(id) {
            self.send.ensure_not_idle(id)
        } else {
            self.recv.ensure_not_idle(id)
        }
    }
    fn ensure_no_conn_error(&self) -> Result<(), proto::Error> {
        if let Some(ref err) = self.conn_error {
            Err(err.clone())
        } else {
            Ok(())
        }
    }
    fn may_have_forgotten_stream(&self, peer: peer::Dyn, id: StreamId) -> bool {
        if id.is_zero() {
            return false;
        }
        if peer.is_local_init(id) {
            self.send.may_have_created_stream(id)
        } else {
            self.recv.may_have_created_stream(id)
        }
    }
    fn clear_queues(&mut self, clear_pending_accept: bool, store: &mut Store, counts: &mut Counts) {
        self.recv.clear_queues(clear_pending_accept, store, counts);
        self.send.clear_queues(store, counts);
    }
}
