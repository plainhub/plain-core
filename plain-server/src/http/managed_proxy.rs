use crate::{http::ConnectionScheme, server::ServerState};
use anyhow::{Result, anyhow, ensure};
use axum::{
    body::{Body, to_bytes},
    extract::{Path, Request, State},
    http::{HeaderMap, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use plain_libs::protocols::http_client;
use plain_rs::{
    features::{
        access::AccessFeature,
        files::resource_link::{self, Resource, Scope},
        services::{
            device::access,
            peers::{management_http, peer_address},
        },
    },
    infrastructure::{
        db::{DPeer, SessionRow, chat_store::peers},
        transport::http_response,
    },
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;

mod socket;
mod upload;

const GRAPHQL_LIMIT: usize = 4 * 1024 * 1024;
const ROUTE_LIFETIME: Duration = Duration::from_secs(30);
pub(super) const MANAGED_RESOURCE: &str = "x-plain-managed-resource";
type RouteSlot = Arc<tokio::sync::Mutex<Option<Arc<PeerRoute>>>>;

#[derive(Default)]
pub(crate) struct ManagedProxy {
    routes: Mutex<HashMap<String, RouteSlot>>,
}

struct PeerRoute {
    peer: DPeer,
    base: http_client::Url,
    client_id: String,
    key: Vec<u8>,
    url_key: Vec<u8>,
    pin: [u8; 32],
    client: http_client::Client,
    supports_managed_resources: bool,
    expires: Instant,
}

impl ManagedProxy {
    async fn route(&self, state: &ServerState, target: &str) -> Result<Arc<PeerRoute>> {
        let peer = peers::get(&state.db, target)?
            .filter(|peer| peer.is_paired() && !peer.token.is_empty())
            .ok_or_else(|| anyhow!("Managed device unavailable"))?;
        let client_id = state.native_client_id().map_err(anyhow::Error::msg)?;
        ensure!(target != client_id, "Invalid managed target");
        let slot = self
            .routes
            .lock()
            .unwrap()
            .entry(target.into())
            .or_default()
            .clone();
        let mut cached = slot.lock().await;
        if let Some(route) = cached.as_ref().filter(|route| {
            route.expires > Instant::now()
                && route.peer.ip == peer.ip
                && route.peer.port == peer.port
                && route.peer.token == peer.token
                && peer_address::same_identity(&route.peer, &peer)
        }) {
            return Ok(route.clone());
        }
        let base = http_client::Url::parse(&peer_address::address(&peer).base_url)?;
        let key = decode_key(&peer.token)?;
        let data = management_http::query(
            state,
            &base,
            &client_id,
            &key,
            json!({"query":"{ app { clientId urlToken tlsCertificateSha256 } __type(name: \"Peer\") { fields { name } } }"}),
            GRAPHQL_LIMIT,
        )
        .await?;
        let app = &data["app"];
        ensure!(
            app["clientId"].as_str() == Some(target),
            "Managed device identity changed"
        );
        let url_key = decode_key(app["urlToken"].as_str().unwrap_or_default())?;
        let pin = decode_pin(app["tlsCertificateSha256"].as_str().unwrap_or_default())?;
        let supports_managed_resources = data["__type"]["fields"]
            .as_array()
            .is_some_and(|fields| fields.iter().any(|field| field["name"] == "managed"));
        let client = if let Some(route) = cached
            .as_ref()
            .filter(|route| route.base == base && route.pin == pin)
        {
            route.client.clone()
        } else {
            http_client::builder()
                .no_proxy()
                .https_only(true)
                .pin_certificate(pin)
                .redirect(http_client::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .read_timeout(Duration::from_secs(30))
                .pool_max_idle_per_host(8)
                .build()?
        };
        let route = Arc::new(PeerRoute {
            peer,
            base,
            client_id,
            key,
            url_key,
            pin,
            client,
            supports_managed_resources,
            expires: Instant::now() + ROUTE_LIFETIME,
        });
        route.check_current(state)?;
        *cached = Some(route.clone());
        Ok(route)
    }
}

impl PeerRoute {
    fn check_current(&self, state: &ServerState) -> Result<()> {
        let peer = peer_address::current(&state.db, &self.peer.id, &self.peer)?;
        ensure!(
            peer.is_paired() && peer.token == self.peer.token,
            "Management session revoked"
        );
        Ok(())
    }
}

pub(crate) async fn call(
    State(state): State<ServerState>,
    Path((target, path)): Path<(String, String)>,
    request: Request,
) -> Response {
    if !same_origin(&request) || !remote_access(&state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if path == "graphql" && request.method() == Method::GET {
        return socket::upgrade(state, target, request).await;
    }
    match (request.method().clone(), path.as_str()) {
        (Method::POST, "graphql") => graphql(state, &target, request).await,
        (Method::POST, "upload" | "upload_chunk") => {
            upload::call(state, &target, &path, request).await
        }
        (Method::GET, "fs" | "zip/dir" | "zip/files") => {
            resource(state, &target, &path, request).await
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

fn same_origin(request: &Request) -> bool {
    if request
        .extensions()
        .get::<ConnectionScheme>()
        .map(|scheme| scheme.0)
        != Some("https")
    {
        return false;
    }
    let Some(origin) = request.headers().get(header::ORIGIN) else {
        return true;
    };
    let origin = origin
        .to_str()
        .ok()
        .and_then(|value| http_client::Url::parse(value).ok());
    let host = request
        .uri()
        .authority()
        .map(|authority| authority.as_str())
        .or_else(|| {
            request
                .headers()
                .get(header::HOST)
                .and_then(|value| value.to_str().ok())
        });
    let expected = host.and_then(|host| http_client::Url::parse(&format!("https://{host}")).ok());
    origin
        .zip(expected)
        .is_some_and(|(actual, expected)| actual.origin() == expected.origin())
}

fn remote_access(state: &ServerState) -> bool {
    state.prefs.get_or("remote_access", true)
}

fn caller(state: &ServerState, id: &str) -> Result<SessionRow> {
    ensure!(remote_access(state), "no_permission");
    state
        .db
        .session_get(id)?
        .filter(|session| session.r#type == plain_rs::features::sessions::SessionType::Login)
        .ok_or_else(|| anyhow!("Session unavailable"))
}

fn current(state: &ServerState, original: &SessionRow) -> Result<()> {
    let session = caller(state, &original.client_id)?;
    ensure!(
        session.token == original.token && session.url_token == original.url_token,
        "Session revoked"
    );
    Ok(())
}

fn decode_key(token: &str) -> Result<Vec<u8>> {
    let key = plain_libs::utils::base64::base64_decode_checked(token)?;
    ensure!(key.len() == 32, "Invalid session key");
    Ok(key)
}

fn decode_pin(hex: &str) -> Result<[u8; 32]> {
    ensure!(
        hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid certificate fingerprint"
    );
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)?;
    }
    Ok(digest)
}

fn caller_id(headers: &HeaderMap) -> &str {
    headers
        .get("c-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
}

pub(super) fn forwarded_resource(
    state: &ServerState,
    request: &axum::http::request::Parts,
    id: &str,
) -> Result<bool, StatusCode> {
    if !request.headers.contains_key(MANAGED_RESOURCE) {
        return Ok(false);
    }
    let client = caller_id(&request.headers);
    let allowed = request
        .extensions
        .get::<ConnectionScheme>()
        .is_some_and(|scheme| scheme.0 == "https")
        && access::is_enabled(&state.prefs, &[AccessFeature::Files])
        && matches!(resource_link::parse(id), Ok((Scope::Session(owner), _)) if owner == client)
        && caller(state, client).is_ok_and(|session| {
            request
                .headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
                .is_some_and(|token| {
                    !token.is_empty()
                        && bool::from(token.as_bytes().ct_eq(session.token.as_bytes()))
                })
        });
    if !allowed {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(true)
}

pub(super) fn forwarded_upload_allowed(state: &ServerState, request: &Request) -> bool {
    !request.headers().contains_key(MANAGED_RESOURCE)
        || (request
            .extensions()
            .get::<ConnectionScheme>()
            .is_some_and(|scheme| scheme.0 == "https")
            && access::is_enabled(&state.prefs, &[AccessFeature::Files]))
}

pub(super) fn mark_forwarded(mut response: Response, managed: bool) -> Response {
    if managed {
        response
            .headers_mut()
            .insert(MANAGED_RESOURCE, axum::http::HeaderValue::from_static("1"));
    }
    response
}

async fn graphql(state: ServerState, target: &str, request: Request) -> Response {
    let Ok(session) = caller(&state, caller_id(request.headers())) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(key) = decode_key(&session.token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(body) = to_bytes(request.into_body(), GRAPHQL_LIMIT).await else {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };
    let Some(plain) = plain_libs::crypto::xchacha_decrypt_raw(&key, &body) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(text) = std::str::from_utf8(&plain) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(text) = state.main_replay.admit(
        &session.client_id,
        text,
        chrono::Utc::now().timestamp_millis(),
    ) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(query) = serde_json::from_str::<plain_graphql::Request>(text) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(features) = authorize_graphql(&state, &query, false) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let Ok(route) = state.managed_proxy.route(&state, target).await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if current(&state, &session).is_err()
        || route.check_current(&state).is_err()
        || !access::is_enabled(&state.prefs, &features)
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(url) = route.base.join("/graphql") else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let mut outgoing = route.client.post(url).header("c-id", &route.client_id);
    if features.contains(&AccessFeature::Files) {
        if !route.supports_managed_resources {
            return StatusCode::BAD_GATEWAY.into_response();
        }
        outgoing = outgoing.header(MANAGED_RESOURCE, "1");
    }
    let response = management_http::exchange(outgoing, &route.key, text, GRAPHQL_LIMIT).await;
    if current(&state, &session).is_err()
        || route.check_current(&state).is_err()
        || !access::is_enabled(&state.prefs, &features)
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let response = match response {
        Ok(response) => response,
        Err(error)
            if matches!(
                error.downcast_ref::<management_http::AuthorizationError>(),
                Some(management_http::AuthorizationError::Permission)
            ) =>
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
    };
    let Some(encrypted) = http_response::Context::new("/graphql", &body).encrypt(&key, &response)
    else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        encrypted,
    )
        .into_response()
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../tests/plain-server/unit/http/managed_proxy.rs"]
mod tests;

fn authorize_graphql(
    state: &ServerState,
    request: &plain_graphql::Request,
    subscription: bool,
) -> Result<Vec<AccessFeature>> {
    use plain_graphql::parser::{
        parse_query,
        types::{OperationType, Selection},
    };
    let document = parse_query(&request.query)?;
    let mut roots = Vec::new();
    let mut kind = None;
    for (name, operation) in document.operations.iter() {
        if request
            .operation_name
            .as_deref()
            .is_some_and(|selected| name.is_none_or(|name| name.as_str() != selected))
        {
            continue;
        }
        ensure!(
            kind.replace(operation.node.ty).is_none(),
            "Operation name required"
        );
        roots.extend(&operation.node.selection_set.node.items);
    }
    let kind = kind.ok_or_else(|| anyhow!("Operation name required"))?;
    ensure!(
        (kind == OperationType::Subscription) == subscription,
        "Invalid managed operation"
    );
    let mut pending = roots
        .into_iter()
        .map(|selection| (selection, true))
        .collect::<Vec<_>>();
    let mut fragments = std::collections::HashSet::new();
    let mut features = Vec::new();
    while let Some((selection, root)) = pending.pop() {
        match &selection.node {
            Selection::Field(field) => {
                let name = field.node.name.node.as_str();
                ensure!(
                    !matches!(name, "urlToken" | "token") && !name.starts_with("__"),
                    "Managed credential field denied"
                );
                if root {
                    let feature = match (kind, name) {
                        (OperationType::Query, "app") => None,
                        (OperationType::Query, "docs" | "docCount") => Some(AccessFeature::Docs),
                        (OperationType::Query, "images" | "imageCount") => {
                            Some(AccessFeature::Images)
                        }
                        (OperationType::Query, "videos" | "videoCount") => {
                            Some(AccessFeature::Videos)
                        }
                        (OperationType::Query, "audios" | "audioCount") => {
                            Some(AccessFeature::Audio)
                        }
                        (
                            OperationType::Query,
                            "files" | "fileCount" | "fileInfo" | "indexedFiles"
                            | "indexedFileCount" | "mounts" | "recentFiles" | "pathExists"
                            | "pathKind" | "uploadedChunks" | "mergeStatus",
                        )
                        | (OperationType::Mutation, "mergeChunks" | "deleteChunks")
                        | (
                            OperationType::Subscription,
                            "accessUpdated" | "mediaScanProgress" | "uploadMergeResult",
                        ) => Some(AccessFeature::Files),
                        _ => return Err(anyhow!("Managed operation denied")),
                    };
                    if let Some(feature) = feature.filter(|feature| !features.contains(feature)) {
                        features.push(feature);
                    }
                }
                pending.extend(
                    field
                        .node
                        .selection_set
                        .node
                        .items
                        .iter()
                        .map(|item| (item, false)),
                );
            }
            Selection::InlineFragment(fragment) => pending.extend(
                fragment
                    .node
                    .selection_set
                    .node
                    .items
                    .iter()
                    .map(|item| (item, root)),
            ),
            Selection::FragmentSpread(spread) => {
                let name = &spread.node.fragment_name.node;
                if fragments.insert((name.clone(), root)) {
                    let fragment = document
                        .fragments
                        .get(name)
                        .ok_or_else(|| anyhow!("Unknown fragment"))?;
                    pending.extend(
                        fragment
                            .node
                            .selection_set
                            .node
                            .items
                            .iter()
                            .map(|item| (item, root)),
                    );
                }
            }
        }
    }
    ensure!(access::is_enabled(&state.prefs, &features), "no_permission");
    Ok(features)
}

async fn resource(state: ServerState, target: &str, path: &str, request: Request) -> Response {
    if !access::is_enabled(&state.prefs, &[AccessFeature::Files]) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(axum::extract::Query(parameters)) =
        axum::extract::Query::<crate::http::fs::Parameters>::try_from_uri(request.uri())
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if !parameters.is_valid()
        || (path != "fs"
            && parameters.action != plain_rs::features::services::files::content::Action::Download)
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Ok((Scope::Session(id), payload)) = resource_link::parse(&parameters.id) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(session) = caller(&state, &id) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(url_key) = decode_key(&session.url_token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let purpose = format!("managed:{target}:{path}:{}", parameters.action.as_str());
    let Ok(plain) = resource_link::decrypt(&Scope::Session(id), &purpose, &url_key, payload) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(route) = state.managed_proxy.route(&state, target).await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if !route.supports_managed_resources {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    if current(&state, &session).is_err()
        || route.check_current(&state).is_err()
        || !access::is_enabled(&state.prefs, &[AccessFeature::Files])
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let scope = Scope::Session(route.client_id.clone());
    let id = if path == "zip/files" {
        #[derive(serde::Deserialize)]
        struct Selection {
            r#type: plain_rs::features::enums::DataType,
        }
        use plain_rs::features::enums::DataType;
        serde_json::from_slice::<Selection>(&plain)
            .map_err(anyhow::Error::from)
            .and_then(|selection| {
                ensure!(
                    matches!(
                        selection.r#type,
                        DataType::Audio | DataType::Video | DataType::Image | DataType::Doc
                    ),
                    "Invalid managed archive selection"
                );
                resource_link::encrypt(&scope, "zip-files", &route.url_key, &plain)
            })
    } else {
        serde_json::from_slice::<Resource>(&plain)
            .map_err(anyhow::Error::from)
            .and_then(|resource| {
                use plain_rs::features::files::resource_link::ResourceType;
                ensure!(
                    matches!(resource.r#type, ResourceType::File | ResourceType::Media),
                    "Invalid managed resource"
                );
                resource_link::seal(&scope, &route.url_key, &resource)
            })
    };
    let Ok(id) = id else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(mut url) = route.base.join(&format!("/{path}")) else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("id", &id);
        if path == "fs" {
            query.append_pair("action", parameters.action.as_str());
            if let Some(size) = parameters.size {
                use plain_rs::features::services::files::content::Size;
                query.append_pair(
                    "size",
                    match size {
                        Size::Small => "sm",
                        Size::Large => "lg",
                    },
                );
            }
            if let Some(version) = parameters.version {
                query.append_pair("v", &version);
            }
        }
    }
    let outgoing = route
        .client
        .get(url)
        .header("c-id", &route.client_id)
        .header(MANAGED_RESOURCE, "1")
        .bearer_auth(&route.peer.token);
    relay(outgoing, request.headers()).await
}

async fn relay(mut request: http_client::RequestBuilder, headers: &HeaderMap) -> Response {
    for name in [
        header::RANGE,
        header::IF_RANGE,
        header::IF_NONE_MATCH,
        header::IF_MODIFIED_SINCE,
    ] {
        if let Some(value) = headers.get(&name) {
            request = request.header(name, value.clone());
        }
    }
    let Ok(response) = request.send().await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let status = response.status();
    if (status.is_success() || status == StatusCode::NOT_MODIFIED)
        && !response.headers().contains_key(MANAGED_RESOURCE)
    {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    if status.is_redirection() && status != StatusCode::NOT_MODIFIED {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    let mut result = Response::builder()
        .status(status)
        .header(header::CACHE_CONTROL, "private, no-cache")
        .header("x-content-type-options", "nosniff")
        .header("referrer-policy", "no-referrer")
        .header(
            header::CONTENT_SECURITY_POLICY,
            "sandbox; default-src 'none'",
        );
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_LENGTH,
        header::CONTENT_DISPOSITION,
        header::CONTENT_RANGE,
        header::ACCEPT_RANGES,
        header::ETAG,
        header::LAST_MODIFIED,
    ] {
        if let Some(value) = response.headers().get(&name) {
            result = result.header(name, value.clone());
        }
    }
    result
        .body(Body::from_stream(response.bytes_stream()))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}
