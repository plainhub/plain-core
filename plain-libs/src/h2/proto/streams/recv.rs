use super::*;
use crate::h2::codec::UserError;
use crate::h2::frame::{DEFAULT_INITIAL_WINDOW_SIZE, PushPromiseHeaderError, Reason};
use crate::h2::proto;
use crate::http::{HeaderMap, Request, Response};
use std::cmp::Ordering;
use std::io;
use std::task::{Context, Poll, Waker};
use std::time::Instant;
#[derive(Debug)]
pub(super) struct Recv {
    init_window_sz: WindowSize,
    flow: FlowControl,
    in_flight_data: WindowSize,
    next_stream_id: Result<StreamId, StreamIdOverflow>,
    last_processed_id: StreamId,
    max_stream_id: StreamId,
    pending_window_updates: store::Queue<stream::NextWindowUpdate>,
    pending_accept: store::Queue<stream::NextAccept>,
    pending_reset_expired: store::Queue<stream::NextResetExpire>,
    reset_duration: Duration,
    buffer: Buffer<Event>,
    refused: Option<StreamId>,
    is_push_enabled: bool,
    is_extended_connect_protocol_enabled: bool,
}
#[derive(Debug)]
pub(super) enum Event {
    Headers(peer::PollMessage),
    Data(DataEvent),
    Trailers(HeaderMap),
    InformationalHeaders(peer::PollMessage),
}
#[derive(Debug)]
pub(super) struct DataEvent {
    pub(super) payload: Bytes,
    pub(super) is_budgeted: bool,
}
#[derive(Debug)]
pub(super) enum RecvHeaderBlockError<T> {
    Oversize(T),
    State(Error),
}
#[derive(Debug)]
pub(crate) enum Open {
    PushPromise,
    Headers,
}
impl Recv {
    pub fn new(peer: peer::Dyn, config: &Config) -> Self {
        let next_stream_id = if peer.is_server() { 1 } else { 2 };
        let mut flow = FlowControl::new();
        flow.inc_window(DEFAULT_INITIAL_WINDOW_SIZE)
            .expect("invalid initial remote window size");
        flow.assign_capacity(DEFAULT_INITIAL_WINDOW_SIZE).unwrap();
        Recv {
            init_window_sz: DEFAULT_INITIAL_WINDOW_SIZE,
            flow,
            in_flight_data: 0 as WindowSize,
            next_stream_id: Ok(next_stream_id.into()),
            pending_window_updates: store::Queue::new(),
            last_processed_id: StreamId::ZERO,
            max_stream_id: StreamId::MAX,
            pending_accept: store::Queue::new(),
            pending_reset_expired: store::Queue::new(),
            reset_duration: config.local_reset_duration,
            buffer: Buffer::new(),
            refused: None,
            is_push_enabled: config.local_push_enabled,
            is_extended_connect_protocol_enabled: config.extended_connect_protocol_enabled,
        }
    }
    pub fn init_window_sz(&self) -> WindowSize {
        self.init_window_sz
    }
    pub fn last_processed_id(&self) -> StreamId {
        self.last_processed_id
    }
    pub fn open(
        &mut self,
        id: StreamId,
        mode: Open,
        counts: &mut Counts,
    ) -> Result<Option<StreamId>, Error> {
        assert!(self.refused.is_none());
        counts.peer().ensure_can_open(id, mode)?;
        let next_id = self.next_stream_id()?;
        if id < next_id {
            proto_err ! (conn : "id ({:?}) < next_id ({:?})" , id , next_id);
            return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
        }
        self.next_stream_id = id.next_id();
        if !counts.can_inc_num_recv_streams() {
            self.refused = Some(id);
            return Ok(None);
        }
        Ok(Some(id))
    }
    pub fn recv_headers(
        &mut self,
        frame: frame::Headers,
        stream: &mut store::Ptr,
        counts: &mut Counts,
    ) -> Result<(), RecvHeaderBlockError<Option<frame::Headers>>> {
        tracing::trace!("opening stream; init_window={}", self.init_window_sz);
        let is_initial = stream.state.recv_open(&frame)?;
        if is_initial && !stream.is_counted {
            if frame.stream_id() > self.last_processed_id {
                self.last_processed_id = frame.stream_id();
            }
            counts.inc_num_recv_streams(stream);
        }
        if !stream.content_length.is_head() {
            use super::stream::ContentLength;
            use crate::http::header;
            if let Some(content_length) = frame.fields().get(header::CONTENT_LENGTH) {
                let content_length = match frame::parse_u64(content_length.as_bytes()) {
                    Ok(v) => v,
                    Err(_) => {
                        proto_err ! (stream : "could not parse content-length; stream={:?}" , stream . id);
                        return Err(Error::library_reset(stream.id, Reason::PROTOCOL_ERROR).into());
                    }
                };
                stream.content_length = ContentLength::Remaining(content_length);
                if frame.is_end_stream()
                    && content_length > 0
                    && frame
                        .pseudo()
                        .status
                        .map_or(true, |status| status != 204 && status != 304)
                {
                    proto_err ! (stream : "recv_headers with END_STREAM: content-length is not zero; stream={:?};" , stream . id);
                    return Err(Error::library_reset(stream.id, Reason::PROTOCOL_ERROR).into());
                }
            }
        }
        if frame.is_over_size() {
            tracing::debug!(
                "stream error REQUEST_HEADER_FIELDS_TOO_LARGE -- \
                 recv_headers: frame is over size; stream={:?}",
                stream.id
            );
            return if counts.peer().is_server() && is_initial {
                let mut res = frame::Headers::new(
                    stream.id,
                    frame::Pseudo::response(
                        crate::http::StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
                    ),
                    HeaderMap::new(),
                );
                res.set_end_stream();
                Err(RecvHeaderBlockError::Oversize(Some(res)))
            } else {
                Err(RecvHeaderBlockError::Oversize(None))
            };
        }
        let stream_id = frame.stream_id();
        let (pseudo, fields) = frame.into_parts();
        if pseudo.protocol.is_some()
            && counts.peer().is_server()
            && !self.is_extended_connect_protocol_enabled
        {
            proto_err ! (stream : "cannot use :protocol if extended connect protocol is disabled; stream={:?}" , stream . id);
            return Err(Error::library_reset(stream.id, Reason::PROTOCOL_ERROR).into());
        }
        if pseudo.status.is_some() && counts.peer().is_server() {
            proto_err ! (stream : "cannot use :status header for requests; stream={:?}" , stream . id);
            return Err(Error::library_reset(stream.id, Reason::PROTOCOL_ERROR).into());
        }
        if !pseudo.is_informational() {
            let message = counts
                .peer()
                .convert_poll_message(pseudo, fields, stream_id)?;
            stream
                .pending_recv
                .push_back(&mut self.buffer, Event::Headers(message));
            stream.notify_recv();
            if counts.peer().is_server() {
                self.pending_accept.push(stream);
            }
        } else {
            let message = counts
                .peer()
                .convert_poll_message(pseudo, fields, stream_id)?;
            tracing::trace!("Received informational response: stream_id={:?}", stream_id);
            stream
                .pending_recv
                .push_back(&mut self.buffer, Event::InformationalHeaders(message));
            stream.notify_recv();
        }
        Ok(())
    }
    pub fn take_request(&mut self, stream: &mut store::Ptr) -> Request<()> {
        use super::peer::PollMessage::*;
        match stream.pending_recv.pop_front(&mut self.buffer) {
            Some(Event::Headers(Server(request))) => request,
            _ => unreachable!("server stream queue must start with Headers"),
        }
    }
    pub fn poll_pushed(
        &mut self,
        cx: &Context,
        stream: &mut store::Ptr,
    ) -> Poll<Option<Result<(Request<()>, store::Key), proto::Error>>> {
        use super::peer::PollMessage::*;
        let mut ppp = stream.pending_push_promises.take();
        let pushed = ppp.pop(stream.store_mut()).map(|mut pushed| {
            match pushed.pending_recv.pop_front(&mut self.buffer) {
                Some(Event::Headers(Server(headers))) => (headers, pushed.key()),
                _ => panic!("Headers not set on pushed stream"),
            }
        });
        stream.pending_push_promises = ppp;
        if let Some(p) = pushed {
            Poll::Ready(Some(Ok(p)))
        } else {
            let is_open = stream.state.ensure_recv_open()?;
            if is_open {
                stream.push_task = Some(cx.waker().clone());
                Poll::Pending
            } else {
                Poll::Ready(None)
            }
        }
    }
    pub fn poll_response(
        &mut self,
        cx: &Context,
        stream: &mut store::Ptr,
    ) -> Poll<Result<Response<()>, proto::Error>> {
        use super::peer::PollMessage::*;
        loop {
            match stream.pending_recv.pop_front(&mut self.buffer) {
                Some(Event::Headers(Client(response))) => return Poll::Ready(Ok(response)),
                Some(Event::InformationalHeaders(_)) => {
                    tracing::trace!(
                        "Skipping informational response in poll_response - should be consumed via poll_informational; stream_id={:?}",
                        stream.id
                    );
                    continue;
                }
                Some(_) => panic!("poll_response called after response returned"),
                None => {
                    if !stream.state.ensure_recv_open()? {
                        proto_err ! (stream : "poll_response: stream={:?} is not opened;" , stream . id);
                        return Poll::Ready(Err(Error::library_reset(
                            stream.id,
                            Reason::PROTOCOL_ERROR,
                        )));
                    }
                    stream.recv_task = Some(cx.waker().clone());
                    return Poll::Pending;
                }
            }
        }
    }
    pub fn poll_informational(
        &mut self,
        cx: &Context,
        stream: &mut store::Ptr,
    ) -> Poll<Option<Result<Response<()>, proto::Error>>> {
        use super::peer::PollMessage::*;
        if let Some(event) = stream.pending_recv.pop_front(&mut self.buffer) {
            match event {
                Event::Headers(Client(response)) => {
                    stream
                        .pending_recv
                        .push_front(&mut self.buffer, Event::Headers(Client(response)));
                    return Poll::Ready(None);
                }
                Event::InformationalHeaders(Client(response)) => {
                    return Poll::Ready(Some(Ok(response)));
                }
                other => {
                    stream.pending_recv.push_front(&mut self.buffer, other);
                }
            }
        }
        if stream.state.ensure_recv_open()? {
            stream.recv_task = Some(cx.waker().clone());
            Poll::Pending
        } else {
            Poll::Ready(None)
        }
    }
    pub fn recv_trailers(
        &mut self,
        frame: frame::Headers,
        stream: &mut store::Ptr,
    ) -> Result<(), Error> {
        stream.state.recv_close()?;
        if stream.ensure_content_length_zero().is_err() {
            proto_err ! (stream : "recv_trailers: content-length is not zero; stream={:?};" , stream . id);
            return Err(Error::library_reset(stream.id, Reason::PROTOCOL_ERROR));
        }
        let trailers = frame.into_fields();
        stream
            .pending_recv
            .push_back(&mut self.buffer, Event::Trailers(trailers));
        stream.notify_recv();
        Ok(())
    }
    pub fn release_connection_capacity(&mut self, capacity: WindowSize, task: &mut Option<Waker>) {
        tracing::trace!(
            "release_connection_capacity; size={}, connection in_flight_data={}",
            capacity,
            self.in_flight_data,
        );
        self.in_flight_data -= capacity;
        let _res = self.flow.assign_capacity(capacity);
        debug_assert!(_res.is_ok());
        if self.flow.unclaimed_capacity().is_some() {
            if let Some(task) = task.take() {
                task.wake();
            }
        }
    }
    pub fn release_capacity(
        &mut self,
        capacity: WindowSize,
        stream: &mut store::Ptr,
        task: &mut Option<Waker>,
    ) -> Result<(), UserError> {
        tracing::trace!("release_capacity; size={}", capacity);
        if capacity > stream.in_flight_recv_data {
            return Err(UserError::ReleaseCapacityTooBig);
        }
        self.release_connection_capacity(capacity, task);
        stream.in_flight_recv_data -= capacity;
        let _res = stream.recv_flow.assign_capacity(capacity);
        debug_assert!(_res.is_ok());
        if stream.recv_flow.unclaimed_capacity().is_some() {
            self.pending_window_updates.push(stream);
            if let Some(task) = task.take() {
                task.wake();
            }
        }
        Ok(())
    }
    pub fn release_closed_capacity(
        &mut self,
        stream: &mut store::Ptr,
        task: &mut Option<Waker>,
        counts: &mut Counts,
    ) {
        debug_assert_eq!(stream.ref_count, 0);
        if stream.in_flight_recv_data != 0 {
            tracing::trace!(
                "auto-release closed stream ({:?}) capacity: {:?}",
                stream.id,
                stream.in_flight_recv_data,
            );
            self.release_connection_capacity(stream.in_flight_recv_data, task);
            stream.in_flight_recv_data = 0;
        }
        self.clear_recv_buffer(stream, task, counts);
    }
    pub fn set_target_connection_window(
        &mut self,
        target: WindowSize,
        task: &mut Option<Waker>,
    ) -> Result<(), Reason> {
        tracing::trace!(
            "set_target_connection_window; target={}; available={}, reserved={}",
            target,
            self.flow.available(),
            self.in_flight_data,
        );
        let current = self
            .flow
            .available()
            .add(self.in_flight_data)?
            .checked_size();
        if target > current {
            self.flow.assign_capacity(target - current)?;
        } else {
            self.flow.claim_capacity(current - target)?;
        }
        if self.flow.unclaimed_capacity().is_some() {
            if let Some(task) = task.take() {
                task.wake();
            }
        }
        Ok(())
    }
    pub(crate) fn apply_local_settings(
        &mut self,
        settings: &frame::Settings,
        store: &mut Store,
    ) -> Result<(), proto::Error> {
        if let Some(val) = settings.is_extended_connect_protocol_enabled() {
            self.is_extended_connect_protocol_enabled = val;
        }
        if let Some(target) = settings.initial_window_size() {
            let old_sz = self.init_window_sz;
            self.init_window_sz = target;
            tracing::trace!("update_initial_window_size; new={}; old={}", target, old_sz,);
            match target.cmp(&old_sz) {
                Ordering::Less => {
                    let dec = old_sz - target;
                    tracing::trace!("decrementing all windows; dec={}", dec);
                    store.try_for_each(|mut stream| {
                        stream
                            .recv_flow
                            .dec_recv_window(dec)
                            .map_err(proto::Error::library_go_away)?;
                        Ok::<_, proto::Error>(())
                    })?;
                }
                Ordering::Greater => {
                    let inc = target - old_sz;
                    tracing::trace!("incrementing all windows; inc={}", inc);
                    store.try_for_each(|mut stream| {
                        stream
                            .recv_flow
                            .inc_window(inc)
                            .map_err(proto::Error::library_go_away)?;
                        stream
                            .recv_flow
                            .assign_capacity(inc)
                            .map_err(proto::Error::library_go_away)?;
                        Ok::<_, proto::Error>(())
                    })?;
                }
                Ordering::Equal => (),
            }
        }
        Ok(())
    }
    pub fn is_end_stream(&self, stream: &store::Ptr) -> bool {
        if !stream.state.is_recv_end_stream() {
            return false;
        }
        stream.pending_recv.is_empty()
    }
    pub fn recv_data(&mut self, frame: frame::Data, stream: &mut store::Ptr) -> Result<(), Error> {
        let sz = frame.flow_controlled_len();
        assert!(sz <= MAX_WINDOW_SIZE as usize);
        let sz = sz as WindowSize;
        let is_ignoring_frame = stream.state.is_local_error();
        if !is_ignoring_frame && !stream.state.is_recv_streaming() {
            proto_err ! (conn : "unexpected DATA frame; stream={:?}" , stream . id);
            return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
        }
        tracing::trace!(
            "recv_data; size={}; connection={}; stream={}",
            sz,
            self.flow.window_size(),
            stream.recv_flow.window_size()
        );
        if is_ignoring_frame {
            tracing::trace!(
                "recv_data; frame ignored on locally reset {:?} for some time",
                stream.id,
            );
            return self.ignore_data(sz);
        }
        self.consume_connection_window(sz)?;
        if stream.recv_flow.window_size() < sz {
            return Err(Error::library_reset(stream.id, Reason::FLOW_CONTROL_ERROR));
        }
        if stream.dec_content_length(frame.payload().len()).is_err() {
            proto_err ! (stream : "recv_data: content-length overflow; stream={:?}; len={:?}" , stream . id , frame . payload () . len () ,);
            return Err(Error::library_reset(stream.id, Reason::PROTOCOL_ERROR));
        }
        if frame.is_end_stream() {
            if stream.ensure_content_length_zero().is_err() {
                proto_err ! (stream : "recv_data: content-length underflow; stream={:?}; len={:?}" , stream . id , frame . payload () . len () ,);
                return Err(Error::library_reset(stream.id, Reason::PROTOCOL_ERROR));
            }
            if stream.state.recv_close().is_err() {
                proto_err ! (conn : "recv_data: failed to transition to closed state; stream={:?}" , stream . id);
                return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
            }
        }
        if !stream.is_recv {
            tracing::trace!(
                "recv_data; frame ignored on stream release {:?} for some time",
                stream.id,
            );
            self.release_connection_capacity(sz, &mut None);
            return Ok(());
        }
        stream
            .recv_flow
            .send_data(sz)
            .map_err(proto::Error::library_go_away)?;
        stream.in_flight_recv_data += sz;
        let padding = (frame.flow_controlled_len() - frame.payload().len()) as WindowSize;
        if padding > 0 {
            tracing::trace!(
                "recv_data; auto-releasing padding of {:?} for {:?}",
                padding,
                stream.id,
            );
            let _res = self.release_capacity(padding, stream, &mut None);
            debug_assert!(_res.is_ok());
        }
        if frame.payload().is_empty() && !frame.is_end_stream() {
            return Ok(());
        }
        let is_budgeted = !frame.is_end_stream();
        let event = Event::Data(DataEvent {
            payload: frame.into_payload(),
            is_budgeted,
        });
        stream.pending_recv.push_back(&mut self.buffer, event);
        stream.notify_recv();
        Ok(())
    }
    pub fn ignore_data(&mut self, sz: WindowSize) -> Result<(), Error> {
        self.consume_connection_window(sz)?;
        self.release_connection_capacity(sz, &mut None);
        Ok(())
    }
    pub fn consume_connection_window(&mut self, sz: WindowSize) -> Result<(), Error> {
        if self.flow.window_size() < sz {
            tracing::debug!(
                "connection error FLOW_CONTROL_ERROR -- window_size ({:?}) < sz ({:?});",
                self.flow.window_size(),
                sz,
            );
            return Err(Error::library_go_away(Reason::FLOW_CONTROL_ERROR));
        }
        self.flow.send_data(sz).map_err(Error::library_go_away)?;
        self.in_flight_data += sz;
        Ok(())
    }
    pub fn recv_push_promise(
        &mut self,
        frame: frame::PushPromise,
        stream: &mut store::Ptr,
    ) -> Result<(), Error> {
        stream.state.reserve_remote()?;
        if frame.is_over_size() {
            tracing::debug!(
                "stream error PROTOCOL_ERROR -- recv_push_promise: \
                 headers frame is over size; promised_id={:?};",
                frame.promised_id(),
            );
            return Err(Error::library_reset(
                frame.promised_id(),
                Reason::PROTOCOL_ERROR,
            ));
        }
        let promised_id = frame.promised_id();
        let (pseudo, fields) = frame.into_parts();
        let req = crate::h2::server::Peer::convert_poll_message(pseudo, fields, promised_id)?;
        if let Err(e) = frame::PushPromise::validate_request(&req) {
            use PushPromiseHeaderError::*;
            match e {
                NotSafeAndCacheable => {
                    proto_err ! (stream : "recv_push_promise: method {} is not safe and cacheable; promised_id={:?}" , req . method () , promised_id ,)
                }
                InvalidContentLength(e) => {
                    proto_err ! (stream : "recv_push_promise; promised request has invalid content-length {:?}; promised_id={:?}" , e , promised_id ,)
                }
            }
            return Err(Error::library_reset(promised_id, Reason::PROTOCOL_ERROR));
        }
        use super::peer::PollMessage::*;
        stream
            .pending_recv
            .push_back(&mut self.buffer, Event::Headers(Server(req)));
        stream.notify_recv();
        stream.notify_push();
        Ok(())
    }
    pub fn ensure_not_idle(&self, id: StreamId) -> Result<(), Reason> {
        if let Ok(next) = self.next_stream_id {
            if id >= next {
                tracing::debug!(
                    "stream ID implicitly closed, PROTOCOL_ERROR; stream={:?}",
                    id
                );
                return Err(Reason::PROTOCOL_ERROR);
            }
        }
        Ok(())
    }
    pub fn recv_reset(
        &mut self,
        frame: frame::Reset,
        stream: &mut Stream,
        counts: &mut Counts,
    ) -> Result<(), Error> {
        if stream.is_pending_accept {
            if counts.can_inc_num_remote_reset_streams() {
                counts.inc_num_remote_reset_streams();
            } else {
                tracing::warn!(
                    "recv_reset; remotely-reset pending-accept streams reached limit ({:?})",
                    counts.max_remote_reset_streams(),
                );
                return Err(Error::library_go_away_data(
                    Reason::ENHANCE_YOUR_CALM,
                    "too_many_resets",
                ));
            }
        }
        stream.state.recv_reset(frame, stream.is_pending_send);
        stream.notify_send();
        stream.notify_recv();
        stream.notify_push();
        Ok(())
    }
    pub fn handle_error(&mut self, err: &proto::Error, stream: &mut Stream) {
        stream.state.handle_error(err);
        stream.notify_send();
        stream.notify_recv();
        stream.notify_push();
    }
    pub fn go_away(&mut self, last_processed_id: StreamId) {
        assert!(self.max_stream_id >= last_processed_id);
        self.max_stream_id = last_processed_id;
    }
    pub fn recv_eof(&mut self, stream: &mut Stream) {
        stream.state.recv_eof();
        stream.notify_send();
        stream.notify_recv();
        stream.notify_push();
    }
    pub(super) fn clear_recv_buffer(
        &mut self,
        stream: &mut Stream,
        task: &mut Option<Waker>,
        counts: &mut Counts,
    ) {
        let mut to_release: WindowSize = 0;
        while let Some(event) = stream.pending_recv.pop_front(&mut self.buffer) {
            if let Event::Data(data) = &event {
                if data.is_budgeted {
                    counts.release_data_frame(data.payload.len());
                }
                to_release = to_release
                    .saturating_add(data.payload.len() as WindowSize)
                    .min(stream.in_flight_recv_data);
            }
        }
        if to_release > 0 {
            stream.in_flight_recv_data -= to_release;
            self.release_connection_capacity(to_release, task);
        }
    }
    pub fn max_stream_id(&self) -> StreamId {
        self.max_stream_id
    }
    pub fn next_stream_id(&self) -> Result<StreamId, Error> {
        if let Ok(id) = self.next_stream_id {
            Ok(id)
        } else {
            Err(Error::library_go_away(Reason::PROTOCOL_ERROR))
        }
    }
    pub fn may_have_created_stream(&self, id: StreamId) -> bool {
        if let Ok(next_id) = self.next_stream_id {
            debug_assert_eq!(id.is_server_initiated(), next_id.is_server_initiated(),);
            id < next_id
        } else {
            true
        }
    }
    pub(super) fn maybe_reset_next_stream_id(&mut self, id: StreamId) {
        if let Ok(next_id) = self.next_stream_id {
            debug_assert_eq!(id.is_server_initiated(), next_id.is_server_initiated());
            if id >= next_id {
                self.next_stream_id = id.next_id();
            }
        }
    }
    pub fn ensure_can_reserve(&self) -> Result<(), Error> {
        if !self.is_push_enabled {
            proto_err ! (conn : "recv_push_promise: push is disabled");
            return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
        }
        Ok(())
    }
    pub fn enqueue_reset_expiration(&mut self, stream: &mut store::Ptr, counts: &mut Counts) {
        if !stream.state.is_local_error() || stream.is_pending_reset_expiration() {
            return;
        }
        if counts.can_inc_num_reset_streams() {
            counts.inc_num_reset_streams();
            tracing::trace!("enqueue_reset_expiration; added {:?}", stream.id);
            self.pending_reset_expired.push(stream);
        } else {
            tracing::trace!(
                "enqueue_reset_expiration; dropped {:?}, over max_concurrent_reset_streams",
                stream.id
            );
        }
    }
    pub fn send_pending_refusal<T, B>(
        &mut self,
        dst: &mut Codec<T, Prioritized<B>>,
    ) -> io::Result<BufferStatus>
    where
        T: AsyncWrite + Unpin,
        B: Buf,
    {
        if let Some(stream_id) = self.refused {
            if !dst.has_send_capacity() {
                return Ok(BufferStatus::CodecFull);
            }
            let frame = frame::Reset::new(stream_id, Reason::REFUSED_STREAM);
            dst.buffer(frame.into()).expect("invalid RST_STREAM frame");
        }
        self.refused = None;
        Ok(BufferStatus::Complete)
    }
    pub fn clear_expired_reset_streams(&mut self, store: &mut Store, counts: &mut Counts) {
        if !self.pending_reset_expired.is_empty() {
            let now = Instant::now();
            let reset_duration = self.reset_duration;
            while let Some(stream) = self.pending_reset_expired.pop_if(store, |stream| {
                let reset_at = stream.reset_at.expect("reset_at must be set if in queue");
                now.saturating_duration_since(reset_at) > reset_duration
            }) {
                counts.transition_after(stream, true);
            }
        }
    }
    pub fn clear_queues(
        &mut self,
        clear_pending_accept: bool,
        store: &mut Store,
        counts: &mut Counts,
    ) {
        self.clear_stream_window_update_queue(store, counts);
        self.clear_all_reset_streams(store, counts);
        if clear_pending_accept {
            self.clear_all_pending_accept(store, counts);
        }
    }
    fn clear_stream_window_update_queue(&mut self, store: &mut Store, counts: &mut Counts) {
        while let Some(stream) = self.pending_window_updates.pop(store) {
            counts.transition(stream, |_, stream| {
                tracing::trace!("clear_stream_window_update_queue; stream={:?}", stream.id);
            })
        }
    }
    fn clear_all_reset_streams(&mut self, store: &mut Store, counts: &mut Counts) {
        while let Some(stream) = self.pending_reset_expired.pop(store) {
            counts.transition_after(stream, true);
        }
    }
    fn clear_all_pending_accept(&mut self, store: &mut Store, counts: &mut Counts) {
        while let Some(stream) = self.pending_accept.pop(store) {
            counts.transition_after(stream, false);
        }
    }
    pub fn buffer_pending<T, B>(
        &mut self,
        store: &mut Store,
        counts: &mut Counts,
        dst: &mut Codec<T, Prioritized<B>>,
    ) -> io::Result<BufferStatus>
    where
        T: AsyncWrite + Unpin,
        B: Buf,
    {
        if self.send_connection_window_update(dst)? == BufferStatus::CodecFull {
            return Ok(BufferStatus::CodecFull);
        }
        if self.send_stream_window_updates(store, counts, dst)? == BufferStatus::CodecFull {
            return Ok(BufferStatus::CodecFull);
        }
        Ok(BufferStatus::Complete)
    }
    fn send_connection_window_update<T, B>(
        &mut self,
        dst: &mut Codec<T, Prioritized<B>>,
    ) -> io::Result<BufferStatus>
    where
        T: AsyncWrite + Unpin,
        B: Buf,
    {
        if let Some(incr) = self.flow.unclaimed_capacity() {
            let frame = frame::WindowUpdate::new(StreamId::zero(), incr);
            if !dst.has_send_capacity() {
                return Ok(BufferStatus::CodecFull);
            }
            dst.buffer(frame.into())
                .expect("invalid WINDOW_UPDATE frame");
            self.flow
                .inc_window(incr)
                .expect("unexpected flow control state");
        }
        Ok(BufferStatus::Complete)
    }
    pub fn send_stream_window_updates<T, B>(
        &mut self,
        store: &mut Store,
        counts: &mut Counts,
        dst: &mut Codec<T, Prioritized<B>>,
    ) -> io::Result<BufferStatus>
    where
        T: AsyncWrite + Unpin,
        B: Buf,
    {
        loop {
            if !dst.has_send_capacity() {
                return Ok(BufferStatus::CodecFull);
            }
            let stream = match self.pending_window_updates.pop(store) {
                Some(stream) => stream,
                None => return Ok(BufferStatus::Complete),
            };
            counts.transition(stream, |_, stream| {
                tracing::trace!("pending_window_updates -- pop; stream={:?}", stream.id);
                debug_assert!(!stream.is_pending_window_update);
                if !stream.state.is_recv_streaming() {
                    return;
                }
                if let Some(incr) = stream.recv_flow.unclaimed_capacity() {
                    let frame = frame::WindowUpdate::new(stream.id, incr);
                    dst.buffer(frame.into())
                        .expect("invalid WINDOW_UPDATE frame");
                    stream
                        .recv_flow
                        .inc_window(incr)
                        .expect("unexpected flow control state");
                }
            })
        }
    }
    pub fn next_incoming(&mut self, store: &mut Store) -> Option<store::Key> {
        self.pending_accept.pop(store).map(|ptr| ptr.key())
    }
    pub fn poll_data(
        &mut self,
        cx: &Context,
        stream: &mut Stream,
    ) -> Poll<Option<Result<DataEvent, proto::Error>>> {
        match stream.pending_recv.pop_front(&mut self.buffer) {
            Some(Event::Data(data)) => Poll::Ready(Some(Ok(data))),
            Some(event) => {
                stream.pending_recv.push_front(&mut self.buffer, event);
                stream.notify_recv();
                Poll::Ready(None)
            }
            None => self.schedule_recv(cx, stream),
        }
    }
    pub fn poll_trailers(
        &mut self,
        cx: &Context,
        stream: &mut Stream,
    ) -> Poll<Option<Result<HeaderMap, proto::Error>>> {
        match stream.pending_recv.pop_front(&mut self.buffer) {
            Some(Event::Trailers(trailers)) => Poll::Ready(Some(Ok(trailers))),
            Some(event) => {
                stream.pending_recv.push_front(&mut self.buffer, event);
                stream.recv_task = Some(cx.waker().clone());
                Poll::Pending
            }
            None => self.schedule_recv(cx, stream),
        }
    }
    fn schedule_recv<T>(
        &mut self,
        cx: &Context,
        stream: &mut Stream,
    ) -> Poll<Option<Result<T, proto::Error>>> {
        if stream.state.ensure_recv_open()? {
            stream.recv_task = Some(cx.waker().clone());
            Poll::Pending
        } else {
            Poll::Ready(None)
        }
    }
}
impl Open {
    pub fn is_push_promise(&self) -> bool {
        matches!(*self, Self::PushPromise)
    }
}
impl<T> From<Error> for RecvHeaderBlockError<T> {
    fn from(err: Error) -> Self {
        RecvHeaderBlockError::State(err)
    }
}
