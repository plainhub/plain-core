use super::*;
use plain_rs::prefs::Prefs;

#[test]
fn local_request_pages_are_bounded_and_never_expose_request_secrets() {
    let mut attempts = LoginAttempts::default();
    for index in (0..66).rev() {
        attempts.pending.insert(format!("{index:032x}"), Pending {
            client_id: "界".repeat(200), client_ip: "fixture".into(),
            request: json!({"browserName":"界".repeat(200), "password":"fixture-password-secret", "token":"fixture-token-secret", "ecdhPublicKey":"fixture-key-secret", "verificationCode":"forged-code"}),
            at: Instant::now(), protection: password_protection("fixture-digest-secret"),
        });
    }
    attempts
        .pending
        .get_mut(&format!("{:032x}", 65))
        .unwrap()
        .at = Instant::now() - Duration::from_secs(301);
    let first = attempts.pending_requests(None);
    assert_eq!(first.requests.len(), 64);
    assert_eq!(
        first.next_after.as_deref(),
        Some(format!("{:032x}", 63).as_str())
    );
    assert_eq!(first.requests[0].client_id.chars().count(), 128);
    assert_eq!(first.requests[0].browser_name.chars().count(), 128);
    assert!(
        first
            .requests
            .iter()
            .all(|entry| entry.verification_code.is_none())
    );
    let json = serde_json::to_string(&first).unwrap();
    for secret in [
        "fixture-password-secret",
        "fixture-token-secret",
        "fixture-key-secret",
        "fixture-digest-secret",
        "forged-code",
    ] {
        assert!(!json.contains(secret));
    }
    let last = attempts.pending_requests(first.next_after.as_deref());
    assert_eq!(last.requests.len(), 1);
    assert_eq!(last.requests[0].request_id, format!("{:032x}", 64));
    assert!(last.next_after.is_none());
}

#[test]
fn the_login_key_is_a_truncation_of_the_same_hex_the_client_sends() {
    let digest = Sha512::digest(b"secret");
    let hex = plain_rs::utils::hex::bytes_to_hex(&digest);
    let key = hash_to_token(&hex);
    assert_eq!(key.len(), 32);
    // The comparison is against the FULL digest hex (SHA-512 = 128 chars); the
    // key is the first 32 ASCII characters of it. Conflating the two silently
    // rejects every login.
    assert_eq!(key, hex.as_bytes()[..32].to_vec());
    assert_eq!(hex.len(), 128);
    assert_ne!(key, hex.as_bytes()[..128].to_vec());
}

#[test]
fn peer_blocks_are_only_accepted_when_usable() {
    let good = json!({"port": 8080, "signaturePublicKey": plain_rs::utils::base64::base64_encode(&[7u8; 32])});
    assert!(peer_chat_paired(&json!({"peer": good})));
    assert!(!peer_chat_paired(&json!({})));
    assert!(!peer_chat_paired(&json!({"peer": null})));
    assert!(!peer_chat_paired(
        &json!({"peer": {"port": 0, "signaturePublicKey": ""}})
    ));
    assert!(!peer_chat_paired(
        &json!({"peer": {"port": 8080, "signaturePublicKey": "not-a-key"}})
    ));
}

#[test]
fn signature_data_matches_the_client_verification_format() {
    assert_eq!(
        signature_data("device", "COMPLETED", "pk", 42, false),
        "device|COMPLETED|pk|42"
    );
    assert_eq!(
        signature_data("device", "COMPLETED", "pk", 42, true),
        "device|COMPLETED|pk|42|true"
    );
}

#[test]
fn login_attempts_are_rate_limited_per_key() {
    let mut attempts = LoginAttempts::default();
    for _ in 0..LOGIN_ATTEMPT_LIMIT {
        assert!(attempts.acquire("fixture-a"));
    }
    assert!(!attempts.acquire("fixture-a"), "the window must lock out");
    assert!(
        attempts.acquire("fixture-b"),
        "another address is unaffected"
    );
}

