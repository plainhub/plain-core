use crate::h2::codec::{SendError, UserError};
pub use crate::h2::frame::Reason;
use crate::h2::frame::StreamId;
use crate::h2::proto::{self, Initiator};
use bytes::Bytes;
use std::{error, fmt, io};
#[derive(Debug)]
pub struct Error {
    kind: Kind,
}
#[derive(Debug)]
enum Kind {
    Reset(StreamId, Reason, Initiator),
    GoAway(Bytes, Reason, Initiator),
    Reason(Reason),
    User(UserError),
    Io(io::Error),
}
impl Error {
    pub fn reason(&self) -> Option<Reason> {
        match self.kind {
            Kind::Reset(_, reason, _) | Kind::GoAway(_, reason, _) | Kind::Reason(reason) => {
                Some(reason)
            }
            _ => None,
        }
    }
    pub fn is_io(&self) -> bool {
        matches!(self.kind, Kind::Io(..))
    }

    pub fn into_io(self) -> Option<io::Error> {
        match self.kind {
            Kind::Io(e) => Some(e),
            _ => None,
        }
    }
    pub(crate) fn from_io(err: io::Error) -> Self {
        Error {
            kind: Kind::Io(err),
        }
    }

    pub fn is_reset(&self) -> bool {
        matches!(self.kind, Kind::Reset(..))
    }
    pub fn is_remote(&self) -> bool {
        matches!(
            self.kind,
            Kind::GoAway(_, _, Initiator::Remote) | Kind::Reset(_, _, Initiator::Remote)
        )
    }
    pub fn is_library(&self) -> bool {
        matches!(
            self.kind,
            Kind::GoAway(_, _, Initiator::Library) | Kind::Reset(_, _, Initiator::Library)
        )
    }
}
impl From<proto::Error> for Error {
    fn from(src: proto::Error) -> Error {
        use crate::h2::proto::Error::*;
        Error {
            kind: match src {
                Reset(stream_id, reason, initiator) => Kind::Reset(stream_id, reason, initiator),
                GoAway(debug_data, reason, initiator) => {
                    Kind::GoAway(debug_data, reason, initiator)
                }
                Io(kind, inner) => {
                    Kind::Io(inner.map_or_else(|| kind.into(), |inner| io::Error::new(kind, inner)))
                }
            },
        }
    }
}
impl From<Reason> for Error {
    fn from(src: Reason) -> Error {
        Error {
            kind: Kind::Reason(src),
        }
    }
}
impl From<SendError> for Error {
    fn from(src: SendError) -> Error {
        match src {
            SendError::User(e) => e.into(),
            SendError::Connection(e) => e.into(),
        }
    }
}
impl From<UserError> for Error {
    fn from(src: UserError) -> Error {
        Error {
            kind: Kind::User(src),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let debug_data = match self.kind {
            Kind::Reset(_, reason, Initiator::User) => {
                return write!(fmt, "stream error sent by user: {}", reason);
            }
            Kind::Reset(_, reason, Initiator::Library) => {
                return write!(fmt, "stream error detected: {}", reason);
            }
            Kind::Reset(_, reason, Initiator::Remote) => {
                return write!(fmt, "stream error received: {}", reason);
            }
            Kind::GoAway(ref debug_data, reason, Initiator::User) => {
                write!(fmt, "connection error sent by user: {}", reason)?;
                debug_data
            }
            Kind::GoAway(ref debug_data, reason, Initiator::Library) => {
                write!(fmt, "connection error detected: {}", reason)?;
                debug_data
            }
            Kind::GoAway(ref debug_data, reason, Initiator::Remote) => {
                write!(fmt, "connection error received: {}", reason)?;
                debug_data
            }
            Kind::Reason(reason) => return write!(fmt, "protocol error: {}", reason),
            Kind::User(ref e) => return write!(fmt, "user error: {}", e),
            Kind::Io(ref e) => return e.fmt(fmt),
        };
        if !debug_data.is_empty() {
            write!(fmt, " ({:?})", debug_data)?;
        }
        Ok(())
    }
}
impl error::Error for Error {}
