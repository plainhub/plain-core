use futures_util::SinkExt;
use futures_util::StreamExt;
use plain_libs::utils::base64::base64_decode;
use plain_libs::utils::base64::base64_encode;
use plain_rs::features::services::discovery::pairing_attempts;
use plain_rs::features::services::discovery::pairing_attempts::PairingPhase;
use plain_rs::runtime::Runtime as ServerState;
use plain_libs::serde_json::Value;
use plain_libs::serde_json::json;
use sha2::Digest;
use std::time::Duration;

#[cfg(feature = "http_transport")]
struct ConfirmationCallbacks {
    host: std::sync::Arc<plain_rs::infrastructure::host::bridge::Host>,
    generation: u64,
    requests: tokio::sync::mpsc::Receiver<Value>,
}

#[cfg(feature = "http_transport")]
impl ConfirmationCallbacks {
    fn new(state: &ServerState) -> Self {
        let (generation, requests) = state.host.connect();
        Self {
            host: state.host.clone(),
            generation,
            requests,
        }
    }
    async fn next(&mut self, method: &str) -> Value {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let request = self.requests.recv().await.unwrap();
                self.host
                    .reply(self.generation, json!({"id":request["id"],"result":null}))
                    .unwrap();
                if request["method"] == method {
                    return request["params"].clone();
                }
            }
        })
        .await
        .unwrap()
    }
}

#[cfg(feature = "http_transport")]
async fn server() -> (
    plain_libs::temporary::TempDir,
    crate::server::ContentServer,
    u16,
) {
    server_with_policy(crate::server::PairingApproval::Required).await
}
#[cfg(feature = "http_transport")]
async fn server_with_policy(
    policy: crate::server::PairingApproval,
) -> (
    plain_libs::temporary::TempDir,
    crate::server::ContentServer,
    u16,
) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open_with_pairing_approval(
        &directory.path().join("data"),
        &directory.path().join("cache"),
        policy,
    )
    .unwrap();
    server
        .runtime_state()
        .prefs
        .set("device_name", "Fixture")
        .unwrap();
    let (certificate_pem, certificate_key) =
        plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, port) = server
        .start_public(0, 0, certificate_pem, certificate_key)
        .await
        .unwrap();
    (directory, server, port)
}

#[cfg(feature = "http_transport")]
async fn phase(
    state: &ServerState,
    generation: &str,
    expected: &[PairingPhase],
) -> pairing_attempts::State {
    let mut events = state.events.subscribe();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let value = state.pairing_runtime.attempts.get(generation).unwrap();
            if expected.contains(&value.phase) {
                return value;
            }
            assert!(
                !matches!(value.phase, PairingPhase::Failed | PairingPhase::Cancelled),
                "{:?}: {:?}",
                value.phase,
                value.error
            );
            events.recv().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| panic!("Expected {expected:?}; current {}",plain_libs::serde_json::to_string(&state.pairing_runtime.attempts.get(generation)).unwrap()))
}

#[cfg(feature = "http_transport")]
async fn trust_identity(state: &ServerState, generation: &str) {
    let waiting = phase(state, generation, &[PairingPhase::TrustRequired]).await;
    assert_eq!(waiting.verification_code.as_ref().unwrap().len(), 64);
    plain_rs::features::services::discovery::pairing_confirmation::confirm(state, generation)
        .await
        .unwrap();
}

