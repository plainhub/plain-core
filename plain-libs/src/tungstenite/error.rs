use crate::http::{Response, header::HeaderName};
use crate::tungstenite::protocol::{Message, frame::coding::Data};
use std::{io, result, str, string};
pub type Result<T, E = Error> = result::Result<T, E>;
#[derive(Debug)]
pub enum Error {
    ConnectionClosed,
    AlreadyClosed,
    Io(io::Error),
    Tls(TlsError),
    Capacity(CapacityError),
    Protocol(ProtocolError),
    WriteBufferFull(Box<Message>),
    Utf8(String),
    AttackAttempt,
    Url(UrlError),
    Http(Box<Response<Option<Vec<u8>>>>),
    HttpFormat(crate::http::Error),
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
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CapacityError {
    TooManyHeaders,
    MessageTooLong { size: usize, max_size: usize },
}
#[derive(Clone, PartialEq, Eq, Debug, Copy)]
pub enum SubProtocolError {
    ServerSentSubProtocolNoneRequested,
    InvalidSubProtocol,
    NoSubProtocol,
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ProtocolError {
    WrongHttpMethod,
    WrongHttpVersion,
    MissingConnectionUpgradeHeader,
    MissingUpgradeWebSocketHeader,
    MissingSecWebSocketVersionHeader,
    MissingSecWebSocketKey,
    InvalidSecWebSocketKey,
    SecWebSocketAcceptKeyMismatch,
    SecWebSocketSubProtocolError(SubProtocolError),
    JunkAfterRequest,
    CustomResponseSuccessful,
    InvalidHeader(Box<HeaderName>),
    HandshakeIncomplete,
    HttparseError(crate::httparse::Error),
    SendAfterClosing,
    ReceivedAfterClosing,
    NonZeroReservedBits,
    UnmaskedFrameFromClient,
    MaskedFrameFromServer,
    FragmentedControlFrame,
    ControlFrameTooBig,
    UnknownControlFrameType(u8),
    UnknownDataFrameType(u8),
    UnexpectedContinueFrame,
    ExpectedFragment(Data),
    ResetWithoutClosingHandshake,
    InvalidOpcode(u8),
    InvalidCloseSequence,
}
#[derive(Debug, PartialEq, Eq)]
pub enum UrlError {
    TlsFeatureNotEnabled,
    NoHostName,
    UnableToConnect(String),
    UnsupportedUrlScheme,
    EmptyHostName,
    NoPathOrQuery,
}
#[derive(Debug)]
#[non_exhaustive]
pub enum TlsError {
    Rustls(Box<rustls::Error>),
    InvalidDnsName,
}
impl From<rustls::Error> for TlsError {
    fn from(e: rustls::Error) -> Self {
        Self::Rustls(e.into())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionClosed => f.write_str("Connection closed normally"),
            Self::AlreadyClosed => f.write_str("Trying to work with closed connection"),
            Self::Io(inner) => write!(f, "IO error: {0}", inner),
            Self::Tls(inner) => write!(f, "TLS error: {0}", inner),
            Self::Capacity(inner) => write!(f, "Space limit exceeded: {0}", inner),
            Self::Protocol(inner) => write!(f, "WebSocket protocol error: {0}", inner),
            Self::WriteBufferFull(_) => f.write_str("Write buffer is full"),
            Self::Utf8(inner) => write!(f, "UTF-8 encoding error: {0}", inner),
            Self::AttackAttempt => f.write_str("Attack attempt detected"),
            Self::Url(inner) => write!(f, "URL error: {0}", inner),
            Self::Http(inner) => write!(f, "HTTP error: {}", inner.status()),
            Self::HttpFormat(inner) => write!(f, "HTTP format error: {0}", inner),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(inner) => Some(inner),
            Self::Tls(inner) => Some(inner),
            Self::Capacity(inner) => Some(inner),
            Self::Protocol(inner) => Some(inner),
            Self::Url(inner) => Some(inner),
            Self::HttpFormat(inner) => Some(inner),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(inner: io::Error) -> Self {
        Self::Io(inner)
    }
}

impl From<TlsError> for Error {
    fn from(inner: TlsError) -> Self {
        Self::Tls(inner)
    }
}

impl From<CapacityError> for Error {
    fn from(inner: CapacityError) -> Self {
        Self::Capacity(inner)
    }
}

impl From<ProtocolError> for Error {
    fn from(inner: ProtocolError) -> Self {
        Self::Protocol(inner)
    }
}

impl From<UrlError> for Error {
    fn from(inner: UrlError) -> Self {
        Self::Url(inner)
    }
}

impl From<crate::http::Error> for Error {
    fn from(inner: crate::http::Error) -> Self {
        Self::HttpFormat(inner)
    }
}

impl std::fmt::Display for CapacityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyHeaders => f.write_str("Too many headers"),
            Self::MessageTooLong { size, max_size } => write!(f, "Message too long: {size} > {max_size}"),
        }
    }
}

