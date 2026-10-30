use crate::assembly::server::ServerState;
use crate::http::file_response::{self, secure};
use crate::services::files::{
    content::{self, Content, Options},
    links,
};
use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

pub(crate) async fn get(State(state): State<ServerState>, request: Request) -> Response {
    let request = request.into_parts().0;
    let query = |key| plain_rs::utils::query::query_get(&request.uri.to_string(), key);
    let id = query("id").unwrap_or_default();
    if id.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let range = request
        .headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let options = Options {
        download: query("dl").as_deref() == Some("1"),
        offset: query("offset").and_then(|v| v.parse().ok()),
        length: query("length").and_then(|v| v.parse().ok()),
        width: query("w").and_then(|v| v.parse().ok()),
        height: query("h").and_then(|v| v.parse().ok()),
        center_crop: query("cc").as_deref() != Some("false"),
        #[cfg(feature = "media")]
        pdf: query("preview").as_deref() == Some("pdf"),
        probe: query("probe").as_deref() == Some("1"),
        transcode: query("tr").as_deref() == Some("1"),
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        starts_view: range.is_empty()
            || matches!(
                plain_rs::utils::http::parse_range_header(range, u64::MAX),
                plain_rs::utils::http::RangeParse::Full
                    | plain_rs::utils::http::RangeParse::Partial(0, _)
            ),
    };
    let result = async {
        let file = links::resolve(&state, &id, query("sid").as_deref())?;
        let content = content::read(&state, file, options).await?;
        let response = match content {
            Content::File {
                path,
                mime,
                name,
                download,
            } => secure(
                file_response::stream_file(&path, &mime, &request.headers).await,
                Some(&name),
                download,
            ),
            Content::Temporary { file, mime, name } => secure(
                file_response::stream_temporary(file, mime, &request.headers).await,
                name.as_deref(),
                false,
            ),
            Content::Resource {
                path,
                name,
                download,
            } => {
                #[cfg(feature = "http_transport")]
                {
                    let body = state
                        .bridge
                        .resource(&path, state.stop.clone())
                        .await
                        .map_err(|s| anyhow::anyhow!("Resource stream failed: {s}"))?;
                    secure(
                        ([(header::CONTENT_TYPE, "application/octet-stream")], body)
                            .into_response(),
                        name.as_deref(),
                        download,
                    )
                }
                #[cfg(not(feature = "http_transport"))]
                {
                    let _ = (path, name, download);
                    StatusCode::NOT_FOUND.into_response()
                }
            }
            Content::Bytes(bytes) => secure(
                ([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response(),
                None,
                false,
            ),
            Content::Codec(codec) => secure(
                axum::Json(serde_json::json!({"codec":codec})).into_response(),
                None,
                false,
            ),
            Content::Thumbnail { bytes, name } => secure(
                file_response::thumbnail(
                    bytes,
                    request
                        .headers
                        .get(header::IF_NONE_MATCH)
                        .and_then(|v| v.to_str().ok()),
                ),
                Some(&name),
                false,
            ),
            Content::NotFound => StatusCode::NOT_FOUND.into_response(),
            Content::Directory => StatusCode::BAD_REQUEST.into_response(),
            Content::SliceTooLarge => {
                (StatusCode::BAD_REQUEST, "range length is too large").into_response()
            }
            Content::TranscodingUnavailable => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "video transcoding is not available for this file",
            )
                .into_response(),
        };
        Ok::<_, anyhow::Error>(response)
    }
    .await;
    result.unwrap_or_else(|error| {
        (
            StatusCode::FORBIDDEN,
            format!("File is expired or does not exist. {error}"),
        )
            .into_response()
    })
}

#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
#[path = "../../tests/unit/http/fs.rs"]
mod tests;
