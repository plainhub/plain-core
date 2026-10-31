use crate::server::ServerState;
use axum::{
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use plain_rs::services::files::archive_downloads::{self, Download, Error};
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct Params {
    #[serde(default)]
    id: String,
}

fn plain(status: StatusCode, body: impl Into<String>) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body.into(),
    )
        .into_response()
}

fn response(result: Result<Download, Error>) -> Response {
    let Download { reader, name } = match result {
        Ok(download) => download,
        Err(error) => {
            return match error {
                Error::Invalid(message) => plain(StatusCode::BAD_REQUEST, message),
                Error::Denied(message) => plain(StatusCode::FORBIDDEN, message),
                Error::Missing => plain(StatusCode::NOT_FOUND, ""),
                Error::Failed(message) => plain(StatusCode::INTERNAL_SERVER_ERROR, message),
                Error::Busy => plain(StatusCode::TOO_MANY_REQUESTS, ""),
            };
        }
    };
    let encoded = crate::http::file_response::escape(&name);
    let disposition = format!("attachment;filename=\"{encoded}\";filename*=utf-8''{encoded}");
    let length = reader.size;
    let stream = futures_util::stream::try_unfold(reader, |mut reader| async move {
        reader
            .read()
            .await
            .map(|chunk| chunk.map(|bytes| (axum::body::Bytes::from(bytes), reader)))
            .map_err(std::io::Error::other)
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(header::CONTENT_LENGTH, length)
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

pub(crate) async fn dir(
    State(state): State<ServerState>,
    Query(params): Query<Params>,
) -> Response {
    response(archive_downloads::directory(&state, &params.id).await)
}

pub(crate) async fn files(
    State(state): State<ServerState>,
    Query(params): Query<Params>,
) -> Response {
    response(archive_downloads::files(&state, &params.id).await)
}

#[cfg(test)]
#[path = "../../tests/unit/http/zip.rs"]
mod tests;
