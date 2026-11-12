mod de;
use crate::axum::{
    extract::{FromRequestParts, rejection::*},
    routing::url_params::UrlParams,
    util::PercentDecodedStr,
};
use crate::axum_core::response::{IntoResponse, Response};
use crate::http::{StatusCode, request::Parts};
use serde::de::DeserializeOwned;
use std::{fmt, sync::Arc};
#[derive(Debug)]
pub struct Path<T>(pub T);
crate::axum_core::__impl_deref!(Path);

impl<T, S> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = PathRejection;
    async fn from_request_parts<'extract>(parts: &'extract mut Parts, _state: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        let params = match parts.extensions.get::<UrlParams>() {
            Some(UrlParams::Params(params)) => params,
            Some(UrlParams::InvalidUtf8InPathParam { key }) => {
                let err = PathDeserializationError {
                    kind: ErrorKind::InvalidUtf8InPathParam {
                        key: key.to_string(),
                    },
                };
                let err = FailedToDeserializePathParams(err);
                return Err(err.into());
            }
            None => {
                return Err(MissingPathParams.into());
            }
        };
        T::deserialize(de::PathDeserializer::new(params))
            .map_err(|err| {
                PathRejection::FailedToDeserializePathParams(FailedToDeserializePathParams(err))
            })
            .map(Path)
    }
}
#[derive(Debug)]
pub(crate) struct PathDeserializationError {
    pub(super) kind: ErrorKind,
}
impl PathDeserializationError {
    pub(super) fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }
    pub(super) fn wrong_number_of_parameters() -> WrongNumberOfParameters<()> {
        WrongNumberOfParameters { got: () }
    }
    #[track_caller]
    pub(super) fn unsupported_type(name: &'static str) -> Self {
        Self::new(ErrorKind::UnsupportedType { name })
    }
}
pub(super) struct WrongNumberOfParameters<G> {
    got: G,
}
impl<G> WrongNumberOfParameters<G> {
    pub(super) fn got<G2>(self, got: G2) -> WrongNumberOfParameters<G2> {
        WrongNumberOfParameters { got }
    }
}
impl WrongNumberOfParameters<usize> {
    pub(super) fn expected(self, expected: usize) -> PathDeserializationError {
        PathDeserializationError::new(ErrorKind::WrongNumberOfParameters {
            got: self.got,
            expected,
        })
    }
}
impl serde::de::Error for PathDeserializationError {
    #[inline]
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Self {
            kind: ErrorKind::Message(msg.to_string()),
        }
    }
}
impl fmt::Display for PathDeserializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}
impl std::error::Error for PathDeserializationError {}
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    WrongNumberOfParameters {
        got: usize,
        expected: usize,
    },
    ParseErrorAtKey {
        key: String,
        value: String,
        expected_type: &'static str,
    },
    ParseErrorAtIndex {
        index: usize,
        value: String,
        expected_type: &'static str,
    },
    ParseError {
        value: String,
        expected_type: &'static str,
    },
    InvalidUtf8InPathParam {
        key: String,
    },
    UnsupportedType {
        name: &'static str,
    },
    Message(String),
}
impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Message(error) => error.fmt(f),
            ErrorKind::InvalidUtf8InPathParam { key } => write!(f, "Invalid UTF-8 in `{key}`"),
            ErrorKind::WrongNumberOfParameters { got, expected } => {
                write!(
                    f,
                    "Wrong number of path arguments for `Path`. Expected {expected} but got {got}"
                )?;
                if *expected == 1 {
                    write!(
                        f,
                        ". Note that multiple parameters must be extracted with a tuple `Path<(_, _)>` or a struct `Path<YourParams>`"
                    )?;
                }
                Ok(())
            }
            ErrorKind::UnsupportedType { name } => write!(f, "Unsupported type `{name}`"),
            ErrorKind::ParseErrorAtKey {
                key,
                value,
                expected_type,
            } => write!(
                f,
                "Cannot parse `{key}` with value `{value:?}` to a `{expected_type}`"
            ),
            ErrorKind::ParseError {
                value,
                expected_type,
            } => write!(f, "Cannot parse `{value:?}` to a `{expected_type}`"),
            ErrorKind::ParseErrorAtIndex {
                index,
                value,
                expected_type,
            } => write!(
                f,
                "Cannot parse value at index {index} with value `{value:?}` to a `{expected_type}`"
            ),
        }
    }
}
#[derive(Debug)]
pub struct FailedToDeserializePathParams(PathDeserializationError);
impl FailedToDeserializePathParams {
    pub fn kind(&self) -> &ErrorKind {
        &self.0.kind
    }
    pub fn into_kind(self) -> ErrorKind {
        self.0.kind
    }
    pub fn body_text(&self) -> String {
        match self.0.kind {
            ErrorKind::Message(_)
            | ErrorKind::InvalidUtf8InPathParam { .. }
            | ErrorKind::ParseError { .. }
            | ErrorKind::ParseErrorAtIndex { .. }
            | ErrorKind::ParseErrorAtKey { .. } => format!("Invalid URL: {}", self.0.kind),
            ErrorKind::WrongNumberOfParameters { .. } | ErrorKind::UnsupportedType { .. } => {
                self.0.kind.to_string()
            }
        }
    }
    pub fn status(&self) -> StatusCode {
        match self.0.kind {
            ErrorKind::Message(_)
            | ErrorKind::InvalidUtf8InPathParam { .. }
            | ErrorKind::ParseError { .. }
            | ErrorKind::ParseErrorAtIndex { .. }
            | ErrorKind::ParseErrorAtKey { .. } => StatusCode::BAD_REQUEST,
            ErrorKind::WrongNumberOfParameters { .. } | ErrorKind::UnsupportedType { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}
impl IntoResponse for FailedToDeserializePathParams {
    fn into_response(self) -> Response {
        let body = self.body_text();
        crate::axum_core::__log_rejection!(
            rejection_type = Self,
            body_text = body,
            status = self.status(),
        );
        (self.status(), body).into_response()
    }
}
impl fmt::Display for FailedToDeserializePathParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for FailedToDeserializePathParams {}
#[derive(Debug)]
pub struct RawPathParams(Vec<(Arc<str>, PercentDecodedStr)>);

impl<S> FromRequestParts<S> for RawPathParams
where
    S: Send + Sync,
{
    type Rejection = RawPathParamsRejection;
    async fn from_request_parts<'extract>(parts: &'extract mut Parts, _state: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        let params = match parts.extensions.get::<UrlParams>() {
            Some(UrlParams::Params(params)) => params,
            Some(UrlParams::InvalidUtf8InPathParam { key }) => {
                return Err(InvalidUtf8InPathParam {
                    key: Arc::clone(key),
                }
                .into());
            }
            None => {
                return Err(MissingPathParams.into());
            }
        };
        Ok(Self(params.clone()))
    }
}
impl RawPathParams {
    pub fn iter(&self) -> RawPathParamsIter<'_> {
        self.into_iter()
    }
}
impl<'a> IntoIterator for &'a RawPathParams {
    type Item = (&'a str, &'a str);
    type IntoIter = RawPathParamsIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        RawPathParamsIter(self.0.iter())
    }
}
#[derive(Debug)]
pub struct RawPathParamsIter<'a>(std::slice::Iter<'a, (Arc<str>, PercentDecodedStr)>);
impl<'a> Iterator for RawPathParamsIter<'a> {
    type Item = (&'a str, &'a str);
    fn next(&mut self) -> Option<Self::Item> {
        let (key, value) = self.0.next()?;
        Some((&**key, value.as_str()))
    }
}
#[derive(Debug)]
pub struct InvalidUtf8InPathParam {
    key: Arc<str>,
}
impl InvalidUtf8InPathParam {
    pub fn body_text(&self) -> String {
        self.to_string()
    }
    pub fn status(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }
}
impl fmt::Display for InvalidUtf8InPathParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid UTF-8 in `{}`", self.key)
    }
}
impl std::error::Error for InvalidUtf8InPathParam {}
impl IntoResponse for InvalidUtf8InPathParam {
    fn into_response(self) -> Response {
        let body = self.body_text();
        crate::axum_core::__log_rejection!(
            rejection_type = Self,
            body_text = body,
            status = self.status(),
        );
        (self.status(), body).into_response()
    }
}
