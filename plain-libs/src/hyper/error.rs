use std::error::Error as StdError;
use std::fmt;
pub type Result<T> = std::result::Result<T, Error>;
type Cause = Box<dyn StdError + Send + Sync>;
pub struct Error {
    inner: Box<ErrorImpl>,
}
struct ErrorImpl {
    kind: Kind,
    cause: Option<Cause>,
}
#[derive(Debug)]
pub(super) enum Kind {
    Parse(Parse),
    User(User),
    IncompleteMessage,
    UnexpectedMessage,
    Canceled,
    ChannelClosed,
    Io,
    HeaderTimeout,
    Body,
    BodyWrite,
    Shutdown,
    Http2,
}
#[derive(Debug)]
pub(super) enum Parse {
    Method,
    Version,
    VersionH2,
    Uri,
    UriTooLong,
    Header(Header),
    TooLarge,
    Status,
    Internal,
}
#[derive(Debug)]
pub(super) enum Header {
    Token,
    ContentLengthInvalid,
    TransferEncodingInvalid,
    TransferEncodingUnexpected,
}
#[derive(Debug)]
pub(super) enum User {
    Body,
    BodyWriteAborted,
    InvalidConnectWithBody,
    Service,
    UnexpectedHeader,
    UnsupportedStatusCode,
    NoUpgrade,
    ManualUpgrade,
    DispatchGone,
}
#[derive(Debug)]
pub(super) struct TimedOut;
impl Error {
    pub fn is_parse(&self) -> bool {
        matches!(self.inner.kind, Kind::Parse(_))
    }

    pub fn is_canceled(&self) -> bool {
        matches!(self.inner.kind, Kind::Canceled)
    }
    pub fn is_closed(&self) -> bool {
        matches!(self.inner.kind, Kind::ChannelClosed)
    }

