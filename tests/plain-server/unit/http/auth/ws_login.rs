use super::*;

#[test]
fn local_request_pages_are_bounded_and_never_expose_request_secrets() {
    let mut attempts = LoginAttempts::default();
    for index in (0..66).rev() {
        attempts.pending.insert(format!("{index:032x}"), Pending {
            client_id: "界".repeat(200), client_ip: "fixture".into(),
            request: json!({"appName":"界".repeat(200), "password":"fixture-password-secret", "token":"fixture-token-secret", "ecdhPublicKey":"fixture-key-secret", "verificationCode":"forged-code"}),
            at: Instant::now(), protection: password_protection(None),
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
    assert_eq!(first.requests[0].app_name.chars().count(), 128);
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
fn peer_blocks_are_only_accepted_when_usable() {
    let good = json!({"port": 8080, "signaturePublicKey": plain_libs::utils::base64::base64_encode(&[7u8; 32])});
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

#[cfg(feature = "http_transport")]
type LoginSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[cfg(feature = "http_transport")]
async fn passwordless_server() -> (
    plain_libs::temporary::TempDir,
    crate::server::ContentServer,
    u16,
) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    state.prefs.set("password_type", "NONE").unwrap();
    state.prefs.set("login_require_approval", true).unwrap();
    plain_rs::features::prefs::security::set_entry(
        &state.prefs,
        "password",
        json!("unexposed-password"),
    )
    .unwrap();
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (port, _) = server.start_public(0, 0, cert_pem, cert_key).await.unwrap();
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

struct TestChannel {
    inner: LoginChannel,
    verification_code: String,
}
impl TestChannel {
    fn encrypt_request(&self, bytes: &[u8], now: u64) -> plain_libs::anyhow::Result<Vec<u8>> {
        match &self.inner {
            LoginChannel::Confirmation(c) => c.encrypt_request(bytes, now),
            LoginChannel::Opaque(c) => Ok(plain_libs::crypto::xchacha_encrypt_raw_with_aad(
                &c.key,
                bytes,
                &plain_libs::auth::auth_aad(&c.context, false),
            )
            .unwrap()),
        }
    }
    fn decrypt_response(&self, bytes: &[u8], now: u64) -> plain_libs::anyhow::Result<Vec<u8>> {
        match &self.inner {
            LoginChannel::Confirmation(c) => c.decrypt_response(bytes, now),
            LoginChannel::Opaque(c) => plain_libs::crypto::xchacha_decrypt_raw_with_aad(
                &c.key,
                bytes,
                &plain_libs::auth::auth_aad(&c.context, true),
            )
            .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid response")),
        }
    }
}
#[cfg(feature = "http_transport")]
async fn browser_login(
    state: &ServerState,
    port: u16,
    client_id: &str,
    password: Option<&str>,
) -> (
    LoginSocket,
    TestChannel,
    plain_libs::crypto::EcdhSession,
    Vec<u8>,
) {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::Message;
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/auth?cid={client_id}"))
            .await
            .unwrap();
    let (channel, prefix) = if let Some(password) = password {
        use opaque_ke::{
            ClientLogin, ClientLoginFinishParameters, CredentialResponse, Identifiers,
        };
        let signing = plain_rs::features::prefs::security::signing_keypair(&state.prefs).unwrap();
        let identity = &signing[32..];
        let context = plain_libs::auth::context(client_id, identity);
        let client =
            ClientLogin::<plain_libs::auth::Suite>::start(&mut rand_core::OsRng, password.as_bytes())
                .unwrap();
        socket
            .send(Message::Binary(client.message.serialize().to_vec().into()))
            .await
            .unwrap();
        let Message::Binary(ke2) = login_frame(&mut socket).await else {
            panic!("KE2 required")
        };
        let ksf = plain_libs::auth::ksf();
        let finish = client
            .state
            .finish(
                &mut rand_core::OsRng,
                password.as_bytes(),
                CredentialResponse::deserialize(&ke2).unwrap(),
                ClientLoginFinishParameters {
                    context: Some(&context),
                    identifiers: Identifiers {
                        client: Some(plain_libs::auth::CLIENT_IDENTITY),
                        server: Some(identity),
                    },
                    ksf: Some(&ksf),
                },
            )
            .unwrap();
        let key = plain_libs::auth::api_key(&finish.session_key, &context);
        let channel = LoginChannel::Opaque(OpaqueChannel {
            key,
            context,
            credential: plain_rs::features::prefs::security::password_credential(&state.prefs)
                .unwrap()
                .unwrap(),
            created: Instant::now(),
        });
        (
            TestChannel {
                verification_code: String::new(),
                inner: channel,
            },
            finish.message.serialize().to_vec(),
        )
    } else {
        let (client, hello) =
            plain_rs::features::services::confirmation_channel::Client::start(client_id).unwrap();
        socket
            .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
            .await
            .unwrap();
        let Message::Text(hello) = login_frame(&mut socket).await else {
            panic!("signed hello required")
        };
        let public =
            plain_rs::features::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
        let channel = client
            .accept(
                &serde_json::from_str(&hello).unwrap(),
                &plain_libs::utils::base64::base64_encode(&public[32..]),
                &state.native_client_id().unwrap(),
                now_ms(),
            )
            .unwrap();

        (
            TestChannel {
                verification_code: channel.verification_code.clone(),
                inner: LoginChannel::Confirmation(channel),
            },
            Vec::new(),
        )
    };
    let session = plain_libs::crypto::EcdhSession::generate();
    let request = json!({"appName":"Fixture","appVersion":"1","osName":"mac","osVersion":"27.2.0","isMobile":false});
    let bytes = serde_json::to_vec(&request).unwrap();
    let mut encrypted = prefix;
    encrypted.extend(channel.encrypt_request(&bytes, now_ms()).unwrap());
    socket
        .send(Message::Binary(encrypted.clone().into()))
        .await
        .unwrap();
    let Message::Binary(pending) = login_frame(&mut socket).await else {
        panic!("encrypted login response required")
    };
    let pending = channel.decrypt_response(&pending, now_ms()).unwrap();
    let pending: Value = serde_json::from_slice(&pending).unwrap();
    assert_eq!(
        pending["status"],
        if password.is_none() || state.prefs.get_or("login_require_approval", true) {
            "PENDING"
        } else {
            "COMPLETED"
        }
    );
    (socket, channel, session, encrypted)
}

#[cfg(feature = "http_transport")]
struct LoginCallbacks {
    host: std::sync::Arc<plain_rs::infrastructure::host::bridge::Host>,
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
    let (_, hello) =
        plain_rs::features::services::confirmation_channel::Client::start("browser").unwrap();
    for index in 0..=LOGIN_ATTEMPT_LIMIT {
        let (mut socket, _) =
            tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/auth?cid=browser"))
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
    let _discovery = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
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
    for _ in 0..plain_rs::infrastructure::host::bridge::REQUEST_CAPACITY {
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
        let (mut socket, channel, _session, _) = browser_login(&state, port, client_id, None).await;
        let request = pending_event(&mut events).await;
        assert!(state.db.session_get(client_id).unwrap().is_none());
        assert_eq!(
            request["request"]["verificationCode"],
            channel.verification_code
        );
        assert_eq!(request["request"]["password"], "");
        assert_eq!(request["request"]["appName"], "Fixture");
        assert_eq!(request["request"]["appVersion"], "1");
        assert_eq!(request["request"]["osName"], "macOS");
        assert_eq!(request["request"]["osVersion"], "27.2.0");
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
        assert!(response.get("loginContext").is_none());
        assert!(response.get("token").is_none());
        let LoginChannel::Confirmation(c) = &channel.inner else {
            unreachable!()
        };
        let shared = c.api_key();
        let saved = state.db.session_get(client_id).unwrap().unwrap();
        assert_eq!(
            saved.token,
            plain_libs::utils::base64::base64_encode(&shared)
        );
        assert_eq!(
            saved.r#type,
            plain_rs::features::sessions::SessionType::Login
        );
        assert_eq!(saved.app_name, "Fixture");
        assert_eq!(saved.app_version, "1");
        assert_eq!(saved.os_name, "macOS");
        assert_eq!(saved.os_version, "27.2.0");
    }
    let records = state.db.audit_events(0, 10, None).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records.iter().any(|row| row.r#type == "login" && row.client_id.as_deref() == Some("approved-browser")));
    assert!(records.iter().any(|row| row.r#type == "login_failed" && row.message == "rejected" && row.client_id.as_deref() == Some("rejected-browser")));
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
        assert!(
            state.db.session_get(&client_id).unwrap().is_none(),
            "passwordless requires local approval even with the preference disabled"
        );
        let id = state
            .login_attempts
            .lock()
            .unwrap()
            .pending
            .iter()
            .find(|(_, p)| p.client_id == client_id)
            .unwrap()
            .0
            .clone();
        server
            .native_management()
            .unwrap()
            .confirm_web_login(id, true)
            .await
            .unwrap();
        assert!(matches!(login_frame(&mut socket).await, Message::Binary(_)));
        assert!(state.db.session_get(&client_id).unwrap().is_some());
        assert!(state.login_attempts.lock().unwrap().pending.is_empty());
        socket.close(None).await.unwrap();
    }
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/auth?cid=blocked"))
            .await
            .unwrap();
    let (_, hello) =
        plain_rs::features::services::confirmation_channel::Client::start("blocked").unwrap();
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
                request: json!({"appName":id}),
                at: now,
                protection: password_protection(None),
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
async fn signed_login_rejects_missing_and_repeated_hellos_before_any_grant() {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::Message;
    for repeat in [false, true] {
        let (_directory, server, port) = passwordless_server().await;
        let state = server.runtime_state();
        let (mut socket, _) =
            tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/auth?cid=browser"))
                .await
                .unwrap();
        if repeat {
            let (_, hello) =
                plain_rs::features::services::confirmation_channel::Client::start("browser")
                    .unwrap();
            let text = serde_json::to_string(&hello).unwrap();
            socket
                .send(Message::Text(text.clone().into()))
                .await
                .unwrap();
            assert!(matches!(login_frame(&mut socket).await, Message::Text(_)));
            socket.send(Message::Text(text.into())).await.unwrap();
        } else {
            socket
                .send(Message::Binary(vec![0; 40].into()))
                .await
                .unwrap();
        }
        assert!(matches!(login_frame(&mut socket).await, Message::Close(_)));
        assert_eq!(
            state
                .db
                .session_list()
                .unwrap()
                .iter()
                .map(|row| row.client_id.clone())
                .collect::<Vec<_>>(),
            vec![state.native_client_id().unwrap()]
        );
        assert!(state.login_attempts.lock().unwrap().pending.is_empty());
        server.shutdown().await;
    }
}

fn password_protection(
    credential: Option<plain_rs::features::prefs::security::PasswordCredential>,
) -> LoginProtection {
    let credential =
        credential.unwrap_or(plain_rs::features::prefs::security::PasswordCredential {
            setup: String::new(),
            record: String::new(),
        });
    LoginProtection::Password {
        credential: credential.clone(),
        channel: LoginChannel::Opaque(OpaqueChannel {
            key: [9; 32],
            context: b"fixture".to_vec(),
            credential,
            created: Instant::now(),
        }),
    }
}

#[tokio::test]
async fn password_login_requires_setup_and_the_configured_password() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let security = &state.prefs;
    plain_rs::features::prefs::security::configure_password_login(security).unwrap();
    let pending = password_protection(None);
    let request = json!({"loginContext": pending.channel().context()});
    assert_eq!(
        pending.validate(&state, &request).unwrap_err().to_string(),
        "password_setup_required"
    );
    plain_rs::features::prefs::security::set_password_login(security, "owned-password").unwrap();
    let protection = password_protection(
        plain_rs::features::prefs::security::password_credential(security).unwrap(),
    );
    let request = json!({"loginContext": protection.channel().context()});
    protection.validate(&state, &request).unwrap();
    assert_eq!(
        pending.validate(&state, &request).unwrap_err().to_string(),
        "invalid_password"
    );
    plain_rs::features::prefs::security::set_password_login(security, "replacement-password")
        .unwrap();
    assert_eq!(
        protection
            .validate(&state, &request)
            .unwrap_err()
            .to_string(),
        "invalid_password"
    );
    server.stop_core().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn server_configuration_selects_the_handshake_mode_without_client_input() {
    use futures_util::SinkExt;
    use plain_rs::features::services::confirmation_channel::Client;
    use tokio_tungstenite::tungstenite::Message;
    for configured in ["FIXED", "NONE"] {
        let (_directory, server, port) = passwordless_server().await;
        let state = server.runtime_state();
        state.prefs.set("password_type", configured).unwrap();
        let (mut socket, _) =
            tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/auth?cid=wrong-mode"))
                .await
                .unwrap();
        if configured == "FIXED" {
            let (_, hello) = Client::start("wrong-mode").unwrap();
            socket
                .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
                .await
                .unwrap();
        } else {
            let client = opaque_ke::ClientLogin::<plain_libs::auth::Suite>::start(
                &mut rand_core::OsRng,
                b"password",
            )
            .unwrap();
            socket
                .send(Message::Binary(client.message.serialize().to_vec().into()))
                .await
                .unwrap();
        }
        assert!(matches!(login_frame(&mut socket).await, Message::Close(_)));
        assert!(state.db.session_get("wrong-mode").unwrap().is_none());
        socket.close(None).await.ok();
        server.shutdown().await;
    }
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn first_password_login_completes_without_owner_approval() {
    let (directory, server, port) = passwordless_server().await;
    let state = server.runtime_state();
    plain_rs::features::prefs::security::configure_password_login(&state.prefs).unwrap();
    plain_rs::features::prefs::security::set_password_login(&state.prefs, "owned-password")
        .unwrap();
    let (mut socket, channel, _, _) =
        browser_login(&state, port, "first-browser", Some("owned-password")).await;
    let LoginChannel::Opaque(c) = &channel.inner else {
        panic!("OPAQUE expected")
    };
    let key = c.key;
    let saved = state.db.session_get("first-browser").unwrap().unwrap();
    assert_eq!(saved.token, plain_libs::utils::base64::base64_encode(&key));
    let records = state.db.audit_events(0, 10, None).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].r#type, "login");
    assert_eq!(records[0].client_id.as_deref(), Some("first-browser"));
    assert_eq!(records[0].message, "Fixture");
    assert!(!records[0].message.contains("owned-password"));
    assert!(!records[0].message.contains(&saved.token));
    socket.close(None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if !state.main_ws.has_client_connection("first-browser") {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        state
            .db
            .session_get("first-browser")
            .unwrap()
            .unwrap()
            .token,
        saved.token
    );
    async fn check(state: ServerState, key: &[u8; 32]) {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("c-id", "first-browser".parse().unwrap());
        let payload = format!(
            "{}|{}|{}",
            now_ms(),
            plain_libs::utils::uuid::Uuid::new_v4(),
            json!({"query":r#"{noteCount(query:"")}"#})
        );
        for (method, path) in [("POST", "/guest_graphql"), ("GET", "/graphql")] {
            let wrong = plain_rs::infrastructure::transport::http_request::encrypt_raw(
                key,
                method,
                path,
                payload.as_bytes(),
            )
            .unwrap();
            let response = crate::http::graphql::call(
                axum::extract::State(state.clone()),
                headers.clone(),
                axum::body::Bytes::from(wrong),
            )
            .await;
            assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        }
        let frame = plain_rs::infrastructure::transport::http_request::encrypt_raw(
            key,
            "POST",
            "/graphql",
            payload.as_bytes(),
        )
        .unwrap();
        let context =
            plain_rs::infrastructure::transport::http_response::Context::new("/graphql", &frame);
        let response = crate::http::graphql::call(
            axum::extract::State(state),
            headers,
            axum::body::Bytes::from(frame),
        )
        .await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let body = context.decrypt(key, &body).unwrap();
        assert!(serde_json::from_slice::<Value>(&body).unwrap()["errors"].is_null());
    }
    check(state.clone(), &key).await;
    assert!(state.login_attempts.lock().unwrap().pending.is_empty());
    server.shutdown().await;
    let reopened = crate::server::ContentServer::open(directory.path()).unwrap();
    assert_eq!(
        reopened
            .runtime_state()
            .db
            .session_get("first-browser")
            .unwrap()
            .unwrap()
            .token,
        saved.token
    );
    check(reopened.runtime_state(), &key).await;
    reopened.stop_core().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn password_login_rejects_frames_without_a_key_exchange_and_without_outer_encryption() {
    use futures_util::SinkExt;
    use plain_rs::features::services::confirmation_channel::Client;
    use tokio_tungstenite::tungstenite::Message;
    let (_directory, server, port) = passwordless_server().await;
    let state = server.runtime_state();
    state.prefs.set("password_type", "FIXED").unwrap();
    let digest = plain_libs::crypto::sha512_hex("unexposed-password");
    for handshake in [false, true] {
        let client_id = if handshake {
            "inner-only"
        } else {
            "no-exchange"
        };
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{port}/auth?cid={client_id}",
        ))
        .await
        .unwrap();
        let context = String::new();
        if handshake {
            let (_, hello) = Client::start(client_id).unwrap();
            socket
                .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
                .await
                .unwrap();
            assert!(matches!(login_frame(&mut socket).await, Message::Close(_)));
            continue;
        }
        let session = plain_libs::crypto::EcdhSession::generate();
        let request = json!({
            "password":digest, "loginContext":context,
            "appName":"Fixture", "appVersion":"1", "osName":"Fixture", "osVersion":"1",
            "isMobile":false, "ecdhPublicKey":plain_libs::utils::base64::base64_encode(&session.public_key_bytes),
        });
        let inner = plain_libs::crypto::xchacha_encrypt_raw(
            &digest.as_bytes()[..32],
            &serde_json::to_vec(&request).unwrap(),
        )
        .unwrap();
        socket.send(Message::Binary(inner.into())).await.unwrap();
        assert!(matches!(login_frame(&mut socket).await, Message::Close(_)));
        assert!(state.db.session_get(client_id).unwrap().is_none());
    }
    assert_eq!(
        state
            .db
            .session_list()
            .unwrap()
            .iter()
            .map(|row| row.client_id.clone())
            .collect::<Vec<_>>(),
        vec![state.native_client_id().unwrap()]
    );
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
                plain_rs::features::prefs::security::set_entry(
                    &state.prefs,
                    "password",
                    json!("changed-password"),
                )
                .unwrap();
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
            "ws://127.0.0.1:{port}/auth?cid=idle-{index}"
        ))
        .await
        .unwrap();
        sockets.push(socket);
    }
    let rejected =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/auth?cid=over-budget"))
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
    assert_eq!(
        server
            .runtime_state()
            .db
            .session_list()
            .unwrap()
            .iter()
            .map(|row| row.client_id.clone())
            .collect::<Vec<_>>(),
        vec![server.runtime_state().native_client_id().unwrap()]
    );
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn a_signed_hello_does_not_extend_the_initial_exchange_deadline() {
    use futures_util::{SinkExt, StreamExt};
    use plain_rs::features::services::confirmation_channel::Client;
    use tokio_tungstenite::tungstenite::Message;
    let (_directory, server, port) = passwordless_server().await;
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/auth?cid=incomplete",))
            .await
            .unwrap();
    let (_, hello) = Client::start("incomplete").unwrap();
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
    assert_eq!(
        server
            .runtime_state()
            .db
            .session_list()
            .unwrap()
            .iter()
            .map(|row| row.client_id.clone())
            .collect::<Vec<_>>(),
        vec![server.runtime_state().native_client_id().unwrap()]
    );
    server.shutdown().await;
}

#[tokio::test]
async fn password_login_verifies_the_salted_record_and_rejects_reusable_stored_values() {
    use plain_rs::features::prefs::security;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    security::set_password_login(&state.prefs, "owned-password").unwrap();
    let stored = security::password_credential(&state.prefs)
        .unwrap()
        .unwrap();
    for secret in [
        "owned-password".to_owned(),
        plain_libs::crypto::sha512_hex("wrong-password"),
        serde_json::to_string(&stored).unwrap(),
    ] {
        let protection = password_protection(Some(stored.clone()));
        let LoginProtection::Password { channel, .. } = protection else {
            unreachable!()
        };
        let session = plain_libs::crypto::EcdhSession::generate();
        let request = json!({"password":secret,"loginContext":channel.context(),"appName":"Fixture","appVersion":"1","osName":"Fixture","osVersion":"1","isMobile":false,"ecdhPublicKey":plain_libs::utils::base64::base64_encode(&session.public_key_bytes)});
        let LoginChannel::Opaque(c) = &channel else {
            unreachable!()
        };
        let frame = plain_libs::crypto::xchacha_encrypt_raw_with_aad(
            &c.key,
            &serde_json::to_vec(&request).unwrap(),
            &plain_libs::auth::auth_aad(&c.context, false),
        )
        .unwrap();
        let result = execute(
            &state,
            Request::Issue {
                client_id: "rejected-browser".into(),
                client_ip: "fixture".into(),
                frame: plain_libs::utils::base64::base64_encode(&frame),
                channel,
            },
        )
        .await;
        assert_eq!(result.unwrap_err().to_string(), "invalid_login_request");
        assert!(state.db.session_get("rejected-browser").unwrap().is_none());
        assert!(state.login_attempts.lock().unwrap().pending.is_empty());
    }
    server.stop_core().await;
}

#[tokio::test]
async fn relogin_rotates_the_session_url_token_and_keeps_the_session_identity() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let request = Pending {
        client_id: "browser".into(),
        client_ip: "fixture".into(),
        request: json!({"appName":"Browser"}),
        at: Instant::now(),
        protection: password_protection(None),
    };
    persist(&state, &request, "first-token").unwrap();
    let first = state.db.session_get("browser").unwrap().unwrap();
    persist(&state, &request, "second-token").unwrap();
    let second = state.db.session_get("browser").unwrap().unwrap();
    assert_ne!(first.url_token, second.url_token);
    assert_eq!(first.client_id, second.client_id);
    assert_eq!(first.created_at, second.created_at);
    assert_eq!(second.token, "second-token");
}

#[path = "login_events.rs"]
mod event_tests;

#[tokio::test]
async fn web_login_cannot_replace_the_native_transport_session() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let id = state.native_client_id().unwrap();
    let stored = state.db.session_get(&id).unwrap().unwrap();
    let request = Pending {
        client_id: id.clone(),
        client_ip: "fixture".into(),
        request: json!({"appName":"Browser"}),
        at: Instant::now(),
        protection: password_protection(None),
    };
    assert_eq!(
        persist(&state, &request, "untrusted-token")
            .unwrap_err()
            .to_string(),
        "invalid_login_request"
    );
    let current = state.db.session_get(&id).unwrap().unwrap();
    assert_eq!(current.token, stored.token);
    assert_eq!(current.url_token, stored.url_token);
    server.shutdown().await;
}
