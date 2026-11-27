use plain_libs::axum::{
    body::Bytes,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
pub(crate) fn respond(
    bytes: Result<Option<Bytes>, plain_rs::features::services::images::thumbnails::Error>,
    conditional: Option<&str>,
) -> Response {
    use sha2::{Digest, Sha256};
    let bytes = match bytes {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                plain_libs::axum::Json(plain_libs::serde_json::json!({"error":error.to_string()})),
            )
                .into_response();
        }
    };
    let etag = format!(
        "\"{}\"",
        plain_libs::utils::hex::bytes_to_hex(&Sha256::digest(&bytes))
    );
    let matched = conditional.is_some_and(|value| {
        value.split(',').any(|tag| {
            let tag = tag.trim();
            tag == "*" || tag.strip_prefix("W/").unwrap_or(tag) == etag
        })
    });
    let mut response = if matched {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let mime = plain_rs::features::services::files::content::thumbnail_mime(&bytes);
        ([(header::CONTENT_TYPE, mime)], bytes).into_response()
    };
    let headers = response.headers_mut();
    headers.insert(
        header::ETAG,
        HeaderValue::from_maybe_shared(Bytes::from(etag)).unwrap(),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    response
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/http/file_response/thumbnail.rs"]
mod tests;
