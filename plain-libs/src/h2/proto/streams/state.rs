use self::Inner::*;
use self::Peer::*;
use crate::h2::codec::UserError;
use crate::h2::frame::{self, Reason, StreamId};
use crate::h2::proto::{self, Error, Initiator, PollReset};
use std::fmt;
use std::io;
#[derive(Clone)]
pub struct State {
    inner: Inner,
}
#[derive(Debug, Clone)]
enum Inner {
    Idle,
    ReservedLocal,
    ReservedRemote,
    Open { local: Peer, remote: Peer },
    HalfClosedLocal(Peer),
    HalfClosedRemote(Peer),
    Closed(Cause),
}
#[derive(Debug, Copy, Clone, Default)]
enum Peer {
    #[default]
    AwaitingHeaders,
    Streaming,
}
#[derive(Debug, Clone)]
enum Cause {
    EndStream,
    Error(Error),
    ErrorAfterEndStream(Error),
    ScheduledLibraryReset(Reason),
}
impl State {
    pub fn send_open(&mut self, eos: bool) -> Result<(), UserError> {
        let local = Streaming;
        self.inner = match self.inner {
            Idle => {
                if eos {
                    HalfClosedLocal(AwaitingHeaders)
                } else {
                    Open {
                        local,
                        remote: AwaitingHeaders,
                    }
                }
            }
            Open {
                local: AwaitingHeaders,
                remote,
            } => {
                if eos {
                    HalfClosedLocal(remote)
                } else {
                    Open { local, remote }
                }
            }
            HalfClosedRemote(AwaitingHeaders) | ReservedLocal => {
                if eos {
                    Closed(Cause::EndStream)
                } else {
                    HalfClosedRemote(local)
                }
            }
            _ => {
                return Err(UserError::UnexpectedFrameType);
            }
        };
        Ok(())
    }
    pub fn recv_open(&mut self, frame: &frame::Headers) -> Result<bool, Error> {
        let mut initial = false;
        let eos = frame.is_end_stream();
        self.inner = match self.inner {
            Idle => {
                initial = true;
                if eos {
                    HalfClosedRemote(AwaitingHeaders)
                } else {
                    Open {
                        local: AwaitingHeaders,
                        remote: if frame.is_informational() {
                            tracing::trace!("skipping 1xx response headers");
                            AwaitingHeaders
                        } else {
                            Streaming
                        },
                    }
                }
            }
            ReservedRemote => {
                initial = true;
                if eos {
                    Closed(Cause::EndStream)
                } else if frame.is_informational() {
                    tracing::trace!("skipping 1xx response headers");
                    ReservedRemote
                } else {
                    HalfClosedLocal(Streaming)
                }
            }
            Open {
                local,
                remote: AwaitingHeaders,
            } => {
                if eos {
                    HalfClosedRemote(local)
                } else {
                    Open {
                        local,
                        remote: if frame.is_informational() {
                            tracing::trace!("skipping 1xx response headers");
                            AwaitingHeaders
                        } else {
                            Streaming
                        },
                    }
                }
            }
            HalfClosedLocal(AwaitingHeaders) => {
                if eos {
                    Closed(Cause::EndStream)
                } else if frame.is_informational() {
                    tracing::trace!("skipping 1xx response headers");
                    HalfClosedLocal(AwaitingHeaders)
                } else {
                    HalfClosedLocal(Streaming)
                }
            }
            ref state => {
                proto_err ! (conn : "recv_open: in unexpected state {:?}" , state);
                return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
            }
        };
        Ok(initial)
    }
    pub fn reserve_remote(&mut self) -> Result<(), Error> {
        match self.inner {
            Idle => {
                self.inner = ReservedRemote;
                Ok(())
            }
            ref state => {
                proto_err ! (conn : "reserve_remote: in unexpected state {:?}" , state);
                Err(Error::library_go_away(Reason::PROTOCOL_ERROR))
            }
        }
    }
    pub fn reserve_local(&mut self) -> Result<(), UserError> {
        match self.inner {
            Idle => {
                self.inner = ReservedLocal;
                Ok(())
            }
            _ => Err(UserError::UnexpectedFrameType),
        }
    }
    pub fn recv_close(&mut self) -> Result<(), Error> {
        match self.inner {
            Open { local, .. } => {
                tracing::trace!("recv_close: Open => HalfClosedRemote({:?})", local);
                self.inner = HalfClosedRemote(local);
                Ok(())
            }
            HalfClosedLocal(..) => {
                tracing::trace!("recv_close: HalfClosedLocal => Closed");
                self.inner = Closed(Cause::EndStream);
                Ok(())
            }
            ref state => {
                proto_err ! (conn : "recv_close: in unexpected state {:?}" , state);
                Err(Error::library_go_away(Reason::PROTOCOL_ERROR))
            }
        }
    }
    pub fn recv_reset(&mut self, frame: frame::Reset, queued: bool) {
        let recv_end_stream = self.is_recv_end_stream();
        match self.inner {
            Closed(..) if !queued => {}
            ref state => {
                tracing::trace!(
                    "recv_reset; frame={:?}; state={:?}; queued={:?}",
                    frame,
                    state,
                    queued
                );
                let error = Error::remote_reset(frame.stream_id(), frame.reason());
                self.inner = Closed(if recv_end_stream {
                    Cause::ErrorAfterEndStream(error)
                } else {
                    Cause::Error(error)
                });
            }
        }
    }
    pub fn handle_error(&mut self, err: &proto::Error) {
        match self.inner {
            Closed(..) => {}
            _ => {
                tracing::trace!("handle_error; err={:?}", err);
                self.inner = Closed(Cause::Error(err.clone()));
            }
        }
    }
    pub fn recv_eof(&mut self) {
        match self.inner {
            Closed(..) => {}
            ref state => {
                tracing::trace!("recv_eof; state={:?}", state);
                self.inner = Closed(Cause::Error(
                    io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "stream closed because of a broken pipe",
                    )
                    .into(),
                ));
            }
        }
    }
    pub fn send_close(&mut self) {
        match self.inner {
            Open { remote, .. } => {
                tracing::trace!("send_close: Open => HalfClosedLocal({:?})", remote);
                self.inner = HalfClosedLocal(remote);
            }
            HalfClosedRemote(..) => {
                tracing::trace!("send_close: HalfClosedRemote => Closed");
                self.inner = Closed(Cause::EndStream);
            }
            ref state => panic!("send_close: unexpected state {:?}", state),
        }
    }
    pub fn set_reset(&mut self, stream_id: StreamId, reason: Reason, initiator: Initiator) {
        self.inner = Closed(Cause::Error(Error::Reset(stream_id, reason, initiator)));
    }
    pub fn set_scheduled_reset(&mut self, reason: Reason) {
        debug_assert!(!self.is_closed());
        self.inner = Closed(Cause::ScheduledLibraryReset(reason));
    }
    pub fn get_scheduled_reset(&self) -> Option<Reason> {
        match self.inner {
            Closed(Cause::ScheduledLibraryReset(reason)) => Some(reason),
            _ => None,
        }
    }
    pub fn is_scheduled_reset(&self) -> bool {
        matches!(self.inner, Closed(Cause::ScheduledLibraryReset(..)))
    }
    pub fn is_local_error(&self) -> bool {
        match self.inner {
            Closed(Cause::Error(ref e) | Cause::ErrorAfterEndStream(ref e)) => e.is_local(),
            Closed(Cause::ScheduledLibraryReset(..)) => true,
            _ => false,
        }
    }
    pub fn is_remote_reset(&self) -> bool {
        matches!(
            self.inner,
            Closed(Cause::Error(Error::Reset(_, _, Initiator::Remote)))
                | Closed(Cause::ErrorAfterEndStream(Error::Reset(
                    _,
                    _,
                    Initiator::Remote
                )))
        )
    }
    pub fn is_reset(&self) -> bool {
        match self.inner {
            Closed(Cause::EndStream) => false,
            Closed(_) => true,
            _ => false,
        }
    }
    pub fn is_send_streaming(&self) -> bool {
        matches!(
            self.inner,
            Open {
                local: Streaming,
                ..
            } | HalfClosedRemote(Streaming)
        )
    }
    pub fn is_recv_headers(&self) -> bool {
        matches!(
            self.inner,
            Idle | Open {
                remote: AwaitingHeaders,
                ..
            } | HalfClosedLocal(AwaitingHeaders)
                | ReservedRemote
        )
    }
    pub fn is_recv_streaming(&self) -> bool {
        matches!(
            self.inner,
            Open {
                remote: Streaming,
                ..
            } | HalfClosedLocal(Streaming)
        )
    }
    pub fn is_recv_end_stream(&self) -> bool {
        matches!(
            self.inner,
            Closed(Cause::EndStream | Cause::ErrorAfterEndStream(_)) | HalfClosedRemote(..)
        )
    }
    pub fn is_closed(&self) -> bool {
        matches!(self.inner, Closed(_))
    }
    pub fn is_send_closed(&self) -> bool {
        matches!(
            self.inner,
            Closed(..) | HalfClosedLocal(..) | ReservedRemote
        )
    }
    pub fn is_idle(&self) -> bool {
        matches!(self.inner, Idle)
    }
    pub fn ensure_recv_open(&self) -> Result<bool, proto::Error> {
        match self.inner {
            Closed(Cause::Error(ref e)) => Err(e.clone()),
            Closed(Cause::ScheduledLibraryReset(reason)) => {
                Err(proto::Error::library_go_away(reason))
            }
            Closed(Cause::EndStream | Cause::ErrorAfterEndStream(_))
            | HalfClosedRemote(..)
            | ReservedLocal => Ok(false),
            _ => Ok(true),
        }
    }
    pub(super) fn ensure_reason(
        &self,
        mode: PollReset,
    ) -> Result<Option<Reason>, crate::h2::Error> {
        match self.inner {
            Closed(Cause::Error(Error::Reset(_, reason, _)))
            | Closed(Cause::ErrorAfterEndStream(Error::Reset(_, reason, _)))
            | Closed(Cause::Error(Error::GoAway(_, reason, _)))
            | Closed(Cause::ErrorAfterEndStream(Error::GoAway(_, reason, _)))
            | Closed(Cause::ScheduledLibraryReset(reason)) => Ok(Some(reason)),
            Closed(Cause::Error(ref e) | Cause::ErrorAfterEndStream(ref e)) => {
                Err(e.clone().into())
            }
            Open {
                local: Streaming, ..
            }
            | HalfClosedRemote(Streaming) => match mode {
                PollReset::AwaitingHeaders => Err(UserError::PollResetAfterSendResponse.into()),
                PollReset::Streaming => Ok(None),
            },
            _ => Ok(None),
        }
    }
}
impl Default for State {
    fn default() -> State {
        State { inner: Inner::Idle }
    }
}
impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}
