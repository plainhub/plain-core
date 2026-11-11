use crate::http::{Response, header::HeaderName};
use crate::tungstenite::protocol::{Message, frame::coding::Data};
use std::{io, result, str, string};
use thiserror::Error;
pub type Result<T, E = Error> = result::Result<T, E>;
#[derive(Error, Debug)]
pub enum Error {
    #[error("Connection closed normally")]
    ConnectionClosed,
    #[error("Trying to work with closed connection")]
    AlreadyClosed,
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("TLS error: {0}")]
    Tls(#[from] TlsError),
    #[error("Space limit exceeded: {0}")]
    Capacity(#[from] CapacityError),
    #[error("WebSocket protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("Write buffer is full")]
    WriteBufferFull(Box<Message>),
    #[error("UTF-8 encoding error: {0}")]
    Utf8(String),
    #[error("Attack attempt detected")]
    AttackAttempt,
    #[error("URL error: {0}")]
    Url(#[from] UrlError),
    #[error ("HTTP error: {}" , . 0 . status ())]
    Http(Box<Response<Option<Vec<u8>>>>),
    #[error("HTTP format error: {0}")]
    HttpFormat(#[from] crate::http::Error),
}
impl From<str::Utf8Error> for Error {
    fn from(err: str::Utf8Error) -> Self {
        Error::Utf8(err.to_string())
    }
}
impl From<string::FromUtf8Error> for Error {
    fn from(err: string::FromUtf8Error) -> Self {
        Error::Utf8(err.to_string())
    }
}
impl From<crate::http::header::InvalidHeaderValue> for Error {
    fn from(err: crate::http::header::InvalidHeaderValue) -> Self {
        Error::HttpFormat(err.into())
    }
}
impl From<crate::http::header::InvalidHeaderName> for Error {
    fn from(err: crate::http::header::InvalidHeaderName) -> Self {
        Error::HttpFormat(err.into())
    }
}
impl From<crate::http::header::ToStrError> for Error {
    fn from(err: crate::http::header::ToStrError) -> Self {
        Error::Utf8(err.to_string())
    }
}
impl From<crate::http::uri::InvalidUri> for Error {
    fn from(err: crate::http::uri::InvalidUri) -> Self {
        Error::HttpFormat(err.into())
    }
}
impl From<crate::http::status::InvalidStatusCode> for Error {
    fn from(err: crate::http::status::InvalidStatusCode) -> Self {
        Error::HttpFormat(err.into())
    }
}
impl From<crate::httparse::Error> for Error {
    fn from(err: crate::httparse::Error) -> Self {
        match err {
            crate::httparse::Error::TooManyHeaders => {
                Error::Capacity(CapacityError::TooManyHeaders)
            }
            e => Error::Protocol(ProtocolError::HttparseError(e)),
        }
    }
}
#[derive(Error, Debug, PartialEq, Eq, Clone, Copy)]
pub enum CapacityError {
    #[error("Too many headers")]
    TooManyHeaders,
    #[error("Message too long: {size} > {max_size}")]
    MessageTooLong { size: usize, max_size: usize },
}
#[derive(Error, Clone, PartialEq, Eq, Debug, Copy)]
pub enum SubProtocolError {
    #[error("Server sent a subprotocol but none was requested")]
    ServerSentSubProtocolNoneRequested,
    #[error("Server sent an invalid subprotocol")]
    InvalidSubProtocol,
    #[error("Server sent no subprotocol")]
    NoSubProtocol,
}
#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum ProtocolError {
    #[error("Unsupported HTTP method used - only GET is allowed")]
    WrongHttpMethod,
    #[error("HTTP version must be 1.1 or higher")]
    WrongHttpVersion,
    #[error("No \"Connection: upgrade\" header")]
    MissingConnectionUpgradeHeader,
    #[error("No \"Upgrade: websocket\" header")]
    MissingUpgradeWebSocketHeader,
    #[error("No \"Sec-WebSocket-Version: 13\" header")]
    MissingSecWebSocketVersionHeader,
    #[error("No \"Sec-WebSocket-Key\" header")]
    MissingSecWebSocketKey,
    #[error("Invalid \"Sec-WebSocket-Key\" header value")]
    InvalidSecWebSocketKey,
    #[error("Key mismatch in \"Sec-WebSocket-Accept\" header")]
    SecWebSocketAcceptKeyMismatch,
    #[error("SubProtocol error: {0}")]
    SecWebSocketSubProtocolError(SubProtocolError),
    #[error("Junk after client request")]
    JunkAfterRequest,
    #[error("Custom response must not be successful")]
    CustomResponseSuccessful,
    #[error("Missing, duplicated or incorrect header {0}")]
    InvalidHeader(Box<HeaderName>),
    #[error("Handshake not finished")]
    HandshakeIncomplete,
    #[error("httparse error: {0}")]
    HttparseError(#[from] crate::httparse::Error),
    #[error("Sending after closing is not allowed")]
    SendAfterClosing,
    #[error("Remote sent after having closed")]
    ReceivedAfterClosing,
    #[error("Reserved bits are non-zero")]
    NonZeroReservedBits,
    #[error("Received an unmasked frame from client")]
    UnmaskedFrameFromClient,
    #[error("Received a masked frame from server")]
    MaskedFrameFromServer,
    #[error("Fragmented control frame")]
    FragmentedControlFrame,
    #[error("Control frame too big (payload must be 125 bytes or less)")]
    ControlFrameTooBig,
    #[error("Unknown control frame type: {0}")]
    UnknownControlFrameType(u8),
    #[error("Unknown data frame type: {0}")]
    UnknownDataFrameType(u8),
    #[error("Continue frame but nothing to continue")]
    UnexpectedContinueFrame,
    #[error("While waiting for more fragments received: {0}")]
    ExpectedFragment(Data),
    #[error("Connection reset without closing handshake")]
    ResetWithoutClosingHandshake,
    #[error("Encountered invalid opcode: {0}")]
    InvalidOpcode(u8),
    #[error("Invalid close sequence")]
    InvalidCloseSequence,
}
#[derive(Error, Debug, PartialEq, Eq)]
pub enum UrlError {
    #[error("TLS support not compiled in")]
    TlsFeatureNotEnabled,
    #[error("No host name in the URL")]
    NoHostName,
    #[error("Unable to connect to {0}")]
    UnableToConnect(String),
    #[error("URL scheme not supported")]
    UnsupportedUrlScheme,
    #[error("URL contains empty host name")]
    EmptyHostName,
    #[error("No path/query in URL")]
    NoPathOrQuery,
}
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TlsError {
    #[error("rustls error: {0}")]
    Rustls(Box<rustls::Error>),
    #[error("Invalid DNS name")]
    InvalidDnsName,
}
impl From<rustls::Error> for TlsError {
    fn from(e: rustls::Error) -> Self {
        Self::Rustls(e.into())
    }
}
