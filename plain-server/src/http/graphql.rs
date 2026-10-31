use crate::server::ServerState;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use serde_json::json;
use subtle::ConstantTimeEq;

fn bearer_value(value: &str) -> &str {
    value.split(' ').nth(1).unwrap_or("")
}

fn valid_custom_bearer(value: &str, session_token: &str) -> bool {
    let bearer = bearer_value(value);
    !bearer.is_empty()
        && !session_token.is_empty()
        && bearer
            .as_bytes()
            .ct_eq(session_token.as_bytes())
            .unwrap_u8()
            == 1
}

/// Answer a development request: the literal client id `dev`, plaintext in
/// and out. Mirrors the server dev branch the legacy router provided, so the
/// NAS keeps this entry point after moving to the public router.
async fn run_development(state: &ServerState, body: Bytes) -> Result<Response, StatusCode> {
    let request = String::from_utf8(body.to_vec()).map_err(|_| StatusCode::BAD_REQUEST)?;
    let result = run_with_context(
        &state.public,
        &request,
        crate::graphql::executor::RequestContext::authenticated_remote(
            state.clone(),
            "dev".to_owned(),
        ),
    )
    .await?;
    Ok(result.into_response())
}

/// Does this `Authorization` header present the server development token?
/// An unset token never matches, and the comparison is constant time so a
/// wrong guess cannot learn the real one from response timing.
pub(crate) fn dev_bearer_matches(state: &ServerState, authorization: &str) -> Result<bool, ()> {
    let expected = plain_rs::prefs::security::dev_token(&state.prefs).map_err(|_| ())?;
    if expected.is_empty() {
        return Ok(false);
    }
    Ok(authorization
        .strip_prefix("Bearer ")
        .is_some_and(|token| token.as_bytes().ct_eq(expected.as_bytes()).into()))
}

/// Answer the core's own shell. There is no session and therefore no session
/// key, so the body travels in plaintext on the loopback listener.
async fn run_local(state: &ServerState, body: Bytes) -> Result<Response, StatusCode> {
    let request = String::from_utf8(body.to_vec()).map_err(|_| StatusCode::BAD_REQUEST)?;
    let client_id = state
        .native_client_id()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let result = run_with_context(
        &state.public,
        &request,
        crate::graphql::executor::RequestContext::authenticated_local(state.clone(), client_id),
    )
    .await?;
    Ok(result.into_response())
}