impl std::error::Error for CapacityError {}

impl std::fmt::Display for SubProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerSentSubProtocolNoneRequested => {
                f.write_str("Server sent a subprotocol but none was requested")
            }
            Self::InvalidSubProtocol => f.write_str("Server sent an invalid subprotocol"),
            Self::NoSubProtocol => f.write_str("Server sent no subprotocol"),
        }
    }
}

impl std::error::Error for SubProtocolError {}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongHttpMethod => {
                f.write_str("Unsupported HTTP method used - only GET is allowed")
            }
            Self::WrongHttpVersion => f.write_str("HTTP version must be 1.1 or higher"),
            Self::MissingConnectionUpgradeHeader => {
                f.write_str("No \"Connection: upgrade\" header")
            }
            Self::MissingUpgradeWebSocketHeader => f.write_str("No \"Upgrade: websocket\" header"),
            Self::MissingSecWebSocketVersionHeader => {
                f.write_str("No \"Sec-WebSocket-Version: 13\" header")
            }
            Self::MissingSecWebSocketKey => f.write_str("No \"Sec-WebSocket-Key\" header"),
            Self::InvalidSecWebSocketKey => {
                f.write_str("Invalid \"Sec-WebSocket-Key\" header value")
            }
            Self::SecWebSocketAcceptKeyMismatch => {
                f.write_str("Key mismatch in \"Sec-WebSocket-Accept\" header")
            }
            Self::SecWebSocketSubProtocolError(inner) => write!(f, "SubProtocol error: {0}", inner),
            Self::JunkAfterRequest => f.write_str("Junk after client request"),
            Self::CustomResponseSuccessful => f.write_str("Custom response must not be successful"),
            Self::InvalidHeader(inner) => {
                write!(f, "Missing, duplicated or incorrect header {0}", inner)
            }
            Self::HandshakeIncomplete => f.write_str("Handshake not finished"),
            Self::HttparseError(inner) => write!(f, "httparse error: {0}", inner),
            Self::SendAfterClosing => f.write_str("Sending after closing is not allowed"),
            Self::ReceivedAfterClosing => f.write_str("Remote sent after having closed"),
            Self::NonZeroReservedBits => f.write_str("Reserved bits are non-zero"),
            Self::UnmaskedFrameFromClient => f.write_str("Received an unmasked frame from client"),
            Self::MaskedFrameFromServer => f.write_str("Received a masked frame from server"),
            Self::FragmentedControlFrame => f.write_str("Fragmented control frame"),
            Self::ControlFrameTooBig => {
                f.write_str("Control frame too big (payload must be 125 bytes or less)")
            }
            Self::UnknownControlFrameType(inner) => {
                write!(f, "Unknown control frame type: {0}", inner)
            }
            Self::UnknownDataFrameType(inner) => write!(f, "Unknown data frame type: {0}", inner),
            Self::UnexpectedContinueFrame => f.write_str("Continue frame but nothing to continue"),
            Self::ExpectedFragment(inner) => {
                write!(f, "While waiting for more fragments received: {0}", inner)
            }
            Self::ResetWithoutClosingHandshake => {
                f.write_str("Connection reset without closing handshake")
            }
            Self::InvalidOpcode(inner) => write!(f, "Encountered invalid opcode: {0}", inner),
            Self::InvalidCloseSequence => f.write_str("Invalid close sequence"),
        }
    }
}

impl std::error::Error for ProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HttparseError(inner) => Some(inner),
            _ => None,
        }
    }
}

impl From<crate::httparse::Error> for ProtocolError {
    fn from(inner: crate::httparse::Error) -> Self {
        Self::HttparseError(inner)
    }
}

impl std::fmt::Display for UrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TlsFeatureNotEnabled => f.write_str("TLS support not compiled in"),
            Self::NoHostName => f.write_str("No host name in the URL"),
            Self::UnableToConnect(inner) => write!(f, "Unable to connect to {0}", inner),
            Self::UnsupportedUrlScheme => f.write_str("URL scheme not supported"),
            Self::EmptyHostName => f.write_str("URL contains empty host name"),
            Self::NoPathOrQuery => f.write_str("No path/query in URL"),
        }
    }
}

impl std::error::Error for UrlError {}

impl std::fmt::Display for TlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rustls(inner) => write!(f, "rustls error: {0}", inner),
            Self::InvalidDnsName => f.write_str("Invalid DNS name"),
        }
    }
}

impl std::error::Error for TlsError {}