#[cfg(feature = "http_transport")]
async fn confirmation(events: &mut ConfirmationCallbacks) -> String {
    confirmation_request(events).await["requestId"]
        .as_str()
        .unwrap()
        .into()
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
    remote_state
        .prefs
        .set("login_require_approval", false)
        .unwrap();
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
    trust_identity(&state, &started.generation).await;
    let request = confirmation_request(&mut events).await;
    let client = state.host.startup();
    assert!(!client.os_name.is_empty());
    assert!(!client.os_version.is_empty());
    let os_name = if cfg!(target_os = "macos") {
        "macOS"
    } else {
        client.os_name.as_str()
    };
    assert_eq!(request["request"]["osName"], os_name);
    assert_eq!(request["request"]["osVersion"], client.os_version);
    assert_eq!(request["request"]["appVersion"], client.app_version);

    let waiting = phase(
        &state,
        &started.generation,
        &[PairingPhase::WaitingConfirmation],
    )
    .await;
    assert_eq!(
        waiting.verification_code.as_deref(),
        request["request"]["verificationCode"].as_str()
    );
    assert_eq!(waiting.verification_code.unwrap().len(), 64);
    assert!(remote_state.db.session_get(&local_id).unwrap().is_none());
    assert!(
        plain_rs::infrastructure::db::chat_store::peers::all(&state.db)
            .unwrap()
            .is_empty()
    );
    remote
        .native_management()
        .unwrap()
        .confirm_web_login(request["requestId"].as_str().unwrap().into(), true)
        .await
        .unwrap();
    phase(&state, &started.generation, &[PairingPhase::Succeeded]).await;
    let peer = plain_rs::infrastructure::db::chat_store::peers::get(&state.db, &remote_id)
        .unwrap()
        .unwrap();
    let paired = plain_rs::infrastructure::db::chat_store::peers::get(&remote_state.db, &local_id)
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
    let session = remote_state.db.session_get(&local_id).unwrap().unwrap();
    assert_eq!(session.os_name, os_name);
    assert_eq!(session.os_version, client.os_version);
    assert_eq!(session.app_name, "PlainApp");
    assert_eq!(session.app_version, client.app_version);
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
    plain_rs::features::prefs::security::set_password_login(&remote_state.prefs,"fixture-password")
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
    phase(
        &state,
        &initial.generation,
        &[PairingPhase::PasswordRequired],
    )
    .await;
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
    phase(
        &state,
        &initial.generation,
        &[PairingPhase::WaitingConfirmation],
    )
    .await;
    crate::http::auth::ws_login::Confirmation::new(remote_state.clone())
        .decide(&request, true)
        .await
        .unwrap();
    let finished = phase(&state, &initial.generation, &[PairingPhase::Succeeded]).await;
    let completed = events.next("systemWebLoginCompleted").await;
    assert!(completed["request"]["password"].is_null());
    assert!(
        !completed
            .to_string()
            .contains(&plain_libs::crypto::sha512_hex("fixture-password"))
    );
    let remote_id = remote_state.native_client_id().unwrap();
    assert_eq!(finished.device_id.as_deref(), Some(remote_id.as_str()));
    let local_id = state.native_client_id().unwrap();
    let peer = plain_rs::infrastructure::db::chat_store::peers::get(&state.db, &remote_id)
        .unwrap()
        .unwrap();
    let other = plain_rs::infrastructure::db::chat_store::peers::get(&remote_state.db, &local_id)
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
        .set(
            plain_rs::features::prefs::pref_keys::DEVICE_NAME,
            "Reused name",
        )
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
    phase(&state, &again.generation, &[PairingPhase::Succeeded]).await;
    let refreshed = plain_rs::infrastructure::db::chat_store::peers::get(&state.db, &remote_id)
        .unwrap()
        .unwrap();
    assert_eq!(refreshed.name, "Reused name");
    assert_eq!(
        (&refreshed.token, &refreshed.key, &refreshed.public_key),
        (&peer.token, &peer.key, &peer.public_key)
    );
    let mut updated = false;
    while let Ok(event) = local_events.try_recv() {
        if event.event_type == plain_rs::features::chat::events::WS_PEER_UPDATED {
            updated |= plain_libs::serde_json::from_str::<Value>(&event.payload).unwrap()["id"] == remote_id;
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
        plain_rs::infrastructure::db::chat_store::peers::all(&state.db)
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
    trust_identity(&state, &first.generation).await;
    let request = confirmation(&mut events).await;
    phase(
        &state,
        &first.generation,
        &[PairingPhase::WaitingConfirmation],
    )
    .await;
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
        plain_rs::infrastructure::db::chat_store::peers::all(&state.db)
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
        PairingPhase::Cancelled
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
    plain_rs::features::prefs::security::set_password_login(&other.prefs,"fixture-password")
    .unwrap();
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
    let result = phase(&state, &first.generation, &[PairingPhase::Failed]).await;
    assert_eq!(result.error.as_deref(), Some("invalid_password"));
    assert!(
        plain_rs::infrastructure::db::chat_store::peers::all(&state.db)
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
            plain_rs::features::chat::enums::DeviceType::Computer,
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
    let waiting = phase(&state, &first.generation, &[PairingPhase::TrustRequired]).await;
    assert!(waiting.identity_changed);
    assert_eq!(waiting.verification_code.as_ref().unwrap().len(), 64);
    pairing_attempts::cancel(&state, &first.generation)
        .await
        .unwrap();
    let peer = plain_rs::infrastructure::db::chat_store::peers::get(&state.db, &remote_id)
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
    trust_identity(&state, &initial.generation).await;
    let request = confirmation(&mut events).await;
    crate::http::auth::ws_login::Confirmation::new(other.clone())
        .decide(&request, false)
        .await
        .unwrap();
    let failed = phase(&state, &initial.generation, &[PairingPhase::Failed]).await;
    assert_eq!(failed.error.as_deref(), Some("rejected"));
    assert!(
        plain_rs::infrastructure::db::chat_store::peers::all(&state.db)
            .unwrap()
            .is_empty()
    );
    plain_rs::features::prefs::security::set_password_login(&other.prefs,"fixture-password")
    .unwrap();
    let scope = plain_rs::infrastructure::host::request_scope::NativeRequestScope::default();
    let response = local.native_graphql().unwrap().execute_scoped(plain_graphql::Request::new(format!("mutation {{ pairDevice(input: {{ host: \"127.0.0.1:{port}\", mode: MANAGE }}) {{ generation }} }}")), scope.clone()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let generation = data["pairDevice"]["generation"].as_str().unwrap();
    phase(&state, generation, &[PairingPhase::PasswordRequired]).await;
    scope.close();
    phase(&state, generation, &[PairingPhase::Cancelled]).await;
    assert!(
        plain_rs::infrastructure::db::chat_store::peers::all(&state.db)
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
    plain_rs::features::prefs::security::set_password_login(&other.prefs,"fixture-password")
    .unwrap();
    let remote_id = other.native_client_id().unwrap();
    let signing =
        plain_rs::features::services::peers::peer_wire::signing_keypair(&other.prefs).unwrap();
    state
        .db
        .login_peer(
            &remote_id,
            "Previous",
            "127.0.0.1",
            port,
            plain_rs::features::chat::enums::DeviceType::Computer,
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
    let failed = phase(&state, &initial.generation, &[PairingPhase::Failed]).await;
    assert_eq!(
        failed.error.as_deref(),
        Some("Failed to save paired device")
    );
    let previous = plain_rs::infrastructure::db::chat_store::peers::get(&state.db, &remote_id)
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

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn web_management_pairs_by_device_id_and_keeps_the_grant_on_a() {
    use plain_libs::protocols::http_client;
    use plain_rs::{
        features::{
            chat::enums::DeviceType,
            services::{peers::management_http, sessions},
        },
        infrastructure::db::{
            DPeer,
            chat_store::{SaveMode, peers},
        },
    };

    let (_a_dir, a, a_port) = server().await;
    let (_b_dir, b, b_port) = server().await;
    let a_state = a.runtime_state();
    let b_state = b.runtime_state();
    let a_id = a_state.native_client_id().unwrap();
    let b_id = b_state.native_client_id().unwrap();
    let mut events = ConfirmationCallbacks::new(&b_state);
    let peer = DPeer::new(
        &b_id,
        "Fixture B",
        "127.0.0.1",
        Some(b_port),
        DeviceType::Computer,
    );
    peers::save(&a_state.db, &[peer], SaveMode::Insert).unwrap();
    let mut browser = sessions::create(&a_state, "fixture-browser".into()).unwrap();
    browser.client_id = "fixture-browser".into();
    browser.r#type = plain_rs::features::sessions::SessionType::Login;
    a_state.db.session_save(&browser).unwrap();
    let http = http_client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let query = |input: Value| {
        let client = http.clone();
        let key = base64_decode(&browser.token);
        let id = browser.client_id.clone();
        async move {
            let body = json!({
                "query":"mutation($input: PairingDeviceInput!) { pairDevice(input: $input) { generation } }",
                "variables":{"input":input},
            });
            let response = management_http::exchange(
                client
                    .post(format!("https://localhost:{a_port}/graphql"))
                    .header("c-id", id),
                &key,
                &body.to_string(),
                8192,
            )
            .await
            .unwrap();
            plain_libs::serde_json::from_slice::<Value>(&response).unwrap()
        }
    };
    let denied =
        query(json!({"id":b_id,"host":format!("127.0.0.1:{b_port}"),"mode":"MANAGE"})).await;
    assert_eq!(denied["errors"][0]["message"], "no_permission");
    let started = query(json!({"id":b_id,"mode":"MANAGE"})).await;
    let generation = started["data"]["pairDevice"]["generation"]
        .as_str()
        .expect("pairing generation");
    phase(&a_state, generation, &[PairingPhase::TrustRequired]).await;
    let trusted = query(json!({"generation":generation,"confirm":true})).await;
    assert!(trusted["errors"].is_null(), "{trusted}");
    let request = confirmation(&mut events).await;
    b.native_management()
        .unwrap()
        .confirm_web_login(request, true)
        .await
        .unwrap();
    phase(&a_state, generation, &[PairingPhase::Succeeded]).await;
    let peer = peers::get(&a_state.db, &b_id).unwrap().unwrap();
    let grant = b_state.db.session_get(&a_id).unwrap().unwrap();
    assert!(!peer.token.is_empty());
    assert_eq!(peer.token, grant.token);
    assert!(
        a_state
            .db
            .session_get(&browser.client_id)
            .unwrap()
            .is_some()
    );
    assert!(
        b_state
            .db
            .session_get(&browser.client_id)
            .unwrap()
            .is_none()
    );
    a.shutdown().await;
    b.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn configured_password_can_complete_without_manual_approval_when_policy_allows() {
    let (_local_dir, local, _) = server().await;
    let (_remote_dir, remote, port) =
        server_with_policy(crate::server::PairingApproval::FollowPasswordLogin).await;
    let state = local.runtime_state();
    let other = remote.runtime_state();
    plain_rs::features::prefs::security::set_password_login(&other.prefs, "fixture-password")
        .unwrap();
    other.prefs.set("login_require_approval", false).unwrap();
    let attempt = pairing_attempts::start(
        &state,
        None,
        Some(format!("127.0.0.1:{port}")),
        Some(pairing_attempts::Mode::Manage),
        Some("fixture-password".into()),
        None,
        None,
    )
    .await
    .unwrap();
    phase(&state, &attempt.generation, &[PairingPhase::Succeeded]).await;
    let peer = plain_rs::infrastructure::db::chat_store::peers::get(
        &state.db,
        &other.native_client_id().unwrap(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        peer.token,
        other
            .db
            .session_get(&state.native_client_id().unwrap())
            .unwrap()
            .unwrap()
            .token
    );
    assert_eq!(base64_decode(&peer.token).len(), 32);
    local.shutdown().await;
    remote.shutdown().await;
}