async fn execute(
    state: &ServerState,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    let client_id = headers
        .get("c-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    if state.authenticated(headers) {
        return run_local(state, body).await;
    }
    if !state.prefs.get_or("desktop_access", true) {
        return Err(StatusCode::NOT_FOUND);
    }
    // The server development bearer. It stands in for a session so the server
    // can be driven before setup, and it answers in plaintext for the same
    // reason — no session key to encrypt with.
    let dev_bearer = match authorization {
        Some(value) => {
            dev_bearer_matches(state, value).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        }
        None => false,
    };
    if dev_bearer && !client_id.is_empty() {
        return run_development(&state, body).await;
    }

    if client_id == state.native_client_id().unwrap_or_default() {
        if !state.authenticated(headers) {
            return Err(StatusCode::UNAUTHORIZED);
        }
        return run_local(&state, body).await;
    }

    if client_id.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let session = state
        .db
        .session_get(client_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let (token_mode, key) = if authorization.is_none() {
        let key = plain_rs::services::sessions::key(&state.db, client_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::UNAUTHORIZED)?;
        (true, key)
    } else {
        if session.r#type != "CUSTOM"
            || !valid_custom_bearer(authorization.unwrap(), &session.token)
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        (false, Vec::new())
    };

    let request = if token_mode {
        let decrypted =
            plain_rs::crypto::xchacha_decrypt_raw(&key, &body).ok_or(StatusCode::UNAUTHORIZED)?;
        if decrypted.is_empty() {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let plaintext = String::from_utf8_lossy(&decrypted).into_owned();
        let now = chrono::Utc::now().timestamp_millis();
        state
            .main_replay
            .admit(client_id, &plaintext, now)
            .map_err(|_| StatusCode::BAD_REQUEST)?
            .to_owned()
    } else {
        String::from_utf8(body.to_vec()).map_err(|_| StatusCode::BAD_REQUEST)?
    };

    plain_rs::services::sessions::touch(state, client_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let result = run_with_context(
        &state.public,
        &request,
        crate::graphql::executor::RequestContext::authenticated_remote(
            state.clone(),
            client_id.to_owned(),
        ),
    )
    .await?;
    let mut response = if token_mode {
        let encrypted = plain_rs::transport::http_response::Context::new("/graphql", &body)
            .encrypt(&key, &result)
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        encrypted.into_response()
    } else {
        result.into_response()
    };
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        CONTENT_TYPE,
        if token_mode {
            "application/octet-stream"
        } else {
            "application/json"
        }
        .parse()
        .unwrap(),
    );
    Ok(response)
}

/// Executes a decrypted request envelope against the contract schema.
///
/// Split out of [`execute`] so the flip itself — Rust answers `/graphql`,
/// nothing is handed to the platform — is testable without standing up a
/// whole server: a document that queries the schema and never reaches the
/// host is the thing to pin down.
#[cfg(test)]
pub(crate) async fn run(
    schema: &crate::graphql::schemas::public::PublicSchema,
    request: &str,
    state: Option<ServerState>,
) -> Result<Vec<u8>, StatusCode> {
    run_with_context(
        schema,
        request,
        crate::graphql::executor::RequestContext::remote(state),
    )
    .await
}

async fn run_with_context(
    schema: &crate::graphql::schemas::public::PublicSchema,
    request: &str,
    context: crate::graphql::executor::RequestContext,
) -> Result<Vec<u8>, StatusCode> {
    let parsed: async_graphql::Request =
        serde_json::from_str(request).map_err(|_| StatusCode::BAD_REQUEST)?;
    serde_json::to_vec(&crate::graphql::executor::execute_graphql(schema, parsed, context).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub(crate) async fn call(
    State(state): State<ServerState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match execute(&state, &headers, body).await {
        Ok(response) => response,
        Err(status) => status.into_response(),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/http/graphql.rs"]
mod tests;

pub(crate) async fn peer(
    State(state): State<ServerState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let id = headers
        .get("c-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let cid = headers
        .get("c-cid")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let (status, body) = crate::graphql::schemas::peer::execute(&state, id, cid, &body).await;
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        [("content-type", "application/octet-stream")],
        body,
    )
        .into_response()
}

async fn execute_guest(state: &ServerState, id: &str, body: &[u8]) -> (u16, Vec<u8>) {
    if id.is_empty() {
        return (401, vec![]);
    }
    let shares = plain_rs::files::shares::Service::new(state.db.clone(), state.prefs.clone());
    if !matches!(shares.active(id, true), Ok(Some(_))) {
        return (403, vec![]);
    }
    let key = match shares
        .token(id)
        .and_then(|key| plain_rs::utils::base64::base64_decode_checked(&key).map_err(Into::into))
    {
        Ok(key) => key,
        Err(_) => return (500, vec![]),
    };
    let plain = match plain_rs::crypto::xchacha_decrypt_raw(&key, body)
        .and_then(|p| String::from_utf8(p).ok())
    {
        Some(p) if !p.is_empty() => p,
        _ => return (401, vec![]),
    };
    let context = plain_rs::transport::http_response::Context::new("/guest_graphql", body);
    let now = chrono::Utc::now().timestamp_millis();
    let body = match state.guest_replay.admit(id, &plain, now) {
        Ok(p) => p,
        Err(_) => return (400, vec![]),
    };
    let request: async_graphql::Request = match serde_json::from_str(body) {
        Ok(p) => p,
        Err(_) => return (400, vec![]),
    };
    if let Err(error) = state.host.notify("systemWebActivity", json!({})) {
        log::warn!("Web activity callback: {error}");
    }
    let schema = crate::graphql::schemas::guest::schema(state.clone(), id.to_owned());
    let result = schema.execute(request).await;
    match serde_json::to_vec(&result)
        .ok()
        .and_then(|p| context.encrypt(&key, &p))
    {
        Some(body) => (200, body),
        None => (500, vec![]),
    }
}

pub(crate) async fn guest(
    State(state): State<ServerState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let id = headers
        .get("c-id")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    let (status, body) = execute_guest(&state, id, &body).await;
    (
        StatusCode::from_u16(status).unwrap(),
        [("content-type", "application/octet-stream")],
        body,
    )
        .into_response()
}

#[cfg(test)]
#[path = "../../tests/unit/http/graphql/guest.rs"]
mod guest_tests;
