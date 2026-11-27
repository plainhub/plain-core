use plain_libs::axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use plain_libs::utils::http::{RangeParse, parse_range_header};
use plain_rs::{
    features::services::files::previews::Temporary,
    infrastructure::host::resource_stream::NativeResourceFile,
};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};

pub(crate) enum Source {
    Path(PathBuf),
    Temporary(Temporary),
    Native(NativeResourceFile),
}

impl Source {
    pub(crate) async fn respond(self, mime: &str, headers: &HeaderMap) -> Response {
        let range = headers.get(header::RANGE).cloned();
        let mime = mime.to_owned();
        tokio::task::spawn_blocking(move || {
            let (file, temporary) = match self {
                Self::Native(input) => {
                    let Ok(size) = u64::try_from(input.length) else {
                        return StatusCode::BAD_GATEWAY.into_response();
                    };
                    return range_response(
                        input.file,
                        input.offset as u64,
                        size,
                        None,
                        &mime,
                        range,
                    );
                }
                Self::Path(path) => (File::open(path), None),
                Self::Temporary(file) => (File::open(&file.0), Some(file)),
            };
            let Ok(file) = file else {
                return StatusCode::NOT_FOUND.into_response();
            };
            let Ok(metadata) = file.metadata() else {
                return StatusCode::NOT_FOUND.into_response();
            };
            if !metadata.is_file() {
                return StatusCode::NOT_FOUND.into_response();
            }
            range_response(file, 0, metadata.len(), temporary, &mime, range)
        })
        .await
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    }
}

fn range_response(
    mut file: impl Read + Seek + Send + 'static,
    base: u64,
    size: u64,
    temporary: Option<Temporary>,
    mime: &str,
    range: Option<HeaderValue>,
) -> Response {
    let range = range
        .as_ref()
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let (status, start, length, end) = match parse_range_header(range, size) {
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
                [(header::CONTENT_RANGE, format!("bytes */{size}"))],
            )
                .into_response();
        }
    };
    if file.seek(SeekFrom::Start(base + start)).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let stream = futures_util::stream::try_unfold(
        (file, length, temporary),
        |(mut file, remaining, temporary)| async move {
            if remaining == 0 {
                return Ok::<_, std::io::Error>(None);
            }
            let (file, mut buffer, read) = tokio::task::spawn_blocking(move || {
                let mut buffer = vec![0; remaining.min(64 * 1024) as usize];
                let read = file.read(&mut buffer)?;
                Ok::<_, std::io::Error>((file, buffer, read))
            })
            .await??;
            if read == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "app file truncated",
                ));
            }
            buffer.truncate(read);
            Ok(Some((buffer, (file, remaining - read as u64, temporary))))
        },
    );
    let mut response = Body::from_stream(stream).into_response();
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_LENGTH, length.into());
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    if let Ok(value) = mime.parse() {
        headers.insert(header::CONTENT_TYPE, value);
    }
    if let Some(end) = end {
        headers.insert(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{size}").parse().unwrap(),
        );
    }
    response
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/http/file_response/stream.rs"]
mod tests;
