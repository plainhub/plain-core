use crate::axum_core::response::{IntoResponse, Response};
use crate::http::{HeaderValue, StatusCode, header::LOCATION};
#[must_use = "needs to be returned from a handler or otherwise turned into a Response to be useful"]
#[derive(Debug, Clone)]
pub struct Redirect {
    status_code: StatusCode,
    location: HeaderValue,
}
impl Redirect {
    pub fn to(uri: &str) -> Self {
        Self::with_status_code(StatusCode::SEE_OTHER, uri)
    }
    pub fn temporary(uri: &str) -> Self {
        Self::with_status_code(StatusCode::TEMPORARY_REDIRECT, uri)
    }

    fn with_status_code(status_code: StatusCode, uri: &str) -> Self {
        assert!(
            status_code.is_redirection(),
            "not a redirection status code"
        );
        Self {
            status_code,
            location: HeaderValue::try_from(uri).expect("URI isn't a valid header value"),
        }
    }
}
impl IntoResponse for Redirect {
    fn into_response(self) -> Response {
        (self.status_code, [(LOCATION, self.location)]).into_response()
    }
}
