use super::lock::Lock;
use alloc::sync::Arc;
use core::fmt;
use core::pin::Pin;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::SeqCst;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll, Waker};
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
}
pub struct Sender<T> {
    inner: Arc<Inner<T>>,
}
impl<T> Unpin for Receiver<T> {}
impl<T> Unpin for Sender<T> {}
struct Inner<T> {
    complete: AtomicBool,
    data: Lock<Option<T>>,
    rx_task: Lock<Option<Waker>>,
}
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner::new());
    let receiver = Receiver {
        inner: inner.clone(),
    };
    let sender = Sender { inner };
    (sender, receiver)
}
impl<T> Inner<T> {
    fn new() -> Self {
        Self {
            complete: AtomicBool::new(false),
            data: Lock::new(None),
            rx_task: Lock::new(None),
        }
    }
    fn send(&self, t: T) -> Result<(), T> {
        if self.complete.load(SeqCst) {
            return Err(t);
        }
        if let Some(mut slot) = self.data.try_lock() {
            assert!(slot.is_none());
            *slot = Some(t);
            drop(slot);
            if self.complete.load(SeqCst) {
                if let Some(mut slot) = self.data.try_lock() {
                    if let Some(t) = slot.take() {
                        return Err(t);
                    }
                }
            }
            Ok(())
        } else {
            Err(t)
        }
    }

    fn is_canceled(&self) -> bool {
        self.complete.load(SeqCst)
    }
    fn drop_tx(&self) {
        self.complete.store(true, SeqCst);
        if let Some(mut slot) = self.rx_task.try_lock() {
            if let Some(task) = slot.take() {
                drop(slot);
                task.wake();
            }
        }
    }

    fn recv(&self, cx: &mut Context<'_>) -> Poll<Result<T, Canceled>> {
        let done = if self.complete.load(SeqCst) {
            true
        } else {
            let task = cx.waker().clone();
            match self.rx_task.try_lock() {
                Some(mut slot) => {
                    *slot = Some(task);
                    false
                }
                None => true,
            }
        };
        if done || self.complete.load(SeqCst) {
            if let Some(mut slot) = self.data.try_lock() {
                if let Some(data) = slot.take() {
                    return Poll::Ready(Ok(data));
                }
            }
            Poll::Ready(Err(Canceled))
        } else {
            Poll::Pending
        }
    }
    fn drop_rx(&self) {
        self.complete.store(true, SeqCst);
        if let Some(mut slot) = self.rx_task.try_lock() {
            let task = slot.take();
            drop(slot);
            drop(task);
        }
    }
}
impl<T> Sender<T> {
    pub fn send(self, t: T) -> Result<(), T> {
        self.inner.send(t)
    }

    pub fn is_canceled(&self) -> bool {
        self.inner.is_canceled()
    }
}
impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.inner.drop_tx()
    }
}
impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sender")
            .field("complete", &self.inner.complete)
            .finish()
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Canceled;
impl fmt::Display for Canceled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "oneshot canceled")
    }
}
impl std::error::Error for Canceled {}
impl<T> Receiver<T> {}
impl<T> Future for Receiver<T> {
    type Output = Result<T, Canceled>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, Canceled>> {
        self.inner.recv(cx)
    }
}
impl<T> FusedFuture for Receiver<T> {
    fn is_terminated(&self) -> bool {
        if self.inner.complete.load(SeqCst) {
            if let Some(slot) = self.inner.data.try_lock() {
                if slot.is_some() {
                    return false;
                }
            }
            true
        } else {
            false
        }
    }
}
impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.inner.drop_rx()
    }
}
impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Receiver")
            .field("complete", &self.inner.complete)
            .finish()
    }
}
#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/async_channel/oneshot.rs"]
mod tests;
