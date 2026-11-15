use super::*;
use crate::graphql::executor::RequestOrigin;
use crate::server::ContentServer;
use plain_graphql::Request;

fn fixture() -> (plain_libs::temporary::TempDir, ContentServer) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    state.prefs.set("background_service_enabled", true).unwrap();
    state.prefs.set("remote_access", true).unwrap();
    for id in ["first", "second"] {
        let mut session = plain_rs::features::services::sessions::create(&state, id.into()).unwrap();
        state.db.session_delete(&session.client_id).unwrap();
        session.client_id = id.into();
        state.db.session_save(&session).unwrap();
    }
    (directory, server)
}

async fn execute(
    state: &ServerState,
    caller: &str,
    origin: RequestOrigin,
    query: &str,
) -> plain_graphql::Response {
    state
        .public
        .execute(
            Request::new(query)
                .data(caller.to_owned())
                .data(origin)
                .data(state.clone()),
        )
        .await
}

#[tokio::test]
async fn authenticated_native_and_remote_clients_can_list_session_metadata() {
    let (_directory, server) = fixture();
    let state = server.runtime_state();
    let remote = execute(
        &state,
        "first",
        RequestOrigin::Remote,
        "{ sessions { clientId clientName } }",
    )
    .await;
    assert!(remote.errors.is_empty(), "{:?}", remote.errors);
    let sessions = remote.data.into_json().unwrap()["sessions"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(sessions.len(), 2);
    assert!(sessions.iter().all(|session| session["clientId"] != state.native_client_id().unwrap()));
    assert!(
        sessions
            .iter()
            .any(|session| session["clientId"] == "first")
    );
    assert!(
        sessions
            .iter()
            .any(|session| session["clientId"] == "second")
    );
    let local = execute(
        &state,
        "native",
        RequestOrigin::Local,
        "{ sessions { clientId } }",
    )
    .await;
    assert!(local.errors.is_empty());
    assert_eq!(
        local.data.into_json().unwrap()["sessions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    server.shutdown().await;
}

#[tokio::test]
async fn authenticated_revocation_and_logout_remove_the_keys_used_by_http_authentication() {
    let (_directory, server) = fixture();
    let state = server.runtime_state();
    let revoked = execute(
        &state,
        "first",
        RequestOrigin::Remote,
        "mutation { revokeSession(clientId:\"second\") }",
    )
    .await;
    assert!(revoked.errors.is_empty(), "{:?}", revoked.errors);
    assert!(
        plain_rs::features::services::sessions::key(&state.db, "second")
            .unwrap()
            .is_none()
    );
    let token = state.db.session_get("first").unwrap().unwrap().token;
    let logout = execute(
        &state,
        "first",
        RequestOrigin::Remote,
        "mutation { logout }",
    )
    .await;
    assert!(logout.errors.is_empty(), "{:?}", logout.errors);
    assert_eq!(logout.data.into_json().unwrap()["logout"], true);
    assert!(
        plain_rs::features::services::sessions::key(&state.db, "first")
            .unwrap()
            .is_none()
    );
    let mut headers = plain_libs::axum::http::HeaderMap::new();
    headers.insert("c-id", "first".parse().unwrap());
    headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
    let response = crate::http::graphql::call(
        plain_libs::axum::extract::State(state.clone()),
        headers,
        plain_libs::axum::body::Bytes::from_static(br#"{"query":"{ sessions { clientId } }"}"#),
    )
    .await;
    assert_eq!(response.status(), plain_libs::axum::http::StatusCode::UNAUTHORIZED);
    let records = state.db.audit_events(0, 10, None).unwrap();
    assert!(records.iter().any(|row| row.r#type == "logout"
        && row.client_id.as_deref() == Some("first") && row.client_name == "first"));
    assert!(records.iter().any(|row| row.r#type == "revoke"
        && row.client_id.as_deref() == Some("second") && row.client_name == "second"));
    server.shutdown().await;
}

#[tokio::test]
async fn management_requires_a_trusted_caller_identity() {
    let (_directory, server) = fixture();
    let state = server.runtime_state();
    let missing = state.public.execute("{ sessions { clientId } }").await;
    assert!(!missing.errors.is_empty());
    let revoked = execute(
        &state,
        "native",
        RequestOrigin::Local,
        "mutation { revokeSession(clientId:\"second\") }",
    )
    .await;
    assert!(revoked.errors.is_empty(), "{:?}", revoked.errors);
    assert!(state.db.session_get("second").unwrap().is_none());
    let audit = state.db.audit_events(0, 10, None).unwrap();
    assert_eq!(audit[0].r#type, "revoke");
    assert_eq!(audit[0].client_name, "second");
    assert_eq!(audit[0].client_id.as_deref(), Some("second"));
    server.shutdown().await;
}

#[tokio::test]
async fn retired_create_session_stays_removed_and_native_management_can_mint_tokens() {
    let (_directory, core) = fixture();
    let state = core.runtime_state();
    let before = state.db.session_list().unwrap().len();
    let remote = execute(
        &state,
        "first",
        RequestOrigin::Remote,
        "mutation { createSession(name:\"Rejected remote credential\") { clientId } }",
    )
    .await;
    assert_eq!(remote.errors.len(), 1);
    assert!(remote.errors[0].message.contains("createSession"));
    assert_eq!(state.db.session_list().unwrap().len(), before);
    let created = core
        .native_management()
        .unwrap()
        .create_session("Owned API token".into())
        .unwrap();
    let id = created.client_id.as_str();
    let token = created.token.as_str();
    assert_eq!(plain_libs::utils::base64::base64_decode(token).len(), 32);
    let saved = state.db.session_get(id).unwrap().unwrap();
    assert_eq!(saved.token, token);
    assert_eq!(saved.r#type, plain_rs::features::sessions::SessionType::ApiToken);
    assert_eq!(state.db.session_list().unwrap().len(), before + 1);
    core.stop_core().await;
}

#[tokio::test]
async fn authenticated_browser_and_native_clients_can_create_named_api_tokens() {
    let (_directory, core) = fixture();
    let state = core.runtime_state();
    let before = state.db.session_list().unwrap().len();
    let missing = state.public.execute(r#"mutation { createApiToken(name:"Rejected") { clientId token } }"#).await;
    assert!(!missing.errors.is_empty());
    assert_eq!(state.db.session_list().unwrap().len(), before);
    let blank = core.native_graphql().unwrap().execute(r#"mutation { createApiToken(name:"  ") { clientId token } }"#.into()).await;
    assert!(!blank.errors.is_empty());
    assert_eq!(state.db.session_list().unwrap().len(), before);
    for local in [false, true] {
        let document = r#"mutation { createApiToken(name:" Fixture token ") { clientId token } }"#;
        let created = if local {
            let response = core.native_graphql().unwrap().execute(document.into()).await;
            assert!(response.errors.is_empty(), "{:?}", response.errors);
            response.data.into_json().unwrap()["createApiToken"].clone()
        } else {
            let mut caller = state.db.session_get("first").unwrap().unwrap();
            caller.r#type = SessionType::Login;
            state.db.session_save(&caller).unwrap();
            let key = plain_rs::features::sessions::token_key(&caller.token).unwrap();
            let document = plain_libs::serde_json::json!({"requestId":plain_rs::features::record_id::generate(),"query":document}).to_string();
            let request = plain_rs::infrastructure::transport::http_request::encrypt_raw(&key, "POST", "/graphql", document.as_bytes()).unwrap();
            let mut headers = plain_libs::axum::http::HeaderMap::new();
            headers.insert("c-id", caller.client_id.parse().unwrap());
            let response = crate::http::graphql::call(
                plain_libs::axum::extract::State(state.clone()), headers,
                plain_libs::axum::body::Bytes::from(request.clone()),
            ).await;
            assert_eq!(response.status(), plain_libs::axum::http::StatusCode::OK);
            let body = plain_libs::axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let plain = plain_rs::infrastructure::transport::http_response::Context::new(&request).decrypt(&key, &body).unwrap();
            let response: plain_libs::serde_json::Value = plain_libs::serde_json::from_slice(&plain).unwrap();
            assert!(response.get("errors").is_none(), "{response}");
            response["data"]["createApiToken"].clone()
        };
        let id = created["clientId"].as_str().unwrap();
        let row = state.db.session_get(id).unwrap().unwrap();
        assert_eq!(row.name, "Fixture token");
        assert_eq!(row.r#type, SessionType::ApiToken);
        assert_eq!(row.token, created["token"].as_str().unwrap());
        assert_eq!(plain_rs::features::sessions::token_key(&row.token).unwrap().len(), 32);
        assert!(state.db.session_get(&state.native_client_id().unwrap()).unwrap().is_none());
        state.db.session_delete(id).unwrap();
        assert_eq!(state.db.session_list().unwrap().len(), before);
    }
    core.stop_core().await;
}

#[tokio::test]
async fn session_schema_contains_metadata_only_and_login_authentication_keeps_working() {
    let (_directory, core) = fixture();
    let state = core.runtime_state();
    let schema = core.native_graphql().unwrap().execute(r#"{ __type(name:"Session") { fields { name } } }"#.into()).await;
    assert!(schema.errors.is_empty(), "{:?}", schema.errors);
    let schema = schema.data.into_json().unwrap();
    let fields: Vec<_> = schema["__type"]["fields"].as_array().unwrap().iter().map(|field| field["name"].as_str().unwrap()).collect();
    assert!(fields.contains(&"clientId"));
    assert!(!fields.contains(&"token"));
    assert!(!fields.contains(&"urlToken"));
    let mut login = state.db.session_get("first").unwrap().unwrap();
    login.r#type = SessionType::Login;
    state.db.session_save(&login).unwrap();
    for origin in [RequestOrigin::Local, RequestOrigin::Remote] {
        let response = execute(&state, "first", origin, "{ sessions { clientId clientName type } }").await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()["sessions"].as_array().unwrap().len(), 2);
    }
    let key = plain_rs::features::sessions::token_key(&login.token).unwrap();
    let document = plain_libs::serde_json::json!({"requestId":plain_rs::features::record_id::generate(),"query":"{ sessions { clientId } app { urlToken } }"}).to_string();
    let request = plain_rs::infrastructure::transport::http_request::encrypt_raw(&key, "POST", "/graphql", document.as_bytes()).unwrap();
    let mut headers = plain_libs::axum::http::HeaderMap::new();
    headers.insert("c-id", "first".parse().unwrap());
    let response = crate::http::graphql::call(plain_libs::axum::extract::State(state.clone()), headers, plain_libs::axum::body::Bytes::from(request.clone())).await;
    assert_eq!(response.status(), plain_libs::axum::http::StatusCode::OK);
    let body = plain_libs::axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let plain = plain_rs::infrastructure::transport::http_response::Context::new(&request).decrypt(&key, &body).unwrap();
    let result: plain_libs::serde_json::Value = plain_libs::serde_json::from_slice(&plain).unwrap();
    assert!(result.get("errors").is_none(), "{result}");
    assert_eq!(result["data"]["sessions"].as_array().unwrap().len(), 2);
    assert_eq!(result["data"]["app"]["urlToken"], login.url_token);
    let saved = state.db.session_get("first").unwrap().unwrap();
    assert_eq!(saved.token, login.token);
    assert_eq!(saved.url_token, login.url_token);
    core.stop_core().await;
}
