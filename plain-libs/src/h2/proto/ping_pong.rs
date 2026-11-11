use crate::h2::codec::Codec;
use crate::h2::frame::Ping;
use crate::h2::proto::{self, PingPayload};
use atomic_waker::AtomicWaker;
use bytes::Buf;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use tokio::io::AsyncWrite;
#[derive(Debug)]
pub(crate) struct PingPong {
    pending_ping: Option<PendingPing>,
    pending_pong: Option<PingPayload>,
    user_pings: Option<UserPingsRx>,
}
#[derive(Debug)]
pub(crate) struct UserPings(Arc<UserPingsInner>);
#[derive(Debug)]
struct UserPingsRx(Arc<UserPingsInner>);
#[derive(Debug)]
struct UserPingsInner {
    state: AtomicUsize,
    ping_task: AtomicWaker,
    pong_task: AtomicWaker,
}
#[derive(Debug)]
struct PendingPing {
    payload: PingPayload,
    sent: bool,
}
#[derive(Debug)]
pub(crate) enum ReceivedPing {
    MustAck,
    Unknown,
    Shutdown,
}
const USER_STATE_EMPTY: usize = 0;
const USER_STATE_PENDING_PING: usize = 1;
const USER_STATE_PENDING_PONG: usize = 2;
const USER_STATE_RECEIVED_PONG: usize = 3;
const USER_STATE_CLOSED: usize = 4;
impl PingPong {
    pub(crate) fn new() -> Self {
        PingPong {
            pending_ping: None,
            pending_pong: None,
            user_pings: None,
        }
    }
    pub(crate) fn take_user_pings(&mut self) -> Option<UserPings> {
        if self.user_pings.is_some() {
            return None;
        }
        let user_pings = Arc::new(UserPingsInner {
            state: AtomicUsize::new(USER_STATE_EMPTY),
            ping_task: AtomicWaker::new(),
            pong_task: AtomicWaker::new(),
        });
        self.user_pings = Some(UserPingsRx(user_pings.clone()));
        Some(UserPings(user_pings))
    }
    pub(crate) fn ping_shutdown(&mut self) {
        assert!(self.pending_ping.is_none());
        self.pending_ping = Some(PendingPing {
            payload: Ping::SHUTDOWN,
            sent: false,
        });
    }
    pub(crate) fn recv_ping(&mut self, ping: Ping) -> ReceivedPing {
        assert!(self.pending_pong.is_none());
        if ping.is_ack() {
            if let Some(pending) = self.pending_ping.take() {
                if &pending.payload == ping.payload() {
                    assert_eq!(
                        &pending.payload,
                        &Ping::SHUTDOWN,
                        "pending_ping should be for shutdown",
                    );
                    tracing::trace!("recv PING SHUTDOWN ack");
                    return ReceivedPing::Shutdown;
                }
                self.pending_ping = Some(pending);
            }
            if let Some(ref users) = self.user_pings {
                if ping.payload() == &Ping::USER && users.receive_pong() {
                    tracing::trace!("recv PING USER ack");
                    return ReceivedPing::Unknown;
                }
            }
            tracing::warn!("recv PING ack that we never sent: {:?}", ping);
            ReceivedPing::Unknown
        } else {
            self.pending_pong = Some(ping.into_payload());
            ReceivedPing::MustAck
        }
    }
    pub(crate) fn send_pending_pong<T, B>(
        &mut self,
        cx: &mut Context,
        dst: &mut Codec<T, B>,
    ) -> Poll<io::Result<()>>
    where
        T: AsyncWrite + Unpin,
        B: Buf,
    {
        if let Some(pong) = self.pending_pong.take() {
            if !dst.poll_ready(cx)?.is_ready() {
                self.pending_pong = Some(pong);
                return Poll::Pending;
            }
            dst.buffer(Ping::pong(pong).into())
                .expect("invalid pong frame");
        }
        Poll::Ready(Ok(()))
    }
    pub(crate) fn send_pending_ping<T, B>(
        &mut self,
        cx: &mut Context,
        dst: &mut Codec<T, B>,
    ) -> Poll<io::Result<()>>
    where
        T: AsyncWrite + Unpin,
        B: Buf,
    {
        if let Some(ref mut ping) = self.pending_ping {
            if !ping.sent {
                if !dst.poll_ready(cx)?.is_ready() {
                    return Poll::Pending;
                }
                dst.buffer(Ping::new(ping.payload).into())
                    .expect("invalid ping frame");
                ping.sent = true;
            }
        } else if let Some(ref users) = self.user_pings {
            if users.0.state.load(Ordering::Acquire) == USER_STATE_PENDING_PING {
                if !dst.poll_ready(cx)?.is_ready() {
                    return Poll::Pending;
                }
                dst.buffer(Ping::new(Ping::USER).into())
                    .expect("invalid ping frame");
                users
                    .0
                    .state
                    .store(USER_STATE_PENDING_PONG, Ordering::Release);
            } else {
                users.0.ping_task.register(cx.waker());
            }
        }
        Poll::Ready(Ok(()))
    }
}
impl ReceivedPing {
    pub(crate) fn is_shutdown(&self) -> bool {
        matches!(*self, Self::Shutdown)
    }
}
impl UserPings {
    pub(crate) fn send_ping(&self) -> Result<(), Option<proto::Error>> {
        let prev = self
            .0
            .state
            .compare_exchange(
                USER_STATE_EMPTY,
                USER_STATE_PENDING_PING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .unwrap_or_else(|v| v);
        match prev {
            USER_STATE_EMPTY => {
                self.0.ping_task.wake();
                Ok(())
            }
            USER_STATE_CLOSED => Err(Some(broken_pipe().into())),
            _ => Err(None),
        }
    }
    pub(crate) fn poll_pong(&self, cx: &mut Context) -> Poll<Result<(), proto::Error>> {
        self.0.pong_task.register(cx.waker());
        let prev = self
            .0
            .state
            .compare_exchange(
                USER_STATE_RECEIVED_PONG,
                USER_STATE_EMPTY,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .unwrap_or_else(|v| v);
        match prev {
            USER_STATE_RECEIVED_PONG => Poll::Ready(Ok(())),
            USER_STATE_CLOSED => Poll::Ready(Err(broken_pipe().into())),
            _ => Poll::Pending,
        }
    }
}
impl UserPingsRx {
    fn receive_pong(&self) -> bool {
        let prev = self
            .0
            .state
            .compare_exchange(
                USER_STATE_PENDING_PONG,
                USER_STATE_RECEIVED_PONG,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .unwrap_or_else(|v| v);
        if prev == USER_STATE_PENDING_PONG {
            self.0.pong_task.wake();
            true
        } else {
            false
        }
    }
}
impl Drop for UserPingsRx {
    fn drop(&mut self) {
        self.0.state.store(USER_STATE_CLOSED, Ordering::Release);
        self.0.pong_task.wake();
    }
}
fn broken_pipe() -> io::Error {
    io::ErrorKind::BrokenPipe.into()
}
