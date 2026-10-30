use axum::{
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::path::Path;

pub(crate) async fn stream_file(
    path: &Path,
    mime: &str,
    headers: &HeaderMap,
) -> axum::response::Response {
    use plain_rs::utils::http::RangeParse;
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(metadata) = file.metadata().await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !metadata.is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let size = metadata.len();
    let range = headers
        .get(axum::http::header::RANGE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let (status, start, length, end) = match plain_rs::utils::http::parse_range_header(range, size)
    {
        RangeParse::Full => (StatusCode::OK, 0, size, None),
        RangeParse::Partial(start, end) => (
            StatusCode::PARTIAL_CONTENT,
            start,
            end - start + 1,
            Some(end),
        ),
        RangeParse::Unsatisfiable => {
            return (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(axum::http::header::CONTENT_RANGE, format!("bytes */{size}"))],
            )
                .into_response();
        }
    };
    if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let stream =
        futures_util::stream::try_unfold((file, length), |(mut file, remaining)| async move {
            if remaining == 0 {
                return Ok::<_, std::io::Error>(None);
            }
            let mut buffer = vec![0; remaining.min(64 * 1024) as usize];
            let read = file.read(&mut buffer).await?;
            if read == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "app file truncated",
                ));
            }
            buffer.truncate(read);
            Ok(Some((buffer, (file, remaining - read as u64))))
        });
    let mut response = axum::body::Body::from_stream(stream).into_response();
    *response.status_mut() = status;
    let output = response.headers_mut();
    output.insert(
        axum::http::header::CONTENT_LENGTH,
        length.to_string().parse().unwrap(),
    );
    output.insert(axum::http::header::ACCEPT_RANGES, "bytes".parse().unwrap());
    output.insert(
        axum::http::header::CACHE_CONTROL,
        "private, max-age=3600".parse().unwrap(),
    );
    if let Ok(value) = mime.parse() {
        output.insert(axum::http::header::CONTENT_TYPE, value);
    }
    if let Some(end) = end {
        output.insert(
            axum::http::header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{size}").parse().unwrap(),
        );
    }
    response
}

pub(crate) fn escape(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (*b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
fn disposition(name: &str, download: bool) -> String {
    let name = escape(name);
    format!(
        "{}; filename=\"{name}\"; filename*=utf-8''{name}",
        if download { "attachment" } else { "inline" }
    )
}
pub(crate) fn secure(mut response: Response, name: Option<&str>, download: bool) -> Response {
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    let mime = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default();
    if mime == "text/html" || mime.ends_with("+xml") || mime.ends_with("/xml") {
        response
            .headers_mut()
            .insert("content-security-policy", "sandbox".parse().unwrap());
    }
    if let Some(name) = name {
        if let Ok(value) = disposition(name, download).parse() {
            response
                .headers_mut()
                .insert(header::CONTENT_DISPOSITION, value);
        }
        response.headers_mut().insert(
            "access-control-expose-headers",
            "Content-Disposition".parse().unwrap(),
        );
    }
    response
}

pub(crate) async fn stream_temporary(
    file: crate::services::files::previews::Temporary,
    mime: &str,
    headers: &HeaderMap,
) -> Response {
    use futures_util::StreamExt;
    let response = stream_file(&file.0, mime, headers).await;
    let (parts, body) = response.into_parts();
    let stream = body.into_data_stream().map(move |chunk| {
        let _ = &file;
        chunk
    });
    Response::from_parts(parts, axum::body::Body::from_stream(stream))
}

pub(crate) fn thumbnail(
    bytes: Result<Option<Vec<u8>>, String>,
    conditional: Option<&str>,
) -> Response {
    use sha2::{Digest, Sha256};
    match bytes {
        Ok(Some(bytes)) => {
            let mime = if bytes.starts_with(b"\x89PNG") {
                "image/png"
            } else {
                "image/jpeg"
            };
            let etag = format!(
                "\"{}\"",
                plain_rs::utils::hex::bytes_to_hex(&Sha256::digest(&bytes))
            );
            if conditional.is_some_and(|value| {
                value.split(',').any(|tag| {
                    let tag = tag.trim();
                    tag == "*" || tag.strip_prefix("W/").unwrap_or(tag) == etag
                })
            }) {
                return (
                    StatusCode::NOT_MODIFIED,
                    [
                        ("etag", etag),
                        ("cache-control", "private, max-age=86400".to_string()),
                    ],
                )
                    .into_response();
            }
            (
                [
                    ("content-type", mime.to_string()),
                    ("etag", etag),
                    ("cache-control", "private, max-age=86400".to_string()),
                ],
                bytes,
            )
                .into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({"error":error})),
        )
            .into_response(),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/http/file_response.rs"]
mod tests;
