use crate::axum_core::body::Body;
use crate::http::{HeaderValue, StatusCode, header};
mod redirect;
pub mod sse;
pub use self::redirect::Redirect;
pub use crate::axum::Extension;
pub use crate::axum::Json;
pub use crate::axum_core::response::{
    AppendHeaders, ErrorResponse, IntoResponse, IntoResponseParts, Response, ResponseParts, Result,
};
pub use sse::Sse;
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Html<T>(pub T);
impl<T> IntoResponse for Html<T>
where
    T: Into<Body>,
{
    fn into_response(self) -> Response {
        (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(crate::mime::TEXT_HTML_UTF_8.as_ref()),
            )],
            self.0.into(),
        )
            .into_response()
    }
}
impl<T> From<T> for Html<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct NoContent;
impl IntoResponse for NoContent {
    fn into_response(self) -> Response {
        StatusCode::NO_CONTENT.into_response()
    }
}
