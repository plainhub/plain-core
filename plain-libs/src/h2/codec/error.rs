use crate::h2::proto::Error;
use std::{error, fmt, io};
#[derive(Debug)]
pub enum SendError {
    Connection(Error),
    User(UserError),
}
#[derive(Debug)]
pub enum UserError {
    InactiveStreamId,
    UnexpectedFrameType,
    PayloadTooBig,
    Rejected,
    ReleaseCapacityTooBig,
    OverflowedStreamId,
    MalformedHeaders,
    MissingUriSchemeAndAuthority,
    PollResetAfterSendResponse,
    SendPingWhilePending,
    SendSettingsWhilePending,
    PeerDisabledServerPush,
    InvalidInformationalStatusCode,
}
impl error::Error for SendError {}
impl fmt::Display for SendError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Connection(ref e) => e.fmt(fmt),
            Self::User(ref e) => e.fmt(fmt),
        }
    }
}
impl From<io::Error> for SendError {
    fn from(src: io::Error) -> Self {
        Self::Connection(src.into())
    }
}
impl From<UserError> for SendError {
    fn from(src: UserError) -> Self {
        SendError::User(src)
    }
}
impl error::Error for UserError {}
impl fmt::Display for UserError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::UserError::*;
        fmt.write_str(match *self {
            InactiveStreamId => "inactive stream",
            UnexpectedFrameType => "unexpected frame type",
            PayloadTooBig => "payload too big",
            Rejected => "rejected",
            ReleaseCapacityTooBig => "release capacity too big",
            OverflowedStreamId => "stream ID overflowed",
            MalformedHeaders => "malformed headers",
            MissingUriSchemeAndAuthority => "request URI missing scheme and authority",
            PollResetAfterSendResponse => "poll_reset after send_response is illegal",
            SendPingWhilePending => "send_ping before received previous pong",
            SendSettingsWhilePending => "sending SETTINGS before received previous ACK",
            PeerDisabledServerPush => "sending PUSH_PROMISE to peer who disabled server push",
            InvalidInformationalStatusCode => "invalid informational status code",
        })
    }
}
