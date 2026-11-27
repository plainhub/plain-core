pub(crate) mod resource;
pub(crate) mod stream;
pub(crate) mod thumbnail;

use plain_libs::axum::{
    http::{HeaderValue, header},
    response::Response,
};

fn disposition(name: &str, download: bool) -> String {
    let name = plain_libs::utils::query::url_encode(name);
    format!(
        "{}; filename=\"{name}\"; filename*=utf-8''{name}",
        if download { "attachment" } else { "inline" }
    )
}
pub(crate) fn secure(mut response: Response, name: Option<&str>, download: bool) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .unwrap_or_default();
    if mime == "text/html" || mime.ends_with("+xml") || mime.ends_with("/xml") {
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("sandbox"),
        );
    }
    if let Some(name) = name {
        if let Ok(value) = disposition(name, download).parse() {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
        headers.insert(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static("Content-Disposition"),
        );
    }
    response
}
