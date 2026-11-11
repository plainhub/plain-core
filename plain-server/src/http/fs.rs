use crate::http::file_response::{self, secure};
use crate::server::ServerState;
use plain_libs::axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use plain_rs::features::services::files::content;
use plain_rs::features::services::files::content::Content;
use plain_rs::features::services::files::content::Options;
use plain_rs::features::services::files::links;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Parameters {
    pub id: String,
    pub action: content::Action,
    pub size: Option<content::Size>,
    #[serde(rename = "v")]
    pub version: Option<String>,
}

impl Parameters {
    pub fn is_valid(&self) -> bool {
        !self.id.is_empty() && (self.size.is_none() || self.action == content::Action::Preview)
    }
}

pub(crate) async fn get(State(state): State<ServerState>, request: Request) -> Response {
    let request = request.into_parts().0;
    let parameters = match plain_libs::axum::extract::Query::<Parameters>::try_from_uri(&request.uri) {
        Ok(parameters) if parameters.is_valid() => parameters.0,
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    let managed = match super::managed_proxy::forwarded_resource(&state, &request, &parameters.id) {
        Ok(managed) => managed,
        Err(status) => return status.into_response(),
    };
    super::managed_proxy::mark_forwarded(serve(state, parameters, request.headers).await, managed)
}

pub(crate) async fn native_preview(
    state: ServerState,
    id: String,
) -> Result<crate::native::resources::FilePreview, String> {
    use crate::native::resources::{FilePreview, MAX_RESOURCE_RESPONSE_BYTES};
    let parameters = Parameters {
        id,
        action: content::Action::Preview,
        size: None,
        version: None,
    };
    let (parts, body) = serve(state, parameters, HeaderMap::new())
        .await
        .into_parts();
    let mut preview = FilePreview {
        status: parts.status.as_u16(),
        headers: Default::default(),
        content: String::new(),
    };
    if parts.status.is_success() {
        for name in [
            header::CONTENT_TYPE,
            header::CONTENT_DISPOSITION,
            header::CONTENT_LENGTH,
            header::LAST_MODIFIED,
        ] {
            if let Some(value) = parts
                .headers
                .get(&name)
                .and_then(|value| value.to_str().ok())
            {
                preview.headers.insert(name.to_string(), value.to_owned());
            }
        }
        let bytes = plain_libs::axum::body::to_bytes(body, MAX_RESOURCE_RESPONSE_BYTES as usize)
            .await
            .map_err(|error| error.to_string())?;
        preview.content = String::from_utf8_lossy(&bytes).into_owned();
    }
    Ok(preview)
}

async fn serve(state: ServerState, parameters: Parameters, mut headers: HeaderMap) -> Response {
    if matches!(
        parameters.action,
        content::Action::Probe | content::Action::Transcode
    ) {
        headers.remove(header::RANGE);
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let access_order = state.thumbnails.next_access_order();
    let options = Options {
        access_order,
        action: parameters.action,
        size: parameters.size,
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        starts_view: parameters.action == content::Action::Preview
            && (range.is_empty()
                || matches!(
                    plain_libs::utils::http::parse_range_header(range, u64::MAX),
                    plain_libs::utils::http::RangeParse::Full
                        | plain_libs::utils::http::RangeParse::Partial(0, _)
                )),
    };
    let result = async {
        let kind = if options.size.is_some() {
            links::ReadKind::Thumbnail
        } else {
            links::ReadKind::Original
        };
        let file = links::resolve(&state, &parameters.id, kind).await?;
        let content = content::read(&state, file, options).await?;
        let response = match content {
            Content::File {
                path,
                mime,
                name,
                download,
            } => secure(
                file_response::stream_file(&path, &mime, &headers).await,
                Some(&name),
                download,
            ),
            Content::Temporary { file, mime, name } => secure(
                file_response::stream_temporary(file, mime, &headers).await,
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
                    match file_response::resource(
                        &state.resources,
                        &path,
                        state.stop.clone(),
                        &headers,
                    )
                    .await
                    {
                        Ok(response) => secure(response, name.as_deref(), download),
                        Err(status) => status.into_response(),
                    }
                }
                #[cfg(not(feature = "http_transport"))]
                {
                    let _ = (path, name, download);
                    StatusCode::NOT_FOUND.into_response()
                }
            }
            Content::Codec(codec) => secure(
                plain_libs::axum::Json(plain_libs::serde_json::json!({"codec":codec})).into_response(),
                None,
                false,
            ),
            Content::Thumbnail { bytes, name } => secure(
                file_response::thumbnail(
                    bytes,
                    headers
                        .get(header::IF_NONE_MATCH)
                        .and_then(|v| v.to_str().ok()),
                ),
                Some(&name),
                false,
            ),
            Content::NotFound => StatusCode::NOT_FOUND.into_response(),
            Content::Directory => StatusCode::BAD_REQUEST.into_response(),
            Content::Markdown { text, name } => secure(
                (
                    [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                    text,
                )
                    .into_response(),
                Some(&name),
                false,
            ),
            Content::TranscodingUnavailable => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "video transcoding is not available for this file",
            )
                .into_response(),
        };
        Ok::<_, plain_libs::anyhow::Error>(response)
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
#[path = "../../../tests/plain-server/unit/http/fs.rs"]
mod tests;
