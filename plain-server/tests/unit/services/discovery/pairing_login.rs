use super::*;
use crate::services::discovery::pairing_attempts;

#[test]
fn management_proof_binds_identity_access_key_and_freshness() {
    let (private, public) = crypto::ed25519_generate();
    let public = base64_encode(&public);
    let mut response = json!({"clientId":"peer","status":"COMPLETED","ecdhPublicKey":"ephemeral","timestamp":1000,"chatPaired":true});
    response["signature"] = json!(crypto::ed25519_sign(
        &private,
        b"peer|COMPLETED|ephemeral|1000|true"
    ));
    assert_eq!(
        verify_response(&response, "peer", &public, 1000).unwrap(),
        "peer"
    );
    assert!(verify_response(&response, "other", &public, 1000).is_err());
    assert!(verify_response(&response, "peer", &public, 301001).is_err());
    assert!(verify_response(&response, "peer", &base64_encode(&[3; 32]), 1000).is_err());
    response["chatPaired"] = json!(false);
    assert!(verify_response(&response, "peer", &public, 1000).is_err());
    assert!(check_identity("peer", "known", "peer", "replacement").is_err());
}

#[cfg(feature = "http_transport")]
async fn server() -> (
    tempfile::TempDir,
    crate::assembly::server::ContentServer,
    u16,
) {
    let directory = tempfile::tempdir().unwrap();
    let server = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    server
        .runtime_state()
        .prefs
        .set("device_name", "Fixture")
        .unwrap();
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let (_, port) = server
        .start_public(
            0,
            0,
            certificate.cert.pem().into_bytes(),
            certificate.key_pair.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
    (directory, server, port)
}

#[cfg(feature = "http_transport")]
async fn phase(
    state: &ServerState,
    generation: &str,
    expected: &[&str],
) -> pairing_attempts::State {
    let mut events = state.events.subscribe();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let value = state.pairing_runtime.attempts.get(generation).unwrap();
            if expected.contains(&value.phase.as_str()) {
                return value;
            }
            assert!(
                !matches!(value.phase.as_str(), "FAILED" | "CANCELLED"),
                "{}: {:?}",
                value.phase,
                value.error
            );
            events.recv().await.unwrap();
        }
    })
    .await
    .unwrap()
}

#[cfg(feature = "http_transport")]
struct ConfirmationCallbacks {
    host: std::sync::Arc<crate::host::bridge::Host>,
    generation: u64,
    requests: tokio::sync::mpsc::Receiver<Value>,
}

#[cfg(feature = "http_transport")]
impl ConfirmationCallbacks {
    fn new(state: &ServerState) -> Self {
        let (generation, requests) = state.host.connect();
        Self { host: state.host.clone(), generation, requests }
    }
    async fn next(&mut self, method: &str) -> Value {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let request = self.requests.recv().await.unwrap();
                self.host.reply(self.generation, json!({"id":request["id"],"result":null})).unwrap();
                if request["method"] == method { return request["params"].clone(); }
            }
        }).await.unwrap()
    }
}

#[cfg(feature = "http_transport")]
async fn confirmation(events: &mut ConfirmationCallbacks) -> String {
    confirmation_request(events).await["requestId"].as_str().unwrap().into()
}

