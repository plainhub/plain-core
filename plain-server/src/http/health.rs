pub(crate) async fn health() -> &'static str {
    "OK"
}

#[cfg(feature = "http_transport")]
pub(crate) async fn ready() -> axum::http::StatusCode {
    axum::http::StatusCode::NO_CONTENT
}
