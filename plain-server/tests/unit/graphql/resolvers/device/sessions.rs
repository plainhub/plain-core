use super::*;
use crate::graphql::executor::RequestOrigin;
use crate::server::ContentServer;
use async_graphql::Request;
use plain_rs::sessions::SessionStore;

fn fixture() -> (tempfile::TempDir, ContentServer) {
    let directory = tempfile::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    state.prefs.set("background_service_enabled", true).unwrap();
    state.prefs.set("desktop_access", true).unwrap();
    for id in ["first", "second"] {
        SessionStore::new(&state.db)
            .upsert(SessionInfo {
                client_id: id.into(),
                client_name: id.into(),
                ..Default::default()
            })
            .unwrap();
    }
    (directory, server)
}

async fn execute(
    state: &ServerState,
    caller: &str,
    origin: RequestOrigin,
    query: &str,
) -> async_graphql::Response {
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
        plain_rs::services::sessions::key(&state.db, "second")
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
        plain_rs::services::sessions::key(&state.db, "first")
            .unwrap()
            .is_none()
    );
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("c-id", "first".parse().unwrap());
    headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
    let response = crate::http::graphql::call(
        axum::extract::State(state.clone()),
        headers,
        axum::body::Bytes::from_static(br#"{"query":"{ sessions { clientId } }"}"#),
    )
    .await;
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
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
    server.shutdown().await;
}

#[tokio::test]
async fn only_the_local_owner_can_mint_custom_session_credentials() {
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
        .create_session("Owned custom credential".into())
        .unwrap();
    let id = created.client_id.as_str();
    let token = created.token.as_str();
    assert_eq!(plain_rs::utils::base64::base64_decode(token).len(), 32);
    assert_eq!(state.db.session_get(id).unwrap().unwrap().token, token);
    assert_eq!(state.db.session_list().unwrap().len(), before + 1);
    core.stop_core().await;
}