#[test]
fn signing_uses_the_core_keypair() {
    let dir = std::env::temp_dir().join(format!(
        "plain-ws-login-sign-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let prefs = Prefs::load(&dir.join("prefs.json")).unwrap();
    let (keypair, public) = plain_rs::crypto::ed25519_generate();
    prefs
        .set(
            "signature_key_pair",
            plain_rs::utils::base64::base64_encode(&keypair),
        )
        .unwrap();

    let signature = sign(&prefs, "device|COMPLETED|pk|42").unwrap();
    assert!(
        !signature.is_empty(),
        "an empty signature looks like a valid answer and fails only on the client"
    );
    assert!(plain_rs::crypto::ed25519_verify(
        &plain_rs::utils::base64::base64_encode(&public),
        b"device|COMPLETED|pk|42",
        &signature
    ));
}

#[test]
fn signing_a_broken_keypair_is_an_error_not_an_empty_signature() {
    let dir = std::env::temp_dir().join(format!(
        "plain-ws-login-broken-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let prefs = Prefs::load(&dir.join("prefs.json")).unwrap();
    prefs.set("signature_key_pair", "AAAA").unwrap();
    assert!(sign(&prefs, "device|COMPLETED|pk|42").is_err());
}

#[cfg(feature = "http_transport")]
type LoginSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[cfg(feature = "http_transport")]
async fn passwordless_server() -> (tempfile::TempDir, crate::server::ContentServer, u16) {
    let directory = tempfile::tempdir().unwrap();
    let server = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    state.prefs.set("password_type", "NONE").unwrap();
    state.prefs.set("login_require_approval", true).unwrap();
    state.prefs.set("password", "unexposed-password").unwrap();
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let (port, _) = server
        .start_public(
            0,
            0,
            cert.cert.pem().into_bytes(),
            cert.key_pair.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
    (directory, server, port)
}

#[cfg(feature = "http_transport")]
async fn login_frame(socket: &mut LoginSocket) -> tokio_tungstenite::tungstenite::Message {
    use futures_util::StreamExt;
    tokio::time::timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

#[cfg(feature = "http_transport")]
async fn browser_login(
    state: &ServerState,
    port: u16,
    client_id: &str,
    password: Option<&str>,
) -> (
    LoginSocket,
    plain_rs::services::confirmation_channel::Channel,
    plain_rs::crypto::EcdhSession,
    Vec<u8>,
) {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::Message;
    let auth = if password.is_some() { "1" } else { "2" };
    let (mut socket, _) = tokio_tungstenite::connect_async(format!(
        "ws://127.0.0.1:{port}/auth?cid={client_id}&auth={auth}"
    ))
    .await
    .unwrap();
    let mode = if password.is_some() {
        plain_rs::services::confirmation_channel::ConfirmationMode::Password
    } else {
        plain_rs::services::confirmation_channel::ConfirmationMode::DeviceConfirmation
    };
    let (client, hello) =
        plain_rs::services::confirmation_channel::Client::start(client_id, mode).unwrap();
    socket
        .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
        .await
        .unwrap();
    let Message::Text(hello) = login_frame(&mut socket).await else {
        panic!("signed hello required")
    };
    let public = plain_rs::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
    let (channel, reveal) = client
        .accept(
            &serde_json::from_str(&hello).unwrap(),
            &plain_rs::utils::base64::base64_encode(&public[32..]),
            &state.native_client_id().unwrap(),
            now_ms(),
        )
        .unwrap();
    socket
        .send(Message::Text(
            serde_json::to_string(&reveal).unwrap().into(),
        ))
        .await
        .unwrap();
    let session = plain_rs::crypto::EcdhSession::generate();
    let mut request = json!({"browserName":"Fixture","browserVersion":"1","osName":"Fixture","osVersion":"1","isMobile":false,"ecdhPublicKey":plain_rs::utils::base64::base64_encode(&session.public_key_bytes),"loginContext":channel.context});
    let digest = password.map(plain_rs::crypto::sha512_hex);
    if let Some(digest) = &digest {
        request["password"] = json!(digest);
    }
    let bytes = serde_json::to_vec(&request).unwrap();
    let bytes = if let Some(digest) = &digest {
        plain_rs::crypto::xchacha_encrypt_raw(&digest.as_bytes()[..32], &bytes).unwrap()
    } else {
        bytes
    };
    let encrypted = channel.encrypt_request(&bytes, now_ms()).unwrap();
    socket
        .send(Message::Binary(encrypted.clone().into()))
        .await
        .unwrap();
    let Message::Binary(pending) = login_frame(&mut socket).await else {
        panic!("encrypted login response required")
    };
    let pending = channel.decrypt_response(&pending, now_ms()).unwrap();
    let pending = if let Some(digest) = &digest {
        plain_rs::crypto::xchacha_decrypt_raw(&digest.as_bytes()[..32], &pending).unwrap()
    } else {
        pending
    };
    let pending: Value = serde_json::from_slice(&pending).unwrap();
    assert_eq!(
        pending["status"],
        if state.prefs.get_or("login_require_approval", true) {
            "PENDING"
        } else {
            "COMPLETED"
        }
    );
    (socket, channel, session, encrypted)
}

#[cfg(feature = "http_transport")]
struct LoginCallbacks {
    host: std::sync::Arc<plain_rs::host::bridge::Host>,
    generation: u64,
    requests: tokio::sync::mpsc::Receiver<Value>,
}

#[cfg(feature = "http_transport")]
impl LoginCallbacks {
    fn new(state: &ServerState) -> Self {
        let (generation, requests) = state.host.connect();
        Self {
            host: state.host.clone(),
            generation,
            requests,
        }
    }

    async fn next(&mut self, method: &str) -> Value {
        tokio::time::timeout(Duration::from_secs(3), async {
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
async fn pending_event(callbacks: &mut LoginCallbacks) -> Value {
    callbacks.next("systemWebLoginRequested").await
}

#[cfg(feature = "http_transport")]
async fn closed_event(callbacks: &mut LoginCallbacks, request_id: &str) {
    let payload = callbacks.next("systemWebLoginClosed").await;
    assert_eq!(payload, json!({"requestId":request_id}));
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn passwordless_hello_consumes_the_same_ip_limiter_before_key_exchange() {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::Message;
    let (_directory, server, port) = passwordless_server().await;
    let (_, hello) = plain_rs::services::confirmation_channel::Client::start(
        "browser",
        plain_rs::services::confirmation_channel::ConfirmationMode::DeviceConfirmation,
    )
    .unwrap();
    for index in 0..=LOGIN_ATTEMPT_LIMIT {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{port}/auth?cid=browser&auth=2"
        ))
        .await
        .unwrap();
        socket
            .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
            .await
            .unwrap();
        let response = login_frame(&mut socket).await;
        if index < LOGIN_ATTEMPT_LIMIT {
            assert!(matches!(response, Message::Text(_)));
        } else {
            assert!(
                matches!(response, Message::Close(Some(close)) if close.reason == "too_many_login_attempts")
            );
        }
        let _ = socket.close(None).await;
    }
    assert!(
        server
            .runtime_state()
            .db
            .session_get("browser")
            .unwrap()
            .is_none()
    );
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn replacing_a_pending_login_closes_the_old_request_before_announcing_the_new_one() {
    let (_directory, server, port) = passwordless_server().await;
    let state = server.runtime_state();
    let mut events = LoginCallbacks::new(&state);
    let (_first, _, _, _) = browser_login(&state, port, "same-browser", None).await;
    let first = pending_event(&mut events).await;
    let first_id = first["requestId"].as_str().unwrap();

    let (_second, _, _, _) = browser_login(&state, port, "same-browser", None).await;
    closed_event(&mut events, first_id).await;
    let second = pending_event(&mut events).await;
    let second_id = second["requestId"].as_str().unwrap();
    assert_ne!(first_id, second_id);
    let pending = server
        .native_management()
        .unwrap()
        .web_login_requests(None)
        .unwrap();
    assert_eq!(pending.requests.len(), 1);
    assert_eq!(pending.requests[0].request_id, second_id);
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn reconnecting_host_receives_pending_login_before_its_close_callback() {
    let (_directory, server, port) = passwordless_server().await;
    let state = server.runtime_state();
    let mut callbacks = LoginCallbacks::new(&state);
    let (_socket, _, _, _) = browser_login(&state, port, "reconnect-browser", None).await;
    let request = pending_event(&mut callbacks).await;
    let request_id = request["requestId"].as_str().unwrap();
    let host = server.host_events().unwrap();
    let snapshot = host.next_request().await.unwrap();
    assert_eq!(snapshot["method"], "systemWebLoginSnapshot");
    assert_eq!(snapshot["params"]["requests"][0]["requestId"], request_id);
    assert_eq!(snapshot["params"]["requests"][0]["request"]["password"], "");
    host.reply(json!({"id":snapshot["id"],"result":null}))
        .unwrap();
    server
        .native_management()
        .unwrap()
        .confirm_web_login(request_id.into(), false)
        .await
        .unwrap();
    loop {
        let next = host.next_request().await.unwrap();
        host.reply(json!({"id":next["id"],"result":null})).unwrap();
        if next["method"] == "systemWebLoginClosed" {
            assert_eq!(next["params"]["requestId"], request_id);
            break;
        }
    }
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn saturated_host_recovers_the_pending_login_snapshot() {
    let (_directory, server, port) = passwordless_server().await;
    let state = server.runtime_state();
    let mut callbacks = LoginCallbacks::new(&state);
    for _ in 0..plain_rs::host::bridge::REQUEST_CAPACITY {
        state.host.notify("blocked", json!({})).unwrap();
    }
    let (_socket, _, _, _) = browser_login(&state, port, "queued-browser", None).await;
    let snapshot = callbacks.next("systemWebLoginSnapshot").await;
    assert_eq!(snapshot["requests"][0]["clientId"], "queued-browser");
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn passwordless_browser_requires_approval_when_enabled_and_only_approval_issues_the_session()
{
    use tokio_tungstenite::tungstenite::Message;
    let (_directory, server, port) = passwordless_server().await;
    let state = server.runtime_state();
    let mut events = LoginCallbacks::new(&state);
    for allowed in [false, true] {
        let client_id = if allowed {
            "approved-browser"
        } else {
            "rejected-browser"
        };
        let (mut socket, channel, session, _) = browser_login(&state, port, client_id, None).await;
        let request = pending_event(&mut events).await;
        assert!(state.db.session_get(client_id).unwrap().is_none());
        assert_eq!(
            request["request"]["verificationCode"],
            channel.verification_code
        );
        assert_eq!(request["request"]["password"], "");
        server
            .native_management()
            .unwrap()
            .confirm_web_login(request["requestId"].as_str().unwrap().into(), allowed)
            .await
            .unwrap();
        closed_event(&mut events, request["requestId"].as_str().unwrap()).await;
        let response = login_frame(&mut socket).await;
        if !allowed {
            assert!(matches!(response, Message::Close(Some(close)) if close.reason == "rejected"));
            assert!(state.db.session_get(client_id).unwrap().is_none());
            continue;
        }
        let Message::Binary(response) = response else {
            panic!("encrypted completion required")
        };
        let response: Value =
            serde_json::from_slice(&channel.decrypt_response(&response, now_ms()).unwrap())
                .unwrap();
        assert_eq!(response["status"], "COMPLETED");
        assert_eq!(response["loginContext"], channel.context);
        assert!(response.get("token").is_none());
        let shared = session
            .compute_shared_key(&plain_rs::utils::base64::base64_decode(
                response["ecdhPublicKey"].as_str().unwrap(),
            ))
            .unwrap();
        assert_eq!(
            state.db.session_get(client_id).unwrap().unwrap().token,
            plain_rs::utils::base64::base64_encode(&shared)
        );
        let signing = plain_rs::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
        let signed = format!(
            "{}|COMPLETED|{}|{}|{}",
            response["clientId"].as_str().unwrap(),
            response["ecdhPublicKey"].as_str().unwrap(),
            response["timestamp"].as_u64().unwrap(),
            channel.context
        );
        assert!(plain_rs::crypto::ed25519_verify(
            &plain_rs::utils::base64::base64_encode(&signing[32..]),
            signed.as_bytes(),
            response["signature"].as_str().unwrap()
        ));
    }
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn disconnect_expiry_mode_changes_and_repeated_requests_cannot_create_a_passwordless_session()
{
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::Message;
    for failure in ["disconnect", "expiry", "mode", "replay"] {
        let (_directory, server, port) = passwordless_server().await;
        let state = server.runtime_state();
        let mut events = LoginCallbacks::new(&state);
        let (mut socket, _, _, frame) = browser_login(&state, port, "browser", None).await;
        let event = pending_event(&mut events).await;
        let id = event["requestId"].as_str().unwrap();
        match failure {
            "disconnect" => {
                socket.close(None).await.unwrap();
                tokio::time::timeout(Duration::from_secs(3), async {
                    while state.main_ws.login_pending(id) {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
            }
            "expiry" => {
                state
                    .login_attempts
                    .lock()
                    .unwrap()
                    .pending
                    .get_mut(id)
                    .unwrap()
                    .at = Instant::now() - Duration::from_secs(301)
            }
            "mode" => {
                state.prefs.set("password_type", "FIXED").unwrap();
            }
            "replay" => {
                socket.send(Message::Binary(frame.into())).await.unwrap();
                assert!(matches!(login_frame(&mut socket).await, Message::Close(_)));
            }
            _ => unreachable!(),
        }
        assert!(
            server
                .native_management()
                .unwrap()
                .confirm_web_login(id.into(), true)
                .await
                .is_err(),
            "{failure}"
        );
        assert!(
            state.db.session_get("browser").unwrap().is_none(),
            "{failure}"
        );
        closed_event(&mut events, id).await;
        server.shutdown().await;
    }
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn successful_passwordless_logins_still_count_toward_the_rolling_ip_limit() {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::Message;
    let (_directory, server, port) = passwordless_server().await;
    let state = server.runtime_state();
    state.prefs.set("login_require_approval", false).unwrap();
    for index in 0..LOGIN_ATTEMPT_LIMIT {
        let client_id = format!("successful-browser-{index}");
        let (mut socket, _, _, _) = browser_login(&state, port, &client_id, None).await;
        assert!(state.db.session_get(&client_id).unwrap().is_some());
        assert!(state.login_attempts.lock().unwrap().pending.is_empty());
        socket.close(None).await.unwrap();
    }
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/auth?cid=blocked&auth=2"))
            .await
            .unwrap();
    let (_, hello) = plain_rs::services::confirmation_channel::Client::start(
        "blocked",
        plain_rs::services::confirmation_channel::ConfirmationMode::DeviceConfirmation,
    )
    .unwrap();
    socket
        .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
        .await
        .unwrap();
    assert!(
        matches!(login_frame(&mut socket).await, Message::Close(Some(close)) if close.reason == "too_many_login_attempts")
    );
    assert!(state.db.session_get("blocked").unwrap().is_none());
    server.shutdown().await;
}

#[test]
fn attempt_history_expires_and_never_exceeds_the_tracked_address_bound() {
    let mut attempts = LoginAttempts::default();
    for index in 0..MAX_TRACKED_CLIENTS {
        assert!(attempts.acquire(&format!("address-{index}")));
    }
    assert!(!attempts.acquire("overflow"));
    assert_eq!(attempts.entries.len(), MAX_TRACKED_CLIENTS);
    for times in attempts.entries.values_mut() {
        for at in times {
            *at -= LOGIN_ATTEMPT_WINDOW;
        }
    }
    assert!(attempts.acquire("new-address"));
    assert_eq!(attempts.entries.len(), 1);
    for _ in 1..LOGIN_ATTEMPT_LIMIT {
        assert!(attempts.acquire("new-address"));
    }
    assert!(!attempts.acquire("new-address"));
}

#[test]
fn recovery_requests_follow_creation_order_independent_of_hash_iteration() {
    let mut attempts = LoginAttempts::default();
    let now = Instant::now();
    for id in ["first", "second", "third"] {
        attempts.pending.insert(
            id.into(),
            Pending {
                client_id: id.into(),
                client_ip: String::new(),
                request: json!({"browserName":id}),
                at: now,
                protection: password_protection(""),
            },
        );
    }
    let mut ids: Vec<_> = attempts.pending.keys().cloned().collect();
    for (index, id) in ids.iter().enumerate() {
        attempts.pending.get_mut(id).unwrap().at = now - Duration::from_secs(index as u64);
    }
    ids.reverse();
    let snapshot = attempts.host_snapshot();
    let actual: Vec<_> = snapshot["requests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["requestId"].as_str().unwrap())
        .collect();
    assert_eq!(actual, ids);
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn committed_login_rejects_missing_changed_and_repeated_reveals_before_any_grant() {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::Message;
    for variant in [
        "before-commitment",
        "missing-reveal",
        "changed-reveal",
        "repeated-reveal",
    ] {
        let (_directory, server, port) = passwordless_server().await;
        let state = server.runtime_state();
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{port}/auth?cid=browser&auth=2"
        ))
        .await
        .unwrap();
        if variant == "before-commitment" {
            socket
                .send(Message::Binary(vec![0; 40].into()))
                .await
                .unwrap();
        } else {
            let (client, hello) = plain_rs::services::confirmation_channel::Client::start(
                "browser",
                plain_rs::services::confirmation_channel::ConfirmationMode::DeviceConfirmation,
            )
            .unwrap();
            socket
                .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
                .await
                .unwrap();
            let Message::Text(response) = login_frame(&mut socket).await else {
                panic!("signed challenge required")
            };
            let keypair =
                plain_rs::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
            let (channel, mut reveal) = client
                .accept(
                    &serde_json::from_str(&response).unwrap(),
                    &plain_rs::utils::base64::base64_encode(&keypair[32..]),
                    "",
                    now_ms(),
                )
                .unwrap();
            match variant {
                "missing-reveal" => socket
                    .send(Message::Binary(
                        channel.encrypt_request(b"{}", now_ms()).unwrap().into(),
                    ))
                    .await
                    .unwrap(),
                "changed-reveal" => {
                    reveal.nonce = plain_rs::utils::base64::base64_encode(&[9; 32]);
                    socket
                        .send(Message::Text(
                            serde_json::to_string(&reveal).unwrap().into(),
                        ))
                        .await
                        .unwrap();
                }
                "repeated-reveal" => {
                    let frame = serde_json::to_string(&reveal).unwrap();
                    socket
                        .send(Message::Text(frame.clone().into()))
                        .await
                        .unwrap();
                    socket.send(Message::Text(frame.into())).await.unwrap();
                }
                _ => unreachable!(),
            }
        }
        assert!(
            matches!(login_frame(&mut socket).await, Message::Close(Some(close)) if close.reason == "invalid_login_request"),
            "{variant}"
        );
        assert!(state.db.session_get("browser").unwrap().is_none());
        assert!(state.login_attempts.lock().unwrap().pending.is_empty());
        server.shutdown().await;
    }
}

fn password_protection(digest: &str) -> LoginProtection {
    use plain_rs::services::confirmation_channel::{Client, ConfirmationMode, ServerHandshake};
    let (private, public) = plain_rs::crypto::ed25519_generate();
    let (client, hello) = Client::start("browser", ConfirmationMode::Password).unwrap();
    let (server, response) = ServerHandshake::start(
        "browser",
        ConfirmationMode::Password,
        "device",
        &hello,
        &private,
        now_ms(),
    )
    .unwrap();
    let (_, reveal) = client
        .accept(
            &response,
            &plain_rs::utils::base64::base64_encode(&public),
            "device",
            now_ms(),
        )
        .unwrap();
    LoginProtection::Password {
        digest: digest.into(),
        channel: server.finish(&reveal, now_ms()).unwrap(),
    }
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn password_login_rejects_frames_without_a_key_exchange_and_without_outer_encryption() {
    use futures_util::SinkExt;
    use plain_rs::services::confirmation_channel::{Client, ConfirmationMode};
    use tokio_tungstenite::tungstenite::Message;
    let (_directory, server, port) = passwordless_server().await;
    let state = server.runtime_state();
    state.prefs.set("password_type", "FIXED").unwrap();
    let digest = password_digest(&state.prefs);
    for handshake in [false, true] {
        let client_id = if handshake {
            "inner-only"
        } else {
            "no-exchange"
        };
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{port}/auth?cid={client_id}&auth=1",
        ))
        .await
        .unwrap();
        let mut context = String::new();
        if handshake {
            let (client, hello) = Client::start(client_id, ConfirmationMode::Password).unwrap();
            socket
                .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
                .await
                .unwrap();
            let Message::Text(hello) = login_frame(&mut socket).await else {
                panic!("signed hello required")
            };
            let signing =
                plain_rs::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
            let (keys, reveal) = client
                .accept(
                    &serde_json::from_str(&hello).unwrap(),
                    &plain_rs::utils::base64::base64_encode(&signing[32..]),
                    &state.native_client_id().unwrap(),
                    now_ms(),
                )
                .unwrap();
            context = keys.context;
            socket
                .send(Message::Text(
                    serde_json::to_string(&reveal).unwrap().into(),
                ))
                .await
                .unwrap();
        }
        let session = plain_rs::crypto::EcdhSession::generate();
        let request = json!({
            "password":digest, "loginContext":context,
            "browserName":"Fixture", "browserVersion":"1", "osName":"Fixture", "osVersion":"1",
            "isMobile":false, "ecdhPublicKey":plain_rs::utils::base64::base64_encode(&session.public_key_bytes),
        });
        let inner = plain_rs::crypto::xchacha_encrypt_raw(
            &digest.as_bytes()[..32],
            &serde_json::to_vec(&request).unwrap(),
        )
        .unwrap();
        socket.send(Message::Binary(inner.into())).await.unwrap();
        assert!(matches!(login_frame(&mut socket).await, Message::Close(_)));
        assert!(state.db.session_get(client_id).unwrap().is_none());
    }
    assert!(state.db.session_list().unwrap().is_empty());
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn password_change_mode_change_expiry_disconnect_and_replay_prevent_late_approval() {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::Message;
    for failure in ["password", "mode", "expiry", "disconnect", "replay"] {
        let (_directory, server, port) = passwordless_server().await;
        let state = server.runtime_state();
        state.prefs.set("password_type", "FIXED").unwrap();
        let mut events = LoginCallbacks::new(&state);
        let (mut socket, _, _, frame) =
            browser_login(&state, port, "browser", Some("unexposed-password")).await;
        let event = pending_event(&mut events).await;
        assert!(event["request"].get("verificationCode").is_none());
        assert_eq!(event["request"]["password"], "");
        let id = event["requestId"].as_str().unwrap();
        match failure {
            "password" => {
                state.prefs.set("password", "changed-password").unwrap();
            }
            "mode" => {
                state.prefs.set("password_type", "NONE").unwrap();
            }
            "expiry" => {
                state
                    .login_attempts
                    .lock()
                    .unwrap()
                    .pending
                    .get_mut(id)
                    .unwrap()
                    .at = Instant::now() - Duration::from_secs(301);
            }
            "disconnect" => {
                socket.close(None).await.unwrap();
                tokio::time::timeout(Duration::from_secs(3), async {
                    while state.main_ws.login_pending(id) {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
            }
            "replay" => {
                socket.send(Message::Binary(frame.into())).await.unwrap();
                assert!(matches!(login_frame(&mut socket).await, Message::Close(_)));
            }
            _ => unreachable!(),
        }
        assert!(
            server
                .native_management()
                .unwrap()
                .confirm_web_login(id.into(), true)
                .await
                .is_err()
        );
        assert!(state.db.session_get("browser").unwrap().is_none());
        server.shutdown().await;
    }
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn idle_connections_consume_the_login_budget_before_any_message() {
    use tokio_tungstenite::tungstenite::Message;
    let (_directory, server, port) = passwordless_server().await;
    let mut sockets = Vec::new();
    for index in 0..LOGIN_ATTEMPT_LIMIT {
        let (socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{port}/auth?cid=idle-{index}&auth=2"
        ))
        .await
        .unwrap();
        sockets.push(socket);
    }
    let rejected = tokio_tungstenite::connect_async(format!(
        "ws://127.0.0.1:{port}/auth?cid=over-budget&auth=2"
    ))
    .await;
    match rejected {
        Ok((mut socket, _)) => {
            assert!(
                matches!(login_frame(&mut socket).await, Message::Close(Some(frame)) if frame.reason == "too_many_login_attempts")
            );
        }
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            assert_eq!(response.status().as_u16(), 429);
        }
        Err(error) => panic!("unexpected connection failure: {error}"),
    }
    let state = server.runtime_state();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let count = state
                .login_attempts
                .lock()
                .unwrap()
                .entries
                .get("127.0.0.1")
                .map(Vec::len)
                .unwrap_or(0);
            if count == LOGIN_ATTEMPT_LIMIT {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(server.runtime_state().db.session_list().unwrap().is_empty());
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn a_signed_hello_does_not_extend_the_initial_exchange_deadline() {
    use futures_util::{SinkExt, StreamExt};
    use plain_rs::services::confirmation_channel::{Client, ConfirmationMode};
    use tokio_tungstenite::tungstenite::Message;
    let (_directory, server, port) = passwordless_server().await;
    let (mut socket, _) = tokio_tungstenite::connect_async(format!(
        "ws://127.0.0.1:{port}/auth?cid=incomplete&auth=2",
    ))
    .await
    .unwrap();
    let (_, hello) = Client::start("incomplete", ConfirmationMode::DeviceConfirmation).unwrap();
    socket
        .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
        .await
        .unwrap();
    assert!(matches!(login_frame(&mut socket).await, Message::Text(_)));
    let response = tokio::time::timeout(Duration::from_secs(12), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(response, Message::Close(Some(frame)) if frame.reason == "timeout"));
    assert!(server.runtime_state().db.session_list().unwrap().is_empty());
    server.shutdown().await;
}
