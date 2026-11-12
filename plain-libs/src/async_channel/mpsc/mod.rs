use self::queue::Queue;
use crate::atomic_waker::AtomicWaker;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll, Waker};
use std::fmt;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};
use std::thread;
mod queue;
pub struct Sender<T> {
    inner: Arc<Inner<T>>,
    sender_task: Arc<Mutex<SenderTask>>,
    maybe_parked: bool,
}
pub struct Receiver<T> {
    inner: Option<Arc<Inner<T>>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendError {
    kind: SendErrorKind,
}
#[derive(Clone, PartialEq, Eq)]
pub struct TrySendError<T> {
    err: SendError,
    val: T,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum SendErrorKind {
    Full,
    Disconnected,
}
impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            SendErrorKind::Full => "send failed because channel is full",
            SendErrorKind::Disconnected => "send failed because receiver is gone",
        })
    }
}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrySendError")
            .field("kind", &self.err.kind)
            .finish()
    }
}
impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.err, f)
    }
}

impl<T> TrySendError<T> {
    pub fn into_inner(self) -> T {
        self.val
    }
}
struct Inner<T> {
    buffer: usize,
    state: AtomicUsize,
    message_queue: Queue<T>,
    parked_queue: Queue<Arc<Mutex<SenderTask>>>,
    num_senders: AtomicUsize,
    recv_task: AtomicWaker,
}
#[derive(Clone, Copy)]
struct State {
    is_open: bool,
    num_messages: usize,
}
const OPEN_MASK: usize = usize::MAX - (usize::MAX >> 1);
const INIT_STATE: usize = OPEN_MASK;
const MAX_CAPACITY: usize = !(OPEN_MASK);
const MAX_BUFFER: usize = MAX_CAPACITY >> 1;
struct SenderTask {
    task: Option<Waker>,
    is_parked: bool,
}
impl SenderTask {
    fn new() -> Self {
        Self {
            task: None,
            is_parked: false,
        }
    }
    #[inline(never)]
    fn notify(&mut self) {
        self.is_parked = false;
        if let Some(task) = self.task.take() {
            task.wake();
        }
    }
}
pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
    assert!(buffer < MAX_BUFFER, "requested buffer size too large");
    let inner = Arc::new(Inner {
        buffer,
        state: AtomicUsize::new(INIT_STATE),
        message_queue: Queue::new(),
        parked_queue: Queue::new(),
        num_senders: AtomicUsize::new(1),
        recv_task: AtomicWaker::new(),
    });
    let tx = Sender {
        inner: inner.clone(),
        sender_task: Arc::new(Mutex::new(SenderTask::new())),
        maybe_parked: false,
    };
    let rx = Receiver { inner: Some(inner) };
    (tx, rx)
}
impl<T> Sender<T> {
    pub fn start_send(&mut self, msg: T) -> Result<(), SendError> {
        self.try_send(msg).map_err(|error| error.err)
    }

    pub fn try_send(&mut self, msg: T) -> Result<(), TrySendError<T>> {
        if !self.poll_unparked(None).is_ready() {
            return Err(TrySendError {
                err: SendError {
                    kind: SendErrorKind::Full,
                },
                val: msg,
            });
        }

        let park_self = match self.inc_num_messages() {
            Some(num_messages) => num_messages > self.inner.buffer,
            None => {
                return Err(TrySendError {
                    err: SendError {
                        kind: SendErrorKind::Disconnected,
                    },
                    val: msg,
                });
            }
        };
        if park_self {
            self.park();
        }
        self.queue_push_and_signal(msg);
        Ok(())
    }

    fn queue_push_and_signal(&self, msg: T) {
        self.inner.message_queue.push(msg);
        self.inner.recv_task.wake_channel();
    }
    fn inc_num_messages(&self) -> Option<usize> {
        let mut curr = self.inner.state.load(SeqCst);
        loop {
            let mut state = decode_state(curr);
            if !state.is_open {
                return None;
            }
            assert!(
                state.num_messages < MAX_CAPACITY,
                "buffer space \
                    exhausted; sending this messages would overflow the state"
            );
            state.num_messages += 1;
            let next = encode_state(&state);
            match self
                .inner
                .state
                .compare_exchange(curr, next, SeqCst, SeqCst)
            {
                Ok(_) => return Some(state.num_messages),
                Err(actual) => curr = actual,
            }
        }
    }
    fn park(&mut self) {
        {
            let mut sender = self.sender_task.lock().unwrap();
            sender.task = None;
            sender.is_parked = true;
        }
        let t = self.sender_task.clone();
        self.inner.parked_queue.push(t);
        let state = decode_state(self.inner.state.load(SeqCst));
        self.maybe_parked = state.is_open;
    }
    pub fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), SendError>> {
        let state = decode_state(self.inner.state.load(SeqCst));
        if !state.is_open {
            return Poll::Ready(Err(SendError {
                kind: SendErrorKind::Disconnected,
            }));
        }
        self.poll_unparked(Some(cx)).map(Ok)
    }

    pub fn is_closed(&self) -> bool {
        !decode_state(self.inner.state.load(SeqCst)).is_open
    }

    pub fn close_channel(&self) {
        self.inner.set_closed();
        self.inner.recv_task.wake_channel();
    }
    fn poll_unparked(&mut self, cx: Option<&mut Context<'_>>) -> Poll<()> {
        if self.maybe_parked {
            let mut task = self.sender_task.lock().unwrap();
            if !task.is_parked {
                self.maybe_parked = false;
                return Poll::Ready(());
            }
            task.task = cx.map(|cx| cx.waker().clone());
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}
impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        let mut curr = self.inner.num_senders.load(SeqCst);
        loop {
            if curr == self.inner.max_senders() {
                panic!("cannot clone `Sender` -- too many outstanding senders");
            }
            debug_assert!(curr < self.inner.max_senders());
            let next = curr + 1;
            match self
                .inner
                .num_senders
                .compare_exchange(curr, next, SeqCst, SeqCst)
            {
                Ok(_) => {
                    return Self {
                        inner: self.inner.clone(),
                        sender_task: Arc::new(Mutex::new(SenderTask::new())),
                        maybe_parked: false,
                    };
                }
                Err(actual) => curr = actual,
            }
        }
    }
}
impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let prev = self.inner.num_senders.fetch_sub(1, SeqCst);
        if prev == 1 {
            self.close_channel();
        }
    }
}
impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sender")
            .field("closed", &self.is_closed())
            .finish()
    }
}
impl<T> Receiver<T> {
    fn close(&mut self) {
        if let Some(inner) = &mut self.inner {
            inner.set_closed();
            while let Some(task) = unsafe { inner.parked_queue.pop_spin() } {
                task.lock().unwrap().notify();
            }
        }
    }

