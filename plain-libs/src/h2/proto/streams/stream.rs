use super::*;
use crate::h2::Reason;
use std::fmt;
use std::task::{Context, Waker};
use std::time::Instant;
pub(super) struct Stream {
    pub id: StreamId,
    pub state: State,
    pub is_counted: bool,
    pub ref_count: usize,
    pub next_pending_send: Option<store::Key>,
    pub is_pending_send: bool,
    pub send_flow: FlowControl,
    pub requested_send_capacity: WindowSize,
    pub buffered_send_data: usize,
    send_task: Option<Waker>,
    pub pending_send: buffer::Deque,
    pub next_pending_send_capacity: Option<store::Key>,
    pub is_pending_send_capacity: bool,
    pub send_capacity_inc: bool,
    pub next_open: Option<store::Key>,
    pub is_pending_open: bool,
    pub is_pending_push: bool,
    pub next_pending_accept: Option<store::Key>,
    pub is_pending_accept: bool,
    pub recv_flow: FlowControl,
    pub in_flight_recv_data: WindowSize,
    pub next_window_update: Option<store::Key>,
    pub is_pending_window_update: bool,
    pub reset_at: Option<Instant>,
    pub next_reset_expire: Option<store::Key>,
    pub pending_recv: buffer::Deque,
    pub is_recv: bool,
    pub recv_task: Option<Waker>,
    pub push_task: Option<Waker>,
    pub pending_push_promises: store::Queue<NextAccept>,
    pub content_length: ContentLength,
}
#[derive(Debug)]
pub enum ContentLength {
    Omitted,
    Head,
    Remaining(u64),
}
#[derive(Debug)]
pub(super) struct NextAccept;
#[derive(Debug)]
pub(super) struct NextSend;
#[derive(Debug)]
pub(super) struct NextSendCapacity;
#[derive(Debug)]
pub(super) struct NextWindowUpdate;
#[derive(Debug)]
pub(super) struct NextOpen;
#[derive(Debug)]
pub(super) struct NextResetExpire;
impl Stream {
    pub fn new(id: StreamId, init_send_window: WindowSize, init_recv_window: WindowSize) -> Stream {
        let mut send_flow = FlowControl::new();
        let mut recv_flow = FlowControl::new();
        recv_flow
            .inc_window(init_recv_window)
            .expect("invalid initial receive window");
        let _res = recv_flow.assign_capacity(init_recv_window);
        debug_assert!(_res.is_ok());
        send_flow
            .inc_window(init_send_window)
            .expect("invalid initial send window size");
        Stream {
            id,
            state: State::default(),
            ref_count: 0,
            is_counted: false,
            next_pending_send: None,
            is_pending_send: false,
            send_flow,
            requested_send_capacity: 0,
            buffered_send_data: 0,
            send_task: None,
            pending_send: buffer::Deque::new(),
            is_pending_send_capacity: false,
            next_pending_send_capacity: None,
            send_capacity_inc: false,
            is_pending_open: false,
            next_open: None,
            is_pending_push: false,
            next_pending_accept: None,
            is_pending_accept: false,
            recv_flow,
            in_flight_recv_data: 0,
            next_window_update: None,
            is_pending_window_update: false,
            reset_at: None,
            next_reset_expire: None,
            pending_recv: buffer::Deque::new(),
            is_recv: true,
            recv_task: None,
            push_task: None,
            pending_push_promises: store::Queue::new(),
            content_length: ContentLength::Omitted,
        }
    }
    pub fn ref_inc(&mut self) {
        assert!(self.ref_count < usize::MAX);
        self.ref_count += 1;
    }
    pub fn ref_dec(&mut self) {
        assert!(self.ref_count > 0);
        self.ref_count -= 1;
    }
    pub fn is_pending_reset_expiration(&self) -> bool {
        self.reset_at.is_some()
    }
    pub fn is_send_ready(&self) -> bool {
        !self.is_pending_open && !self.is_pending_push
    }
    pub fn is_closed(&self) -> bool {
        self.state.is_closed() && self.pending_send.is_empty() && self.buffered_send_data == 0
    }
    pub fn is_released(&self) -> bool {
        self.is_closed()
            && self.ref_count == 0
            && !self.is_pending_send
            && !self.is_pending_send_capacity
            && !self.is_pending_accept
            && !self.is_pending_window_update
            && !self.is_pending_open
            && self.reset_at.is_none()
    }
    pub fn is_canceled_interest(&self) -> bool {
        self.ref_count == 0 && !self.state.is_closed()
    }
    pub fn capacity(&self, max_buffer_size: usize) -> WindowSize {
        let available = self.send_flow.available().as_size() as usize;
        let buffered = self.buffered_send_data;
        available.min(max_buffer_size).saturating_sub(buffered) as WindowSize
    }
    pub fn assign_capacity(&mut self, capacity: WindowSize, max_buffer_size: usize) {
        let prev_capacity = self.capacity(max_buffer_size);
        debug_assert!(capacity > 0);
        let _res = self.send_flow.assign_capacity(capacity);
        debug_assert!(_res.is_ok());
        tracing::trace!(
            "  assigned capacity to stream; available={}; buffered={}; id={:?}; max_buffer_size={} prev={}",
            self.send_flow.available(),
            self.buffered_send_data,
            self.id,
            max_buffer_size,
            prev_capacity,
        );
        if prev_capacity < self.capacity(max_buffer_size) {
            self.notify_capacity();
        }
    }
    pub fn send_data(&mut self, len: WindowSize, max_buffer_size: usize) {
        let prev_capacity = self.capacity(max_buffer_size);
        let _res = self.send_flow.send_data(len);
        debug_assert!(_res.is_ok());
        debug_assert!(self.buffered_send_data >= len as usize);
        self.buffered_send_data -= len as usize;
        self.requested_send_capacity -= len;
        tracing::trace!(
            "  sent stream data; available={}; buffered={}; id={:?}; max_buffer_size={} prev={}",
            self.send_flow.available(),
            self.buffered_send_data,
            self.id,
            max_buffer_size,
            prev_capacity,
        );
        if prev_capacity < self.capacity(max_buffer_size) {
            self.notify_capacity();
        }
    }
    pub fn notify_capacity(&mut self) {
        self.send_capacity_inc = true;
        tracing::trace!("  notifying task");
        self.notify_send();
    }
    pub fn dec_content_length(&mut self, len: usize) -> Result<(), ()> {
        match self.content_length {
            ContentLength::Remaining(ref mut rem) => match rem.checked_sub(len as u64) {
                Some(val) => *rem = val,
                None => return Err(()),
            },
            ContentLength::Head => {
                if len != 0 {
                    return Err(());
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub fn ensure_content_length_zero(&self) -> Result<(), ()> {
        match self.content_length {
            ContentLength::Remaining(0) => Ok(()),
            ContentLength::Remaining(_) => Err(()),
            _ => Ok(()),
        }
    }
    pub fn notify_send(&mut self) {
        if let Some(task) = self.send_task.take() {
            task.wake();
        }
    }
    pub fn wait_send(&mut self, cx: &Context) {
        self.send_task = Some(cx.waker().clone());
    }
    pub fn notify_recv(&mut self) {
        if let Some(task) = self.recv_task.take() {
            task.wake();
        }
    }
    pub(super) fn notify_push(&mut self) {
        if let Some(task) = self.push_task.take() {
            task.wake();
        }
    }
    pub(super) fn set_reset(&mut self, reason: Reason, initiator: Initiator) {
        self.state.set_reset(self.id, reason, initiator);
        self.notify_send();
        self.notify_push();
        self.notify_recv();
    }
}
impl fmt::Debug for Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stream")
            .field("id", &self.id)
            .field("state", &self.state)
            .field("is_counted", &self.is_counted)
            .field("ref_count", &self.ref_count)
            .h2_field_some("next_pending_send", &self.next_pending_send)
            .h2_field_if("is_pending_send", &self.is_pending_send)
            .field("send_flow", &self.send_flow)
            .field("requested_send_capacity", &self.requested_send_capacity)
            .field("buffered_send_data", &self.buffered_send_data)
            .h2_field_some("send_task", &self.send_task.as_ref().map(|_| ()))
            .h2_field_if_then(
                "pending_send",
                !self.pending_send.is_empty(),
                &self.pending_send,
            )
            .h2_field_some(
                "next_pending_send_capacity",
                &self.next_pending_send_capacity,
            )
            .h2_field_if("is_pending_send_capacity", &self.is_pending_send_capacity)
            .h2_field_if("send_capacity_inc", &self.send_capacity_inc)
            .h2_field_some("next_open", &self.next_open)
            .h2_field_if("is_pending_open", &self.is_pending_open)
            .h2_field_if("is_pending_push", &self.is_pending_push)
            .h2_field_some("next_pending_accept", &self.next_pending_accept)
            .h2_field_if("is_pending_accept", &self.is_pending_accept)
            .field("recv_flow", &self.recv_flow)
            .field("in_flight_recv_data", &self.in_flight_recv_data)
            .h2_field_some("next_window_update", &self.next_window_update)
            .h2_field_if("is_pending_window_update", &self.is_pending_window_update)
            .h2_field_some("reset_at", &self.reset_at)
            .h2_field_some("next_reset_expire", &self.next_reset_expire)
            .h2_field_if_then(
                "pending_recv",
                !self.pending_recv.is_empty(),
                &self.pending_recv,
            )
            .h2_field_if("is_recv", &self.is_recv)
            .h2_field_some("recv_task", &self.recv_task.as_ref().map(|_| ()))
            .h2_field_some("push_task", &self.push_task.as_ref().map(|_| ()))
            .h2_field_if_then(
                "pending_push_promises",
                !self.pending_push_promises.is_empty(),
                &self.pending_push_promises,
            )
            .field("content_length", &self.content_length)
            .finish()
    }
}
impl store::Next for NextAccept {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_pending_accept
    }
    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_pending_accept = key;
    }
    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_pending_accept.take()
    }
    fn is_queued(stream: &Stream) -> bool {
        stream.is_pending_accept
    }
    fn set_queued(stream: &mut Stream, val: bool) {
        stream.is_pending_accept = val;
    }
}
impl store::Next for NextSend {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_pending_send
    }
    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_pending_send = key;
    }
    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_pending_send.take()
    }
    fn is_queued(stream: &Stream) -> bool {
        stream.is_pending_send
    }
    fn set_queued(stream: &mut Stream, val: bool) {
        if val {
            debug_assert!(!stream.is_pending_open);
        }
        stream.is_pending_send = val;
    }
}
impl store::Next for NextSendCapacity {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_pending_send_capacity
    }
    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_pending_send_capacity = key;
    }
    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_pending_send_capacity.take()
    }
    fn is_queued(stream: &Stream) -> bool {
        stream.is_pending_send_capacity
    }
    fn set_queued(stream: &mut Stream, val: bool) {
        stream.is_pending_send_capacity = val;
    }
}
impl store::Next for NextWindowUpdate {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_window_update
    }
    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_window_update = key;
    }
    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_window_update.take()
    }
    fn is_queued(stream: &Stream) -> bool {
        stream.is_pending_window_update
    }
    fn set_queued(stream: &mut Stream, val: bool) {
        stream.is_pending_window_update = val;
    }
}
impl store::Next for NextOpen {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_open
    }
    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_open = key;
    }
    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_open.take()
    }
    fn is_queued(stream: &Stream) -> bool {
        stream.is_pending_open
    }
    fn set_queued(stream: &mut Stream, val: bool) {
        if val {
            debug_assert!(!stream.is_pending_send);
        }
        stream.is_pending_open = val;
    }
}
impl store::Next for NextResetExpire {
    fn next(stream: &Stream) -> Option<store::Key> {
        stream.next_reset_expire
    }
    fn set_next(stream: &mut Stream, key: Option<store::Key>) {
        stream.next_reset_expire = key;
    }
    fn take_next(stream: &mut Stream) -> Option<store::Key> {
        stream.next_reset_expire.take()
    }
    fn is_queued(stream: &Stream) -> bool {
        stream.reset_at.is_some()
    }
    fn set_queued(stream: &mut Stream, val: bool) {
        if val {
            stream.reset_at = Some(Instant::now());
        } else {
            stream.reset_at = None;
        }
    }
}
impl ContentLength {
    pub fn is_head(&self) -> bool {
        matches!(*self, Self::Head)
    }
}
