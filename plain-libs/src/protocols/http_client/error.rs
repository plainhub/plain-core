use super::{StatusCode, Url};
use std::{error::Error as StdError, fmt, io};

pub(super) type BoxError = Box<dyn StdError + Send + Sync>;

#[derive(Debug)]
enum Kind {
    Builder,
    Request,
    Connect,
    Redirect,
    Status(StatusCode),
    Decode,
    Upgrade,
}

#[derive(Debug)]
pub struct Error(Box<Inner>);

#[derive(Debug)]
struct Inner {
    kind: Kind,
    source: Option<BoxError>,
    url: Option<Box<Url>>,
}

impl Error {
    fn new(kind: Kind, source: impl Into<BoxError>) -> Self {
        Self(Box::new(Inner {
            kind,
            source: Some(source.into()),
            url: None,
        }))
    }

    pub(super) fn builder(source: impl Into<BoxError>) -> Self {
        Self::new(Kind::Builder, source)
    }

    pub(super) fn request(source: impl Into<BoxError>) -> Self {
        Self::new(Kind::Request, source)
    }

    pub(super) fn connect(source: impl Into<BoxError>) -> Self {
        Self::new(Kind::Connect, source)
    }

    pub(super) fn redirect(source: impl Into<BoxError>) -> Self {
        Self::new(Kind::Redirect, source)
    }

    pub(super) fn from_status(status: StatusCode, url: Url) -> Self {
        Self(Box::new(Inner {
            kind: Kind::Status(status),
            source: None,
            url: Some(Box::new(url)),
        }))
    }

    pub(super) fn decode(source: impl Into<BoxError>) -> Self {
        Self::new(Kind::Decode, source)
    }

    pub(super) fn upgrade(source: impl Into<BoxError>) -> Self {
        Self::new(Kind::Upgrade, source)
    }

    pub(super) fn with_url(mut self, url: Url) -> Self {
        if self.0.url.is_none() {
            self.0.url = Some(Box::new(url));
        }
        self
    }

    pub fn status(&self) -> Option<StatusCode> {
        match self.0.kind {
            Kind::Status(status) => Some(status),
            _ => None,
        }
    }

    pub fn is_connect(&self) -> bool {
        std::iter::successors(Some(self as &dyn StdError), |error| (*error).source()).any(|error| {
            error
                .downcast_ref::<Self>()
                .is_some_and(|error| matches!(error.0.kind, Kind::Connect))
                || error
                    .downcast_ref::<crate::hyper_util::client::legacy::Error>()
                    .is_some_and(|error| error.is_connect())
        })
    }

    pub fn is_timeout(&self) -> bool {
        std::iter::successors(Some(self as &dyn StdError), |error| (*error).source()).any(|error| {
            error.is::<tokio::time::error::Elapsed>()
                || error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|error| error.kind() == io::ErrorKind::TimedOut)
        })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.kind {
            Kind::Builder => f.write_str("builder error")?,
            Kind::Request | Kind::Connect => f.write_str("error sending request")?,
            Kind::Redirect => f.write_str("error following redirect")?,
            Kind::Status(status) => {
                let category = if status.is_client_error() {
                    "client"
                } else {
                    "server"
                };
                write!(f, "HTTP status {category} error ({status})")?;
            }
            Kind::Decode => f.write_str("error decoding response body")?,
            Kind::Upgrade => f.write_str("error upgrading connection")?,
        }
        if let Some(url) = &self.0.url {
            write!(f, " for url ({url})")?;
        }
        Ok(())
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0
            .source
            .as_deref()
            .map(|source| source as &dyn StdError)
    }
}