    fn next_message(&mut self) -> Poll<Option<T>> {
        let inner = match self.inner.as_mut() {
            None => return Poll::Ready(None),
            Some(inner) => inner,
        };
        match unsafe { inner.message_queue.pop_spin() } {
            Some(msg) => {
                self.unpark_one();
                self.dec_num_messages();
                Poll::Ready(Some(msg))
            }
            None => {
                let state = decode_state(inner.state.load(SeqCst));
                if state.is_closed() {
                    self.inner = None;
                    Poll::Ready(None)
                } else {
                    Poll::Pending
                }
            }
        }
    }
    fn unpark_one(&mut self) {
        if let Some(inner) = &mut self.inner {
            if let Some(task) = unsafe { inner.parked_queue.pop_spin() } {
                task.lock().unwrap().notify();
            }
        }
    }
    fn dec_num_messages(&self) {
        if let Some(inner) = &self.inner {
            inner.state.fetch_sub(1, SeqCst);
        }
    }
}

impl<T> FusedStream for Receiver<T> {
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}
impl<T> Stream for Receiver<T> {
    type Item = T;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        match self.next_message() {
            Poll::Ready(msg) => {
                if msg.is_none() {
                    self.inner = None;
                }
                Poll::Ready(msg)
            }
            Poll::Pending => {
                self.inner.as_ref().unwrap().recv_task.register(cx.waker());
                self.next_message()
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if let Some(inner) = &self.inner {
            decode_state(inner.state.load(SeqCst)).size_hint()
        } else {
            (0, Some(0))
        }
    }
}
impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.close();
        if self.inner.is_some() {
            loop {
                match self.next_message() {
                    Poll::Ready(Some(_)) => {}
                    Poll::Ready(None) => break,
                    Poll::Pending => {
                        let state = decode_state(self.inner.as_ref().unwrap().state.load(SeqCst));
                        if state.is_closed() {
                            break;
                        }
                        thread::yield_now();
                    }
                }
            }
        }
    }
}
impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let closed = if let Some(ref inner) = self.inner {
            decode_state(inner.state.load(SeqCst)).is_closed()
        } else {
            false
        };
        f.debug_struct("Receiver").field("closed", &closed).finish()
    }
}
impl<T> Inner<T> {
    fn max_senders(&self) -> usize {
        MAX_CAPACITY - self.buffer
    }
    fn set_closed(&self) {
        let curr = self.state.load(SeqCst);
        if !decode_state(curr).is_open {
            return;
        }
        self.state.fetch_and(!OPEN_MASK, SeqCst);
    }
}
// Message publication uses SeqCst state and Release/Acquire queues with one receiver.
unsafe impl<T: Send> Send for Inner<T> {}
unsafe impl<T: Send> Sync for Inner<T> {}
impl State {
    fn is_closed(&self) -> bool {
        !self.is_open && self.num_messages == 0
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_open {
            (self.num_messages, None)
        } else {
            (self.num_messages, Some(self.num_messages))
        }
    }
}
fn decode_state(num: usize) -> State {
    State {
        is_open: num & OPEN_MASK == OPEN_MASK,
        num_messages: num & MAX_CAPACITY,
    }
}
fn encode_state(state: &State) -> usize {
    let mut num = state.num_messages;
    if state.is_open {
        num |= OPEN_MASK;
    }
    num
}
#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/async_channel/mpsc.rs"]
mod tests;

impl std::error::Error for SendError {}
impl<T: core::any::Any> std::error::Error for TrySendError<T> {}
impl<T> Unpin for Receiver<T> {}
impl<T> Unpin for Sender<T> {}
