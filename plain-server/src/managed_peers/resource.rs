use super::{PeerRoute, request_error};
use crate::{native::resources::MAX_RESOURCE_RESPONSE_BYTES, server::ServerState};
use axum::http::{Method, StatusCode, header};
use plain_libs::protocols::http_client;
use plain_rs::features::{
    files::resource_link::{self, Resource, Scope},
    services::files::content::Size,
};
use std::sync::Arc;

pub(crate) const MANAGED_RESOURCE: &str = "x-plain-managed-resource";

pub(crate) struct PeerResource {
    state: ServerState,
    route: Arc<PeerRoute>,
    url: http_client::Url,
    pub(crate) size: u64,
    pub(crate) mime: String,
    prepared: Option<plain_libs::temporary::TempPath>,
}

impl PeerResource {
    pub(crate) async fn open(
        state: ServerState,
        peer_id: &str,
        reference: &str,
        thumbnail: Option<Size>,
    ) -> Result<Self, String> {
        let route = state
            .managed_proxy
            .peers
            .route(&state, peer_id)
            .await
            .map_err(request_error)?;
        if !route.supports_managed_resources {
            return Err("Managed resources unavailable".into());
        }
        route.check_current(&state).map_err(request_error)?;
        let id = resource_link::seal(
            &Scope::Session(route.client_id.clone()),
            &route.url_key,
            &Resource::reference(reference),
        )
        .map_err(request_error)?;
        let mut url = route.base.join("/fs").map_err(|error| error.to_string())?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("id", &id).append_pair(
                "action",
                if thumbnail.is_some() {
                    "preview"
                } else {
                    "download"
                },
            );
            if let Some(size) = thumbnail {
                query.append_pair(
                    "size",
                    match size {
                        Size::Small => "sm",
                        Size::Large => "lg",
                    },
                );
            }
        }
        let response = request(&route, Method::HEAD, url.clone())
            .send()
            .await
            .map_err(|error| error.to_string())?;
        validate(&response, StatusCode::OK)?;
        let size = response
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or("Resource length missing")?;
        let mime = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .ok_or("Resource type missing")?
            .to_owned();
        let range_supported = response
            .headers()
            .get(header::ACCEPT_RANGES)
            .is_some_and(|value| value == "bytes");
        route.check_current(&state).map_err(request_error)?;
        let mut resource = Self {
            state,
            route,
            url,
            size,
            mime,
            prepared: None,
        };
        if !range_supported && size <= MAX_RESOURCE_RESPONSE_BYTES {
            use futures_util::StreamExt;
            let response = resource.download().await?;
            let stream = response.bytes_stream().map(|chunk| {
                resource.check_current()?;
                chunk.map_err(|error| error.to_string())
            });
            let temporary = resource
                .state
                .remote_cache
                .stage_stream_in(
                    &resource.state.cache.root().join("native_resources"),
                    size,
                    stream,
                )
                .await?;
            resource.prepared = Some(temporary.into_temp_path());
        }
        Ok(resource)
    }

    pub(crate) fn check_current(&self) -> Result<(), String> {
        self.state.native_client_id()?;
        self.route.check_current(&self.state).map_err(request_error)
    }

    pub(crate) async fn download(&self) -> Result<http_client::Response, String> {
        self.check_current()?;
        let response = request(&self.route, Method::GET, self.url.clone())
            .send()
            .await
            .map_err(|error| error.to_string())?;
        validate(&response, StatusCode::OK)?;
        self.check_current()?;
        if response.content_length() != Some(self.size) {
            return Err("Resource length changed".into());
        }
        Ok(response)
    }

    pub(crate) async fn read_range(&self, offset: u64, length: u64) -> Result<Vec<u8>, String> {
        self.check_current()?;
        if length > MAX_RESOURCE_RESPONSE_BYTES
            || offset.checked_add(length).is_none_or(|end| end > self.size)
        {
            return Err("Invalid resource range".into());
        }
        if length == 0 {
            return Ok(Vec::new());
        }
        let mut stop = self.state.stop.clone();
        tokio::select! {
            biased;
            _ = stop.wait_for(|stopped| *stopped) => Err("Rust core stopped".into()),
            result = async {
                if let Some(prepared) = &self.prepared {
                    use tokio::io::{AsyncReadExt, AsyncSeekExt};
                    let mut file = tokio::fs::File::open(prepared.as_ref()).await.map_err(|error| error.to_string())?;
                    self.check_current()?;
                    file.seek(std::io::SeekFrom::Start(offset)).await.map_err(|error| error.to_string())?;
                    let mut bytes = vec![0; length as usize];
                    for chunk in bytes.chunks_mut(crate::native::resources::MAX_RESOURCE_READ_BYTES) {
                        file.read_exact(chunk).await.map_err(|error| error.to_string())?;
                    }
                    self.check_current()?;
                    return Ok(bytes);
                }
                let end = offset + length - 1;
                let mut response = request(&self.route, Method::GET, self.url.clone())
                    .header(header::RANGE, format!("bytes={offset}-{end}"))
                    .send().await.map_err(|error| error.to_string())?;
                validate(&response, StatusCode::PARTIAL_CONTENT)?;
                let expected_range = format!("bytes {offset}-{end}/{}", self.size);
                if response.content_length() != Some(length) || response.headers().get(header::CONTENT_RANGE).and_then(|value| value.to_str().ok()) != Some(expected_range.as_str()) {
                    return Err("Resource range response mismatch".into());
                }
                let mut bytes = Vec::with_capacity(length as usize);
                while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
                    self.check_current()?;
                    if chunk.len() > length as usize - bytes.len() { return Err("Resource exceeds requested range".into()); }
                    bytes.extend_from_slice(&chunk);
                }
                self.check_current()?;
                if bytes.len() as u64 != length { return Err("Resource ended before requested range".into()); }
                Ok(bytes)
            } => result,
        }
    }
}

fn request(
    route: &PeerRoute,
    method: Method,
    url: http_client::Url,
) -> http_client::RequestBuilder {
    route
        .client
        .request(method, url)
        .header("c-id", &route.client_id)
        .header(MANAGED_RESOURCE, "1")
        .bearer_auth(&route.peer.token)
}

fn validate(response: &http_client::Response, status: StatusCode) -> Result<(), String> {
    match response.status() {
        StatusCode::UNAUTHORIZED => return Err("unauthorized".into()),
        StatusCode::FORBIDDEN => return Err("remote_access_disabled".into()),
        actual if actual != status => return Err(format!("Resource response HTTP {actual}")),
        _ => {}
    }
    if response
        .headers()
        .get(MANAGED_RESOURCE)
        .and_then(|value| value.to_str().ok())
        != Some("1")
    {
        return Err("Managed resource receipt missing".into());
    }
    if response.headers().contains_key(header::CONTENT_ENCODING) {
        return Err("Encoded resource response denied".into());
    }
    Ok(())
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../tests/plain-server/unit/managed_peers/resource.rs"]
mod tests;
