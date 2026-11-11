use crate::try_lock::TryLock;
use std::fmt;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::task::{self, Poll, Waker};
pub fn new() -> (Giver, Taker) {
    let inner = Arc::new(Inner {
        state: AtomicUsize::new(usize::from(State::Idle)),
        task: TryLock::new(None),
    });
    let inner2 = inner.clone();
    (Giver { inner }, Taker { inner: inner2 })
}
pub struct Giver {
    inner: Arc<Inner>,
}
pub struct Taker {
    inner: Arc<Inner>,
}
#[derive(Clone)]
pub struct SharedGiver {
    inner: Arc<Inner>,
}
pub struct Closed {
    _inner: (),
}
#[derive(Clone, Copy, Debug)]
enum State {
    Idle,
    Want,
    Give,
    Closed,
}
impl From<State> for usize {
    fn from(s: State) -> usize {
        match s {
            State::Idle => 0,
            State::Want => 1,
            State::Give => 2,
            State::Closed => 3,
        }
    }
}
impl From<usize> for State {
    fn from(num: usize) -> State {
        match num {
            0 => State::Idle,
            1 => State::Want,
            2 => State::Give,
            3 => State::Closed,
            _ => unreachable!("unknown state: {}", num),
        }
    }
}
struct Inner {
    state: AtomicUsize,
    task: TryLock<Option<Waker>>,
}
impl Giver {
    pub fn want(&mut self) -> impl Future<Output = Result<(), Closed>> + '_ {
        Want(self)
    }
    pub fn poll_want(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Closed>> {
        loop {
            let state = self.inner.state.load(SeqCst).into();
            match state {
                State::Want => {
                    return Poll::Ready(Ok(()));
                }
                State::Closed => {
                    return Poll::Ready(Err(Closed { _inner: () }));
                }
                State::Idle | State::Give => {
                    if let Some(mut locked) = self.inner.task.try_lock_explicit(SeqCst, SeqCst) {
                        let old = self.inner.state.compare_exchange(
                            state.into(),
                            usize::from(State::Give),
                            SeqCst,
                            SeqCst,
                        );
                        if old == Ok(state.into()) {
                            let park = locked
                                .as_ref()
                                .map(|w| !w.will_wake(cx.waker()))
                                .unwrap_or(true);
                            if park {
                                let old = mem::replace(&mut *locked, Some(cx.waker().clone()));
                                drop(locked);
                                if let Some(prev_task) = old {
                                    prev_task.wake();
                                };
                            }
                            return Poll::Pending;
                        }
                    } else {
                    }
                }
            }
        }
    }
    #[inline]
    pub fn give(&self) -> bool {
        let old = self.inner.state.compare_exchange(
            usize::from(State::Want),
            usize::from(State::Idle),
            SeqCst,
            SeqCst,
        );
        old == Ok(usize::from(State::Want))
    }
    #[inline]
    pub fn is_wanting(&self) -> bool {
        self.inner.state.load(SeqCst) == usize::from(State::Want)
    }
    #[inline]
    pub fn is_canceled(&self) -> bool {
        self.inner.state.load(SeqCst) == usize::from(State::Closed)
    }
    #[inline]
    pub fn shared(self) -> SharedGiver {
        SharedGiver { inner: self.inner }
    }
}
impl fmt::Debug for Giver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Giver")
            .field("state", &self.inner.state())
            .finish()
    }
}
impl SharedGiver {
    #[inline]
    pub fn is_wanting(&self) -> bool {
        self.inner.state.load(SeqCst) == usize::from(State::Want)
    }
    #[inline]
    pub fn is_canceled(&self) -> bool {
        self.inner.state.load(SeqCst) == usize::from(State::Closed)
    }
}
impl fmt::Debug for SharedGiver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedGiver")
            .field("state", &self.inner.state())
            .finish()
    }
}
impl Taker {
    #[inline]
    pub fn cancel(&mut self) {
        self.signal(State::Closed)
    }
    #[inline]
    pub fn want(&mut self) {
        debug_assert!(
            self.inner.state.load(SeqCst) != usize::from(State::Closed),
            "want called after cancel"
        );
        self.signal(State::Want)
    }
    #[inline]
    fn signal(&mut self, state: State) {
        let old_state = self.inner.state.swap(state.into(), SeqCst).into();
        match old_state {
            State::Idle | State::Want | State::Closed => (),
            State::Give => loop {
                if let Some(mut locked) = self.inner.task.try_lock_explicit(SeqCst, SeqCst) {
                    if let Some(task) = locked.take() {
                        drop(locked);
                        task.wake();
                    }
                    return;
                } else {
                }
            },
        }
    }
}
impl Drop for Taker {
    #[inline]
    fn drop(&mut self) {
        self.signal(State::Closed);
    }
}
impl fmt::Debug for Taker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Taker")
            .field("state", &self.inner.state())
            .finish()
    }
}
impl fmt::Debug for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Closed").finish()
    }
}
impl Inner {
    #[inline]
    fn state(&self) -> State {
        self.state.load(SeqCst).into()
    }
}
struct Want<'a>(&'a mut Giver);
impl Future for Want<'_> {
    type Output = Result<(), Closed>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        self.0.poll_want(cx)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/want.rs"]
mod tests;
