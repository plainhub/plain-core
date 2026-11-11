use crate::http::header;
use crate::http::header::MaxSizeReached;
use crate::http::method;
use crate::http::status;
use crate::http::uri;
use std::error;
use std::fmt;
use std::result;
pub struct Error {
    inner: ErrorKind,
}
pub type Result<T> = result::Result<T, Error>;
enum ErrorKind {
    StatusCode(status::InvalidStatusCode),
    Method(method::InvalidMethod),
    Uri(uri::InvalidUri),
    UriParts(uri::InvalidUriParts),
    HeaderName(header::InvalidHeaderName),
    HeaderValue(header::InvalidHeaderValue),
    MaxSizeReached(MaxSizeReached),
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("crate::http::Error")
            .field(&self.get_ref())
            .finish()
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.get_ref(), f)
    }
}
impl Error {
    pub(crate) fn is_empty_uri(&self) -> bool {
        match self.inner {
            ErrorKind::Uri(ref err) => err.is_empty(),
            _ => false,
        }
    }
    pub fn is<T: error::Error + 'static>(&self) -> bool {
        self.get_ref().is::<T>()
    }
    pub fn get_ref(&self) -> &(dyn error::Error + 'static) {
        use self::ErrorKind::*;
        match self.inner {
            StatusCode(ref e) => e,
            Method(ref e) => e,
            Uri(ref e) => e,
            UriParts(ref e) => e,
            HeaderName(ref e) => e,
            HeaderValue(ref e) => e,
            MaxSizeReached(ref e) => e,
        }
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.get_ref().source()
    }
}
impl From<MaxSizeReached> for Error {
    fn from(err: MaxSizeReached) -> Error {
        Error {
            inner: ErrorKind::MaxSizeReached(err),
        }
    }
}
impl From<status::InvalidStatusCode> for Error {
    fn from(err: status::InvalidStatusCode) -> Error {
        Error {
            inner: ErrorKind::StatusCode(err),
        }
    }
}
impl From<method::InvalidMethod> for Error {
    fn from(err: method::InvalidMethod) -> Error {
        Error {
            inner: ErrorKind::Method(err),
        }
    }
}
impl From<uri::InvalidUri> for Error {
    fn from(err: uri::InvalidUri) -> Error {
        Error {
            inner: ErrorKind::Uri(err),
        }
    }
}
impl From<uri::InvalidUriParts> for Error {
    fn from(err: uri::InvalidUriParts) -> Error {
        Error {
            inner: ErrorKind::UriParts(err),
        }
    }
}
impl From<header::InvalidHeaderName> for Error {
    fn from(err: header::InvalidHeaderName) -> Error {
        Error {
            inner: ErrorKind::HeaderName(err),
        }
    }
}
impl From<header::InvalidHeaderValue> for Error {
    fn from(err: header::InvalidHeaderValue) -> Error {
        Error {
            inner: ErrorKind::HeaderValue(err),
        }
    }
}
impl From<std::convert::Infallible> for Error {
    fn from(err: std::convert::Infallible) -> Error {
        match err {}
    }
}
