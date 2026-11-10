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
        services::{device::access, peers::management_http},
    },
    infrastructure::{db::SessionRow, transport::http_response},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use subtle::ConstantTimeEq;

mod attachment;
mod raw;
mod socket;
mod upload;

pub(super) use crate::managed_peers::resource::MANAGED_RESOURCE;
use crate::managed_peers::{GRAPHQL_LIMIT, PeerRoute, ROUTE_LIFETIME, decode_key, decode_pin};
#[derive(Default)]
pub(crate) struct ManagedProxy {
    pub(crate) peers: crate::managed_peers::ManagedPeerRoutes,
    attachments: Mutex<HashMap<String, attachment::RouteSlot>>,
    native_client: std::sync::OnceLock<http_client::Client>,
}

impl ManagedProxy {
    async fn route(&self, state: &ServerState, target: &str) -> Result<Arc<PeerRoute>> {
        self.peers.route(state, target).await
    }
}

pub(crate) async fn call(
    State(state): State<ServerState>,
    Path((target, path)): Path<(String, String)>,
    request: Request,
) -> Response {
    if native_loopback(&state, &request) {
        if path == "graphql" {
            return StatusCode::FORBIDDEN.into_response();
        }
        if target.starts_with("c.") {
            return raw::native(state, &target, request).await;
        }
        if request.headers().contains_key(header::UPGRADE) {
            return raw::native_socket(state, &target, request).await;
        }
    }
    if request.method() == Method::GET && path == "fs" {
        return resource(state, &target, &path, request).await;
    }
    if !same_origin(&request) || !remote_access(&state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if path == "auth" && request.method() == Method::GET {
        return raw::auth(state, target, request).await;
    }
    if path == "graphql" && request.method() == Method::GET {
        return socket::upgrade(state, target, request).await;
    }
    match (request.method().clone(), path.as_str()) {
        (Method::POST, "graphql") => graphql(state, &target, request).await,
        (Method::POST, "upload" | "upload_chunk") => {
            upload::call(state, &target, &path, request).await
        }
        (Method::GET, "zip/dir" | "zip/files") => resource(state, &target, &path, request).await,
        _ => raw::call(state, &target, &path, request).await,
    }
}

pub(super) fn native_loopback(state: &ServerState, request: &Request) -> bool {
    use axum::extract::ConnectInfo;
    use std::net::SocketAddr;
    let host = request
        .uri()
        .authority()
        .map(|host| host.as_str())
        .or_else(|| {
            request
                .headers()
                .get(header::HOST)
                .and_then(|host| host.to_str().ok())
        });
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|peer| peer.0.ip().is_loopback())
        && request
            .extensions()
            .get::<ConnectionScheme>()
            .is_some_and(|scheme| scheme.0 == "http")
        && host
            .and_then(|host| host.parse::<axum::http::uri::Authority>().ok())
            .is_some_and(|authority| {
                matches!(authority.host(), "localhost" | "127.0.0.1" | "[::1]")
            })
        && request.headers().get(header::ORIGIN).is_none_or(|origin| {
            matches!(
                origin.as_bytes(),
                b"tauri://localhost" | b"http://tauri.localhost" | b"https://tauri.localhost"
            ) || (state.build_debug.load(std::sync::atomic::Ordering::Relaxed)
                && origin.as_bytes() == b"http://localhost:4000")
        })
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

pub(crate) fn caller(state: &ServerState, id: &str) -> Result<SessionRow> {
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
    let Some(plain) = plain_rs::infrastructure::transport::http_request::decrypt_raw(
        &key, "POST", "/graphql", &body,
    ) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(text) = std::str::from_utf8(&plain) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(text) = state.main_replay.admit(
        &state.db,
        &session.client_id,
        text,
        chrono::Utc::now().timestamp_millis(),
    ) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(query) = serde_json::from_str::<plain_graphql::Request>(text) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if authorize_graphql(&query, false).is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(route) = state.managed_proxy.route(&state, target).await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if current(&state, &session).is_err() || route.check_current(&state).is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let response = route.exchange(&state, text, None).await;
    if current(&state, &session).is_err() || route.check_current(&state).is_err() {
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

fn authorize_graphql(request: &plain_graphql::Request, subscription: bool) -> Result<()> {
    use plain_graphql::parser::{
        parse_query,
        types::{OperationType, Selection},
    };
    let document = parse_query(&request.query)?;
    let mut operations = document.operations.iter().filter(|(name, _)| {
        request
            .operation_name
            .as_deref()
            .is_none_or(|selected| name.is_some_and(|name| name.as_str() == selected))
    });
    let (_, operation) = operations
        .next()
        .ok_or_else(|| anyhow!("Operation name required"))?;
    ensure!(operations.next().is_none(), "Operation name required");
    ensure!(
        (operation.node.ty == OperationType::Subscription) == subscription,
        "Invalid managed operation"
    );
    let mut pending: Vec<_> = operation.node.selection_set.node.items.iter().collect();
    let mut fragments = std::collections::HashSet::new();
    while let Some(selection) = pending.pop() {
        match &selection.node {
            Selection::Field(field) => {
                ensure!(
                    !matches!(field.node.name.node.as_str(), "urlToken" | "token"),
                    "Managed credential field denied"
                );
                pending.extend(&field.node.selection_set.node.items);
            }
            Selection::InlineFragment(fragment) => {
                pending.extend(&fragment.node.selection_set.node.items)
            }
            Selection::FragmentSpread(spread) => {
                let name = &spread.node.fragment_name.node;
                if fragments.insert(name) {
                    let fragment = document
                        .fragments
                        .get(name)
                        .ok_or_else(|| anyhow!("Unknown fragment"))?;
                    pending.extend(&fragment.node.selection_set.node.items);
                }
            }
        }
    }
    Ok(())
}

async fn resource(state: ServerState, target: &str, path: &str, request: Request) -> Response {
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
    let native = path == "fs"
        && native_loopback(&state, &request)
        && state.native_client_id().is_ok_and(|native| native == id);
    let session = if native {
        None
    } else {
        if !same_origin(&request) {
            return StatusCode::FORBIDDEN.into_response();
        }
        let Ok(session) = caller(&state, &id) else {
            return StatusCode::UNAUTHORIZED.into_response();
        };
        Some(session)
    };
    let url_key = if let Some(session) = &session {
        let Ok(key) = decode_key(&session.url_token) else {
            return StatusCode::UNAUTHORIZED.into_response();
        };
        key
    } else {
        state.url_key.to_vec()
    };
    let scope = Scope::Session(id);
    let purpose = format!("managed:{target}:{path}:{}", parameters.action.as_str());
    let plain = session
        .as_ref()
        .and_then(|_| resource_link::decrypt(&scope, &purpose, &url_key, payload).ok());
    let (Some(session), Some(plain)) = (session.as_ref(), plain) else {
        if path == "fs"
            && let Ok(reference) = resource_link::decrypt(
                &scope,
                &format!("chat-attachment:{target}"),
                &url_key,
                payload,
            )
        {
            return attachment::call(
                &state,
                target,
                parameters,
                &reference,
                session.as_ref(),
                request.headers(),
            )
            .await;
        }
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(route) = state.managed_proxy.route(&state, target).await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if !route.supports_managed_resources {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    if current(&state, session).is_err() || route.check_current(&state).is_err() {
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
    let mut parameters = parameters;
    parameters.id = id;
    let Ok(url) = resource_url(&route.base, path, &parameters) else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let outgoing = route
        .client
        .get(url)
        .header("c-id", &route.client_id)
        .header(MANAGED_RESOURCE, "1")
        .bearer_auth(&route.peer.token);
    relay(outgoing, request.headers(), true).await
}

fn resource_url(
    base: &http_client::Url,
    path: &str,
    parameters: &crate::http::fs::Parameters,
) -> Result<http_client::Url> {
    use plain_rs::features::services::files::content::Size;
    let mut url = base.join(&format!("/{path}"))?;
    let mut query = url.query_pairs_mut();
    query.append_pair("id", &parameters.id);
    if path == "fs" {
        query.append_pair("action", parameters.action.as_str());
        if let Some(size) = parameters.size {
            query.append_pair(
                "size",
                match size {
                    Size::Small => "sm",
                    Size::Large => "lg",
                },
            );
        }
        if let Some(version) = &parameters.version {
            query.append_pair("v", version);
        }
    }
    drop(query);
    Ok(url)
}

async fn relay(
    mut request: http_client::RequestBuilder,
    headers: &HeaderMap,
    managed: bool,
) -> Response {
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
    if managed
        && (status.is_success() || status == StatusCode::NOT_MODIFIED)
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
