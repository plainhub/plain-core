use crate::axum_core::body::Body;
mod append_headers;
mod into_response;
mod into_response_parts;
pub use self::{
    append_headers::AppendHeaders,
    into_response::IntoResponse,
    into_response_parts::{IntoResponseParts, ResponseParts, TryIntoHeaderError},
};
pub type Response<T = Body> = crate::http::Response<T>;
pub type Result<T, E = ErrorResponse> = std::result::Result<T, E>;
impl<T> IntoResponse for Result<T>
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Ok(ok) => ok.into_response(),
            Err(err) => err.0,
        }
    }
}
#[derive(Debug)]
#[must_use]
pub struct ErrorResponse(Response);
impl<T> From<T> for ErrorResponse
where
    T: IntoResponse,
{
    fn from(value: T) -> Self {
        Self(value.into_response())
    }
}