    pub fn is_shutdown(&self) -> bool {
        if matches!(self.inner.kind, Kind::Shutdown) {
            return true;
        }
        false
    }
    pub fn is_timeout(&self) -> bool {
        if matches!(self.inner.kind, Kind::HeaderTimeout) {
            return true;
        }
        self.find_source::<TimedOut>().is_some()
    }
    pub(super) fn new(kind: Kind) -> Error {
        Error {
            inner: Box::new(ErrorImpl { kind, cause: None }),
        }
    }
    pub(super) fn with<C: Into<Cause>>(mut self, cause: C) -> Error {
        self.inner.cause = Some(cause.into());
        self
    }
    pub(super) fn kind(&self) -> &Kind {
        &self.inner.kind
    }
    pub(crate) fn find_source<E: StdError + 'static>(&self) -> Option<&E> {
        let mut cause = self.source();
        while let Some(err) = cause {
            if let Some(typed) = err.downcast_ref() {
                return Some(typed);
            }
            cause = err.source();
        }
        None
    }
    pub(super) fn h2_reason(&self) -> crate::h2::Reason {
        self.find_source::<crate::h2::Error>()
            .and_then(|h2_err| h2_err.reason())
            .unwrap_or(crate::h2::Reason::INTERNAL_ERROR)
    }
    pub(super) fn new_canceled() -> Error {
        Error::new(Kind::Canceled)
    }
    pub(super) fn new_incomplete() -> Error {
        Error::new(Kind::IncompleteMessage)
    }
    pub(super) fn new_too_large() -> Error {
        Error::new(Kind::Parse(Parse::TooLarge))
    }
    pub(super) fn new_version_h2() -> Error {
        Error::new(Kind::Parse(Parse::VersionH2))
    }
    pub(super) fn new_unexpected_message() -> Error {
        Error::new(Kind::UnexpectedMessage)
    }
    pub(super) fn new_io(cause: std::io::Error) -> Error {
        Error::new(Kind::Io).with(cause)
    }
    pub(super) fn new_closed() -> Error {
        Error::new(Kind::ChannelClosed)
    }
    pub(super) fn new_body<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::Body).with(cause)
    }
    pub(super) fn new_body_write<E: Into<Cause>>(cause: E) -> Error {
        Error::new(Kind::BodyWrite).with(cause)
    }
    pub(super) fn new_body_write_aborted() -> Error {
        Error::new(Kind::User(User::BodyWriteAborted))
    }
    fn new_user(user: User) -> Error {
        Error::new(Kind::User(user))
    }
    pub(super) fn new_user_header() -> Error {
        Error::new_user(User::UnexpectedHeader)
    }
    pub(super) fn new_header_timeout() -> Error {
        Error::new(Kind::HeaderTimeout)
    }
    pub(super) fn new_user_unsupported_status_code() -> Error {
        Error::new_user(User::UnsupportedStatusCode)
    }
    pub(super) fn new_user_no_upgrade() -> Error {
        Error::new_user(User::NoUpgrade)
    }
    pub(super) fn new_user_manual_upgrade() -> Error {
        Error::new_user(User::ManualUpgrade)
    }
    pub(super) fn new_user_service<E: Into<Cause>>(cause: E) -> Error {
        Error::new_user(User::Service).with(cause)
    }
    pub(super) fn new_user_body<E: Into<Cause>>(cause: E) -> Error {
        Error::new_user(User::Body).with(cause)
    }
    pub(super) fn new_user_invalid_connect() -> Error {
        Error::new_user(User::InvalidConnectWithBody)
    }
    pub(super) fn new_shutdown(cause: std::io::Error) -> Error {
        Error::new(Kind::Shutdown).with(cause)
    }
    pub(super) fn new_user_dispatch_gone() -> Error {
        Error::new(Kind::User(User::DispatchGone))
    }
    pub(super) fn new_h2(cause: crate::h2::Error) -> Error {
        if cause.is_io() {
            Error::new_io(cause.into_io().expect("crate::h2::Error::is_io"))
        } else {
            Error::new(Kind::Http2).with(cause)
        }
    }
    fn description(&self) -> &str {
        match self.inner.kind {
            Kind::Parse(Parse::Method) => "invalid HTTP method parsed",
            Kind::Parse(Parse::Version) => "invalid HTTP version parsed",
            Kind::Parse(Parse::VersionH2) => "invalid HTTP version parsed (found HTTP2 preface)",
            Kind::Parse(Parse::Uri) => "invalid URI",
            Kind::Parse(Parse::UriTooLong) => "URI too long",
            Kind::Parse(Parse::Header(Header::Token)) => "invalid HTTP header parsed",
            Kind::Parse(Parse::Header(Header::ContentLengthInvalid)) => {
                "invalid content-length parsed"
            }
            Kind::Parse(Parse::Header(Header::TransferEncodingInvalid)) => {
                "invalid transfer-encoding parsed"
            }
            Kind::Parse(Parse::Header(Header::TransferEncodingUnexpected)) => {
                "unexpected transfer-encoding parsed"
            }
            Kind::Parse(Parse::TooLarge) => "message head is too large",
            Kind::Parse(Parse::Status) => "invalid HTTP status-code parsed",
            Kind::Parse(Parse::Internal) => {
                "internal error inside Hyper and/or its dependencies, please report"
            }
            Kind::IncompleteMessage => "connection closed before message completed",
            Kind::UnexpectedMessage => "received unexpected message from connection",
            Kind::ChannelClosed => "channel closed",
            Kind::Canceled => "operation was canceled",
            Kind::HeaderTimeout => "read header from client timeout",
            Kind::Body => "error reading a body from connection",
            Kind::BodyWrite => "error writing a body to connection",
            Kind::Shutdown => "error shutting down connection",
            Kind::Http2 => "http2 error",
            Kind::Io => "connection error",
            Kind::User(User::Body) => "error from user's Body stream",
            Kind::User(User::BodyWriteAborted) => "user body write aborted",
            Kind::User(User::InvalidConnectWithBody) => {
                "user sent CONNECT request with non-zero body"
            }
            Kind::User(User::Service) => "error from user's Service",
            Kind::User(User::UnexpectedHeader) => "user sent unexpected header",
            Kind::User(User::UnsupportedStatusCode) => {
                "response has 1xx status code, not supported by server"
            }
            Kind::User(User::NoUpgrade) => "no upgrade available",
            Kind::User(User::ManualUpgrade) => "upgrade expected but low level API in use",
            Kind::User(User::DispatchGone) => "dispatch task is gone",
        }
    }
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("crate::hyper::Error");
        f.field(&self.inner.kind);
        if let Some(cause) = &self.inner.cause {
            f.field(cause);
        }
        f.finish()
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner
            .cause
            .as_ref()
            .map(|cause| &**cause as &(dyn StdError + 'static))
    }
}
impl From<Parse> for Error {
    fn from(err: Parse) -> Error {
        Error::new(Kind::Parse(err))
    }
}
impl Parse {
    pub(crate) fn content_length_invalid() -> Self {
        Parse::Header(Header::ContentLengthInvalid)
    }
    pub(crate) fn transfer_encoding_invalid() -> Self {
        Parse::Header(Header::TransferEncodingInvalid)
    }
    pub(crate) fn transfer_encoding_unexpected() -> Self {
        Parse::Header(Header::TransferEncodingUnexpected)
    }
}
impl From<crate::httparse::Error> for Parse {
    fn from(err: crate::httparse::Error) -> Parse {
        match err {
            crate::httparse::Error::HeaderName
            | crate::httparse::Error::HeaderValue
            | crate::httparse::Error::NewLine
            | crate::httparse::Error::Token => Parse::Header(Header::Token),
            crate::httparse::Error::Status => Parse::Status,
            crate::httparse::Error::TooManyHeaders => Parse::TooLarge,
            crate::httparse::Error::Version => Parse::Version,
        }
    }
}
impl From<crate::http::method::InvalidMethod> for Parse {
    fn from(_: crate::http::method::InvalidMethod) -> Parse {
        Parse::Method
    }
}
impl From<crate::http::status::InvalidStatusCode> for Parse {
    fn from(_: crate::http::status::InvalidStatusCode) -> Parse {
        Parse::Status
    }
}
impl From<crate::http::uri::InvalidUri> for Parse {
    fn from(_: crate::http::uri::InvalidUri) -> Parse {
        Parse::Uri
    }
}
impl From<crate::http::uri::InvalidUriParts> for Parse {
    fn from(_: crate::http::uri::InvalidUriParts) -> Parse {
        Parse::Uri
    }
}
impl fmt::Display for TimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("operation timed out")
    }
}
impl StdError for TimedOut {}