#[cfg(feature = "http_transport")]
async fn confirmation_request(events: &mut ConfirmationCallbacks) -> Value {
    events.next("systemWebLoginRequested").await
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn passwordless_native_management_displays_the_same_code_and_commits_only_after_approval() {
    let (_local_dir, local, _) = server().await;
    let (_remote_dir, remote, port) = server().await;
    let state = local.runtime_state();
    let remote_state = remote.runtime_state();
    remote_state.prefs.set("password_type", "NONE").unwrap();
    remote_state.prefs.set("login_require_approval", false).unwrap();
    let mut events = ConfirmationCallbacks::new(&remote_state);
    let remote_id = remote_state.native_client_id().unwrap();
    let local_id = state.native_client_id().unwrap();
    let started = pairing_attempts::start(
        &state,
        Some(remote_id.clone()),
        Some(format!("127.0.0.1:{port}")),
        Some(pairing_attempts::Mode::Manage),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let request = confirmation_request(&mut events).await;
    let waiting = phase(&state, &started.generation, &["WAITING_CONFIRMATION"]).await;
    assert_eq!(
        waiting.verification_code.as_deref(),
        request["request"]["verificationCode"].as_str()
    );
    assert_eq!(waiting.verification_code.unwrap().len(), 9);
    assert!(remote_state.db.session_get(&local_id).unwrap().is_none());
    assert!(
        plain_rs::db::chat_store::peers::all(&state.db)
            .unwrap()
            .is_empty()
    );
    remote
        .native_management()
        .unwrap()
        .confirm_web_login(request["requestId"].as_str().unwrap().into(), true)
        .await
        .unwrap();
    phase(&state, &started.generation, &["SUCCEEDED"]).await;
    let peer = plain_rs::db::chat_store::peers::get(&state.db, &remote_id)
        .unwrap()
        .unwrap();
    let paired = plain_rs::db::chat_store::peers::get(&remote_state.db, &local_id)
        .unwrap()
        .unwrap();
    assert_eq!(peer.key, paired.key);
    assert_eq!(
        peer.token,
        remote_state
            .db
            .session_get(&local_id)
            .unwrap()
            .unwrap()
            .token
    );
    assert_eq!(base64_decode(&peer.key).len(), 32);
    assert_ne!(peer.key, peer.token);
    local.shutdown().await;
    remote.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn actual_native_management_uses_encrypted_auth_and_saves_both_grants_once() {
    let (_local_dir, local, _local_port) = server().await;
    let (_remote_dir, remote, remote_port) = server().await;
    let state = local.runtime_state();
    let remote_state = remote.runtime_state();
    remote_state.prefs.set("password_type", "FIXED").unwrap();
    remote_state
        .prefs
        .set("password", "fixture-password")
        .unwrap();
    let mut events = ConfirmationCallbacks::new(&remote_state);
    let http_port = remote_state.public_ports().await.unwrap().0;
    let initial = pairing_attempts::start(
        &state,
        None,
        Some(format!("http://127.0.0.1:{http_port}")),
        Some(pairing_attempts::Mode::Manage),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(initial.device_id.is_none());
    phase(&state, &initial.generation, &["PASSWORD_REQUIRED"]).await;
    pairing_attempts::start(
        &state,
        None,
        None,
        None,
        Some("fixture-password".into()),
        Some(initial.generation.clone()),
        None,
    )
    .await
    .unwrap();
    let request = confirmation(&mut events).await;
    phase(&state, &initial.generation, &["WAITING_CONFIRMATION"]).await;
    crate::http::auth::ws_login::Confirmation::new(remote_state.clone())
        .decide(&request, true)
        .await
        .unwrap();
    let finished = phase(&state, &initial.generation, &["SUCCEEDED"]).await;
    let completed = events.next("systemWebLoginCompleted").await;
    assert!(completed["request"]["password"].is_null());
    assert!(
        !completed
            .to_string()
            .contains(&plain_rs::crypto::sha512_hex("fixture-password"))
    );
    let remote_id = remote_state.native_client_id().unwrap();
    assert_eq!(finished.device_id.as_deref(), Some(remote_id.as_str()));
    let local_id = state.native_client_id().unwrap();
    let peer = plain_rs::db::chat_store::peers::get(&state.db, &remote_id)
        .unwrap()
        .unwrap();
    let other = plain_rs::db::chat_store::peers::get(&remote_state.db, &local_id)
        .unwrap()
        .unwrap();
    assert!(peer.is_paired());
    assert_eq!(peer.port, Some(remote_port));
    assert_eq!(base64_decode(&peer.token).len(), 32);
    assert_eq!(base64_decode(&peer.key).len(), 32);
    assert_eq!(peer.key, other.key);
    assert_ne!(peer.key, peer.token);
    assert_eq!(
        remote_state
            .db
            .session_get(&local_id)
            .unwrap()
            .unwrap()
            .token,
        peer.token
    );
    state.management.shutdown().await;
    remote_state
        .prefs
        .set(plain_rs::prefs::pref_keys::DEVICE_NAME, "Reused name")
        .unwrap();
    let mut local_events = state.events.subscribe();
    let again = pairing_attempts::start(
        &state,
        Some(remote_id.clone()),
        None,
        Some(pairing_attempts::Mode::Manage),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    phase(&state, &again.generation, &["SUCCEEDED"]).await;
    let refreshed = plain_rs::db::chat_store::peers::get(&state.db, &remote_id)
        .unwrap()
        .unwrap();
    assert_eq!(refreshed.name, "Reused name");
    assert_eq!(
        (&refreshed.token, &refreshed.key, &refreshed.public_key),
        (&peer.token, &peer.key, &peer.public_key)
    );
    let mut updated = false;
    while let Ok(event) = local_events.try_recv() {
        if event.event_type == plain_rs::chat::events::WS_PEER_UPDATED {
            updated |= serde_json::from_str::<Value>(&event.payload).unwrap()["id"] == remote_id;
        }
    }
    assert!(updated);

    assert_eq!(
        remote_state
            .db
            .session_get(&local_id)
            .unwrap()
            .unwrap()
            .token,
        peer.token
    );
    assert_eq!(
        plain_rs::db::chat_store::peers::all(&state.db)
            .unwrap()
            .len(),
        1
    );
    local.shutdown().await;
    remote.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn no_password_still_requires_remote_approval_and_cancel_prevents_local_commit() {
    let (_local_dir, local, _) = server().await;
    let (_remote_dir, remote, port) = server().await;
    let state = local.runtime_state();
    let remote_state = remote.runtime_state();
    remote_state.prefs.set("password_type", "NONE").unwrap();
    let mut events = ConfirmationCallbacks::new(&remote_state);
    let first = pairing_attempts::start(
        &state,
        Some(remote_state.native_client_id().unwrap()),
        Some(format!("127.0.0.1:{port}")),
        Some(pairing_attempts::Mode::Manage),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let request = confirmation(&mut events).await;
    phase(&state, &first.generation, &["WAITING_CONFIRMATION"]).await;
    assert!(
        pairing_attempts::cancel(&state, &first.generation)
            .await
            .unwrap()
    );
    assert!(
        !pairing_attempts::cancel(&state, &first.generation)
            .await
            .unwrap()
    );
    let _ = crate::http::auth::ws_login::Confirmation::new(remote_state.clone())
        .decide(&request, true)
        .await;
    assert!(
        plain_rs::db::chat_store::peers::all(&state.db)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        state
            .pairing_runtime
            .attempts
            .get(&first.generation)
            .unwrap()
            .phase,
        "CANCELLED"
    );
    local.shutdown().await;
    remote.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn wrong_password_never_creates_a_local_grant() {
    let (_local_dir, local, _) = server().await;
    let (_remote_dir, remote, port) = server().await;
    let state = local.runtime_state();
    let other = remote.runtime_state();
    other.prefs.set("password_type", "FIXED").unwrap();
    other.prefs.set("password", "fixture-password").unwrap();
    let first = pairing_attempts::start(
        &state,
        None,
        Some(format!("127.0.0.1:{port}")),
        Some(pairing_attempts::Mode::Manage),
        Some("wrong".into()),
        None,
        None,
    )
    .await
    .unwrap();
    let result = phase(&state, &first.generation, &["FAILED"]).await;
    assert_eq!(result.error.as_deref(), Some("invalid_password"));
    assert!(
        plain_rs::db::chat_store::peers::all(&state.db)
            .unwrap()
            .is_empty()
    );
    assert!(
        other
            .db
            .session_get(&state.native_client_id().unwrap())
            .unwrap()
            .is_none()
    );
    local.shutdown().await;
    remote.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn known_key_mismatch_preserves_existing_grants() {
    let (_local_dir, local, _) = server().await;
    let (_remote_dir, remote, port) = server().await;
    let state = local.runtime_state();
    let other = remote.runtime_state();
    let remote_id = other.native_client_id().unwrap();
    state
        .db
        .login_peer(
            &remote_id,
            "Previous",
            "127.0.0.1",
            port,
            plain_rs::chat::enums::DeviceType::Computer,
            "previous-session",
            &base64_encode(&[9; 32]),
            &base64_encode(&[8; 32]),
        )
        .unwrap();
    let first = pairing_attempts::start(
        &state,
        Some(remote_id.clone()),
        Some(format!("127.0.0.1:{port}")),
        Some(pairing_attempts::Mode::Manage),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let failed = phase(&state, &first.generation, &["FAILED"]).await;
    assert_eq!(failed.error.as_deref(), Some("peer_identity_changed"));
    let peer = plain_rs::db::chat_store::peers::get(&state.db, &remote_id)
        .unwrap()
        .unwrap();
    assert_eq!(peer.token, "previous-session");
    assert_eq!(peer.key, base64_encode(&[8; 32]));
    assert!(
        other
            .db
            .session_get(&state.native_client_id().unwrap())
            .unwrap()
            .is_none()
    );
    local.shutdown().await;
    remote.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn remote_rejection_and_native_scope_closure_do_not_save_credentials() {
    let (_local_dir, local, _) = server().await;
    let (_remote_dir, remote, port) = server().await;
    let state = local.runtime_state();
    let other = remote.runtime_state();
    let mut events = ConfirmationCallbacks::new(&other);
    let initial = pairing_attempts::start(
        &state,
        None,
        Some(format!("127.0.0.1:{port}")),
        Some(pairing_attempts::Mode::Manage),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let request = confirmation(&mut events).await;
    crate::http::auth::ws_login::Confirmation::new(other.clone())
        .decide(&request, false)
        .await
        .unwrap();
    let failed = phase(&state, &initial.generation, &["FAILED"]).await;
    assert_eq!(failed.error.as_deref(), Some("rejected"));
    assert!(
        plain_rs::db::chat_store::peers::all(&state.db)
            .unwrap()
            .is_empty()
    );
    other.prefs.set("password_type", "FIXED").unwrap();
    other.prefs.set("password", "fixture-password").unwrap();
    let scope = crate::native::graphql::NativeRequestScope::default();
    let response = local.native_graphql().unwrap().execute_scoped(async_graphql::Request::new(format!("mutation {{ pairDevice(input: {{ host: \"127.0.0.1:{port}\", mode: MANAGE }}) {{ generation }} }}")), scope.clone()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let generation = data["pairDevice"]["generation"].as_str().unwrap();
    phase(&state, generation, &["PASSWORD_REQUIRED"]).await;
    scope.close();
    phase(&state, generation, &["CANCELLED"]).await;
    assert!(
        plain_rs::db::chat_store::peers::all(&state.db)
            .unwrap()
            .is_empty()
    );
    assert!(
        other
            .db
            .session_get(&state.native_client_id().unwrap())
            .unwrap()
            .is_none()
    );
    local.shutdown().await;
    remote.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn a_local_commit_failure_preserves_the_old_row_and_reports_the_remote_grant_honestly() {
    let (_local_dir, local, _) = server().await;
    let (_remote_dir, remote, port) = server().await;
    let state = local.runtime_state();
    let other = remote.runtime_state();
    other.prefs.set("password_type", "FIXED").unwrap();
    other.prefs.set("password", "fixture-password").unwrap();
    let remote_id = other.native_client_id().unwrap();
    let signing = crate::services::peers::peer_wire::signing_keypair(&other.prefs).unwrap();
    state
        .db
        .login_peer(
            &remote_id,
            "Previous",
            "127.0.0.1",
            port,
            plain_rs::chat::enums::DeviceType::Computer,
            "previous-session",
            &base64_encode(&signing[32..]),
            &base64_encode(&[7; 32]),
        )
        .unwrap();
    state.db.with_conn(|conn| conn.execute_batch("CREATE TRIGGER fail_local_pair BEFORE UPDATE ON peers BEGIN SELECT RAISE(ABORT,'fixture local failure'); END;")).unwrap();
    let mut events = ConfirmationCallbacks::new(&other);
    let initial = pairing_attempts::start(
        &state,
        Some(remote_id.clone()),
        Some(format!("127.0.0.1:{port}")),
        Some(pairing_attempts::Mode::Manage),
        Some("fixture-password".into()),
        None,
        None,
    )
    .await
    .unwrap();
    let request = confirmation(&mut events).await;
    crate::http::auth::ws_login::Confirmation::new(other.clone())
        .decide(&request, true)
        .await
        .unwrap();
    let failed = phase(&state, &initial.generation, &["FAILED"]).await;
    assert_eq!(
        failed.error.as_deref(),
        Some("Failed to save paired device")
    );
    let previous = plain_rs::db::chat_store::peers::get(&state.db, &remote_id)
        .unwrap()
        .unwrap();
    assert_eq!(previous.name, "Previous");
    assert_eq!(previous.token, "previous-session");
    assert_eq!(previous.key, base64_encode(&[7; 32]));
    assert!(
        other
            .db
            .session_get(&state.native_client_id().unwrap())
            .unwrap()
            .is_some()
    );
    local.shutdown().await;
    remote.shutdown().await;
}
