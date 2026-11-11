use crate::http::{Request, Response};
use crate::http_body::Body;
use crate::hyper::{body::Incoming, proto::h2::client::ResponseFutMap};
use pin_project_lite::pin_project;
use std::task::{Context, Poll};
use std::{future::Future, pin::Pin};
use tokio::sync::{mpsc, oneshot};
pub(crate) type RetryPromise<T, U> = oneshot::Receiver<Result<U, TrySendError<T>>>;
pub(crate) type Promise<T> = oneshot::Receiver<Result<T, crate::hyper::Error>>;
#[derive(Debug)]
pub struct TrySendError<T> {
    pub(crate) error: crate::hyper::Error,
    pub(crate) message: Option<T>,
}
pub(crate) fn channel<T, U>() -> (Sender<T, U>, Receiver<T, U>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let (giver, taker) = crate::want::new();
    let tx = Sender {
        buffered_once: false,
        giver,
        inner: tx,
    };
    let rx = Receiver { inner: rx, taker };
    (tx, rx)
}
pub(crate) struct Sender<T, U> {
    buffered_once: bool,
    giver: crate::want::Giver,
    inner: mpsc::UnboundedSender<Envelope<T, U>>,
}
pub(crate) struct UnboundedSender<T, U> {
    giver: crate::want::SharedGiver,
    inner: mpsc::UnboundedSender<Envelope<T, U>>,
}
impl<T, U> Sender<T, U> {
    pub(crate) fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<crate::hyper::Result<()>> {
        self.giver
            .poll_want(cx)
            .map_err(|_| crate::hyper::Error::new_closed())
    }
    pub(crate) fn is_ready(&self) -> bool {
        self.giver.is_wanting()
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.giver.is_canceled()
    }
    fn can_send(&mut self) -> bool {
        if self.giver.give() || !self.buffered_once {
            self.buffered_once = true;
            true
        } else {
            false
        }
    }
    pub(crate) fn try_send(&mut self, val: T) -> Result<RetryPromise<T, U>, T> {
        if !self.can_send() {
            return Err(val);
        }
        let (tx, rx) = oneshot::channel();
        self.inner
            .send(Envelope(Some((val, Callback::Retry(Some(tx))))))
            .map(move |_| rx)
            .map_err(|mut e| (e.0).0.take().expect("envelope not dropped").0)
    }
    pub(crate) fn send(&mut self, val: T) -> Result<Promise<U>, T> {
        if !self.can_send() {
            return Err(val);
        }
        let (tx, rx) = oneshot::channel();
        self.inner
            .send(Envelope(Some((val, Callback::NoRetry(Some(tx))))))
            .map(move |_| rx)
            .map_err(|mut e| (e.0).0.take().expect("envelope not dropped").0)
    }
    pub(crate) fn unbound(self) -> UnboundedSender<T, U> {
        UnboundedSender {
            giver: self.giver.shared(),
            inner: self.inner,
        }
    }
}
impl<T, U> UnboundedSender<T, U> {
    pub(crate) fn is_ready(&self) -> bool {
        !self.giver.is_canceled()
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.giver.is_canceled()
    }
    pub(crate) fn try_send(&mut self, val: T) -> Result<RetryPromise<T, U>, T> {
        let (tx, rx) = oneshot::channel();
        self.inner
            .send(Envelope(Some((val, Callback::Retry(Some(tx))))))
            .map(move |_| rx)
            .map_err(|mut e| (e.0).0.take().expect("envelope not dropped").0)
    }
    pub(crate) fn send(&mut self, val: T) -> Result<Promise<U>, T> {
        let (tx, rx) = oneshot::channel();
        self.inner
            .send(Envelope(Some((val, Callback::NoRetry(Some(tx))))))
            .map(move |_| rx)
            .map_err(|mut e| (e.0).0.take().expect("envelope not dropped").0)
    }
}
impl<T, U> Clone for UnboundedSender<T, U> {
    fn clone(&self) -> Self {
        UnboundedSender {
            giver: self.giver.clone(),
            inner: self.inner.clone(),
        }
    }
}
pub(crate) struct Receiver<T, U> {
    inner: mpsc::UnboundedReceiver<Envelope<T, U>>,
    taker: crate::want::Taker,
}
impl<T, U> Receiver<T, U> {
    pub(crate) fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<(T, Callback<T, U>)>> {
        match self.inner.poll_recv(cx) {
            Poll::Ready(item) => {
                Poll::Ready(item.map(|mut env| env.0.take().expect("envelope not dropped")))
            }
            Poll::Pending => {
                self.taker.want();
                Poll::Pending
            }
        }
    }
    pub(crate) fn close(&mut self) {
        self.taker.cancel();
        self.inner.close();
    }
    pub(crate) fn try_recv(&mut self) -> Option<(T, Callback<T, U>)> {
        match crate::hyper::common::task::now_or_never(self.inner.recv()) {
            Some(Some(mut env)) => env.0.take(),
            _ => None,
        }
    }
}
impl<T, U> Drop for Receiver<T, U> {
    fn drop(&mut self) {
        self.taker.cancel();
    }
}
struct Envelope<T, U>(Option<(T, Callback<T, U>)>);
impl<T, U> Drop for Envelope<T, U> {
    fn drop(&mut self) {
        if let Some((val, cb)) = self.0.take() {
            cb.send(Err(TrySendError {
                error: crate::hyper::Error::new_canceled().with("connection closed"),
                message: Some(val),
            }));
        }
    }
}
pub(crate) enum Callback<T, U> {
    Retry(Option<oneshot::Sender<Result<U, TrySendError<T>>>>),
    NoRetry(Option<oneshot::Sender<Result<U, crate::hyper::Error>>>),
}
impl<T, U> Drop for Callback<T, U> {
    fn drop(&mut self) {
        match self {
            Callback::Retry(tx) => {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(Err(TrySendError {
                        error: dispatch_gone(),
                        message: None,
                    }));
                }
            }
            Callback::NoRetry(tx) => {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(Err(dispatch_gone()));
                }
            }
        }
    }
}
#[cold]
fn dispatch_gone() -> crate::hyper::Error {
    crate::hyper::Error::new_user_dispatch_gone().with(if std::thread::panicking() {
        "user code panicked"
    } else {
        "runtime dropped the dispatch task"
    })
}
impl<T, U> Callback<T, U> {
    pub(crate) fn is_canceled(&self) -> bool {
        match self {
            Callback::Retry(Some(tx)) => tx.is_closed(),
            Callback::NoRetry(Some(tx)) => tx.is_closed(),
            _ => unreachable!(),
        }
    }
    pub(crate) fn poll_canceled(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match self {
            Callback::Retry(Some(tx)) => tx.poll_closed(cx),
            Callback::NoRetry(Some(tx)) => tx.poll_closed(cx),
            _ => unreachable!(),
        }
    }
    pub(crate) fn send(mut self, val: Result<U, TrySendError<T>>) {
        match &mut self {
            Callback::Retry(tx) => {
                let _ = tx
                    .take()
                    .expect("callback sender not dropped before send")
                    .send(val);
            }
            Callback::NoRetry(tx) => {
                let _ = tx
                    .take()
                    .expect("callback sender not dropped before send")
                    .send(val.map_err(|e| e.error));
            }
        }
    }
}
impl<T> TrySendError<T> {
    pub fn take_message(&mut self) -> Option<T> {
        self.message.take()
    }
    pub fn message(&self) -> Option<&T> {
        self.message.as_ref()
    }
    pub fn into_error(self) -> crate::hyper::Error {
        self.error
    }
    pub fn error(&self) -> &crate::hyper::Error {
        &self.error
    }
}
pin_project! { pub struct SendWhen < B , E > where B : Body , B : 'static , { #[pin] pub (crate) when : ResponseFutMap < B , E >, #[pin] pub (crate) call_back : Option < Callback < Request < B >, Response < Incoming >>>, } }
impl<B, E> Future for SendWhen<B, E>
where
    B: Body + 'static,
    E: crate::hyper::rt::bounds::Http2UpgradedExec<B::Data>,
{
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let mut call_back = this.call_back.take().expect("polled after complete");
        match Pin::new(&mut this.when).poll(cx) {
            Poll::Ready(Ok(res)) => {
                call_back.send(Ok(res));
                Poll::Ready(())
            }
            Poll::Pending => {
                match call_back.poll_canceled(cx) {
                    Poll::Ready(v) => v,
                    Poll::Pending => {
                        this.call_back.set(Some(call_back));
                        return Poll::Pending;
                    }
                }
                trace!("send_when canceled");
                this.when.as_mut().cancel();
                Poll::Ready(())
            }
            Poll::Ready(Err((error, message))) => {
                call_back.send(Err(TrySendError { error, message }));
                Poll::Ready(())
            }
        }
    }
}
