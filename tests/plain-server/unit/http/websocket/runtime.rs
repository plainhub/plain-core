use super::*;
use plain_libs::protocols::http_client;
use plain_rs::infrastructure::protocols::websocket as wire;
#[test]
fn websocket_query_has_no_client_selected_login_mode() {
    let params: Params = plain_libs::serde_json::from_value(json!({"cid": "browser"})).unwrap();
    assert_eq!(params.cid, "browser");
    for auth in ["1", "2"] {
        assert!(
            plain_libs::serde_json::from_value::<Params>(json!({"cid": "browser", "auth": auth}))
                .is_err()
        );
    }
}

#[test]
fn upstream_json_keeps_the_control_envelope_registry() {
    let input=controls(br#"{"type":"screenMirrorControl","input":{"action":"SCROLL","x":0.5,"y":0.25,"deltaX":-10,"deltaY":120}}"#);
    assert_eq!(input.len(), 1);
    assert_eq!(input[0]["action"], "SCROLL");
    assert_eq!(input[0]["deltaX"], -10);
    for bytes in [
        br#"{"type":"deviceCommand","action":"BACK"}"#.as_slice(),
        br#"{"action":"BACK"}"#,
        b"not json",
        b"{}",
        b"[1,2,3]",
        br#"{"type":"screenMirrorControl","input":{"action":"INVALID"}}"#,
    ] {
        assert!(controls(bytes).is_empty());
    }
}
#[test]
fn touch_frames_keep_endianness_order_cancel_and_the_reserved_stream_id() {
    let bytes = [
        0x54, 4, 0xff, 0xff, 0, 7, 0x34, 0x12, 0x78, 0x56, 0, 0, 1, 3, 0xff, 0xff, 0xff, 0xff, 0,
        0, 2, 7, 0, 0, 0, 0, 0, 0, 3, 7, 0, 0, 0, 0, 0, 0,
    ];
    let items = controls(&bytes);
    assert_eq!(items.len(), 4);
    assert_eq!(
        items
            .iter()
            .map(|row| row["action"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["TOUCH_DOWN", "TOUCH_MOVE", "TOUCH_UP", "TOUCH_UP"]
    );
    assert_eq!(items[0]["pointerId"], 7);
    assert_eq!(
        items[0]["x"].as_f64().unwrap(),
        (f32::from(0x1234u16) / 65535.0) as f64
    );
    assert_eq!(
        items[0]["y"].as_f64().unwrap(),
        (f32::from(0x5678u16) / 65535.0) as f64
    );
    assert_eq!(items[1]["x"], 1.0);
    for bytes in [
        b"".as_slice(),
        &[0x54],
        &[0x54, 0, 0, 0],
        &[0x53, 1, 0, 0],
        &[0x54, 1, 0, 0, 0],
    ] {
        assert!(controls(bytes).is_empty());
    }
}
#[test]
fn rejecting_confirmation_retains_the_legacy_retry_close_code() {
    let runtime = Runtime::default();
    let (outgoing, _) = mpsc::channel(1);
    let (closing, receiver) = tokio::sync::watch::channel(None);
    runtime.connections.lock().unwrap().insert(
        "connection".into(),
        Connection {
            channel: Channel::Auth,
            client_id: "client".into(),
            registered: false,
            native: false,
            pending: Some("request".into()),
            outgoing: Some(outgoing),
            closing,
        },
    );
    runtime.reject("request");
    assert_eq!(*receiver.borrow(), Some((1013, "rejected")));
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn auth_confirmation_uses_host_callbacks_and_returns_the_hosts_decision_to_the_browser() {
    let _discovery = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    use futures_util::SinkExt;
    use plain_libs::tokio_tungstenite::tungstenite::Message as ClientMessage;
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = std::sync::Arc::new(
        plain_rs::features::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap(),
    );
    prefs.set("background_service_enabled", true).unwrap();
    prefs.set("remote_access", true).unwrap();
    prefs.set("login_require_approval", true).unwrap();
    prefs.set("password_type", "FIXED").unwrap();
    prefs.set("client_id", "phone").unwrap();
    let (private, _) = plain_libs::crypto::ed25519_generate();
    prefs
        .set(
            "signature_key_pair",
            plain_libs::utils::base64::base64_encode(&private),
        )
        .unwrap();
    plain_rs::features::prefs::security::set_entry(
        &prefs,
        "password",
        plain_libs::serde_json::json!("secret"),
    )
    .unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let bridge = std::sync::Arc::new(server.host_events().unwrap());
    let host = bridge.clone();
    let (callbacks, mut incoming) = tokio::sync::mpsc::unbounded_channel();
    let consumer = tokio::spawn(async move {
        while let Some(request) = host.next_request().await {
            if request["method"] == "systemWebLoginRequested" {
                callbacks.send(request["params"].clone()).unwrap();
            }
            host.reply(json!({"id":request["id"],"result":null}))
                .unwrap();
        }
    });
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (port, _) = server.start_public(0, 0, cert_pem, cert_key).await.unwrap();
    for (cid, allowed) in [("rejected-browser", false), ("accepted-browser", true)] {
        let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{port}/auth?cid={cid}"
        ))
        .await
        .unwrap();
        let keys = password_keys(&mut socket, &server.runtime_state(), cid).await;
        let request = json!({

            "appName":"Test", "appVersion":"1", "osName":"Fixture", "osVersion":"1",
            "peer":null,
        });
        let inner = plain_libs::serde_json::to_vec(&request).unwrap();
        let frame = keys
            .encrypt_request(
                &inner,
                plain_libs::chrono::Utc::now().timestamp_millis() as u64,
            )
            .unwrap();
        socket
            .send(ClientMessage::Binary(frame.clone().into()))
            .await
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(response, ClientMessage::Binary(_)));
        let payload = tokio::time::timeout(Duration::from_secs(3), incoming.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(payload["clientId"], cid);
        assert_eq!(payload["request"]["password"], "");
        assert_eq!(payload["request"]["appName"], "Test");
        assert_eq!(payload["request"]["osName"], "Fixture");
        let id = payload["requestId"].as_str().unwrap();
        confirm_login(&server, id, allowed).await.unwrap();
        let response = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if allowed {
            let ClientMessage::Binary(frame) = response else {
                panic!("expected completed auth frame")
            };
            let inner = keys
                .decrypt_response(
                    &frame,
                    plain_libs::chrono::Utc::now().timestamp_millis() as u64,
                )
                .unwrap();
            let decrypted = inner;
            let result: Value = plain_libs::serde_json::from_slice(&decrypted).unwrap();
            assert_eq!(result["status"], "COMPLETED");
            let state = server.runtime_state();
            for _ in 0..3 {
                crate::http::auth::ws_login::admit_attempt(&state, cid, "127.0.0.1").unwrap();
            }
            assert!(crate::http::auth::ws_login::admit_attempt(&state, cid, "127.0.0.1").is_err());
            assert!(
                server
                    .runtime_state()
                    .db
                    .session_get(cid)
                    .unwrap()
                    .is_some()
            );
        } else {
            let ClientMessage::Close(Some(frame)) = response else {
                panic!("expected rejected close frame")
            };
            assert_eq!(u16::from(frame.code), 1013);
            assert_eq!(frame.reason, "rejected");
            assert!(
                server
                    .runtime_state()
                    .db
                    .session_get(cid)
                    .unwrap()
                    .is_none()
            );
        }
        assert!(confirm_login(&server, id, true).await.is_err());
    }
    server.shutdown().await;
    consumer.await.unwrap();
}

fn public_fixture() -> (plain_libs::temporary::TempDir, crate::server::ContentServer) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = std::sync::Arc::new(
        plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    prefs.set("remote_access", true).unwrap();
    let core = crate::server::ContentServer::start(
        directory.path(),
        &plain_libs::utils::base64::base64_encode(&[4; 32]),
        prefs,
    )
    .unwrap();
    let bundle = directory.path().join("web");
    std::fs::create_dir(&bundle).unwrap();
    std::fs::write(
        bundle.join("index.html"),
        "<!doctype html><html><head></head><body>fixture</body></html>",
    )
    .unwrap();
    core.set_web_root(bundle.to_str().unwrap());
    (directory, core)
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn malformed_login_frames_exhaust_the_ip_limit_before_decryption_despite_rotating_clients() {
    use futures_util::SinkExt;
    use plain_libs::tokio_tungstenite::tungstenite::Message as ClientMessage;

    let (_directory, core) = public_fixture();
    let state = core.runtime_state();
    plain_rs::features::prefs::security::set_entry(
        &state.prefs,
        "password",
        plain_libs::serde_json::json!("secret"),
    )
    .unwrap();
    state.prefs.set("password_type", "FIXED").unwrap();
    state.prefs.set("login_require_approval", false).unwrap();
    for attempt in 0..7 {
        let cid = format!("login-limit-{attempt}");
        let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{}/auth?cid={cid}",
            core.port
        ))
        .await
        .unwrap();
        if attempt >= 5 {
            socket
                .send(ClientMessage::Binary(vec![0; 32].into()))
                .await
                .ok();
        } else {
            let keys = password_keys(&mut socket, &state, &cid).await;
            let request = json!({

                "appName":"Test", "appVersion":"1", "osName":"Fixture", "osVersion":"1",
                "peer":null,
            });
            let inner = if attempt == 0 {
                plain_libs::serde_json::to_vec(&request).unwrap()
            } else {
                b"invalid".to_vec()
            };
            let frame = keys
                .encrypt_request(
                    &inner,
                    plain_libs::chrono::Utc::now().timestamp_millis() as u64,
                )
                .unwrap();
            socket
                .send(ClientMessage::Binary(frame.into()))
                .await
                .unwrap();
            let response = tokio::time::timeout(Duration::from_secs(3), socket.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            if attempt == 0 {
                let ClientMessage::Binary(frame) = response else {
                    panic!("expected completed login")
                };
                let inner = keys
                    .decrypt_response(
                        &frame,
                        plain_libs::chrono::Utc::now().timestamp_millis() as u64,
                    )
                    .unwrap();
                let plaintext = inner;
                let result: Value = plain_libs::serde_json::from_slice(&plaintext).unwrap();
                assert_eq!(result["status"], "COMPLETED");
                assert!(result.get("loginContext").is_none());
                assert!(state.db.session_get(&cid).unwrap().is_some());
            } else {
                assert!(
                    matches!(response, ClientMessage::Close(Some(frame)) if frame.reason == "invalid_password")
                );
                assert!(state.db.session_get(&cid).unwrap().is_none());
            }
            socket.close(None).await.ok();
            assert_eq!(state.db.session_list().unwrap().len(), 2);
            continue;
        }
        let response = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let ClientMessage::Close(Some(frame)) = response else {
            panic!("expected rate-limited login")
        };
        assert_eq!(frame.reason, "too_many_login_attempts");
        assert!(state.db.session_get(&cid).unwrap().is_none());
        socket.close(None).await.ok();
        assert_eq!(state.db.session_list().unwrap().len(), 2);
    }
    core.shutdown().await;
}

#[tokio::test]
async fn root_serves_the_spa_and_old_ws_path_never_upgrades() {
    let (_directory, core) = public_fixture();
    let client = http_client::builder().build().unwrap();
    for path in ["/", "/ws?cid=browser"] {
        let response = client
            .get(format!("http://127.0.0.1:{}{path}", core.port))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );
        assert!(response.text().await.unwrap().contains("fixture"));
    }
    for path in ["/?cid=browser", "/ws?cid=browser"] {
        let denied = plain_libs::tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{}{path}",
            core.port
        ))
        .await;
        match denied {
            Err(plain_libs::tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(response.status(), 200)
            }
            _ => panic!("the retired event endpoint must never upgrade"),
        }
    }
    core.shutdown().await;
}

async fn confirm_login(
    server: &crate::server::ContentServer,
    id: &str,
    allowed: bool,
) -> Result<(), String> {
    server
        .native_management()?
        .confirm_web_login(id.to_owned(), allowed)
        .await
        .map(|_| ())
}

#[tokio::test]
async fn sequenced_controls_preserve_order_without_clock_or_database_replay_receipts() {
    use futures_util::SinkExt;
    use plain_libs::tokio_tungstenite::tungstenite::Message as ClientMessage;
    let (_directory, core) = public_fixture();
    let state = core.runtime_state();
    let key = plain_libs::utils::base64::base64_decode(&state.token);
    let client = state.native_client_id().unwrap();
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let host = state.host.clone();
    let (sent, mut received) = tokio::sync::mpsc::channel(8);
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["SCREEN_MIRROR"]),
                "systemScreenMirrorResetTouch" => json!(true),
                "systemScreenMirrorControls" => {
                    sent.send(request["params"]["inputs"].clone())
                        .await
                        .unwrap();
                    json!(true)
                }
                method => panic!("Unexpected platform primitive: {method}"),
            };
            let _ = host.reply(generation, json!({"id":request["id"],"result":result}));
        }
    });
    let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async(format!(
        "ws://127.0.0.1:{}/screen?cid={client}",
        core.port
    ))
    .await
    .unwrap();
    let registration = wire::seal_init(
        &key,
        &client,
        "/screen",
        json!({"clientNonce":"00".repeat(32)})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    let receipts = || {
        state
            .db
            .with_conn(|conn| {
                conn.query_row("SELECT COUNT(*) FROM replay_receipts", [], |row| {
                    row.get::<_, i64>(0)
                })
            })
            .unwrap()
    };
    assert!(
        state
            .db
            .admit_request(
                "http-fixture",
                &client,
                4_000_000_000_000,
                "fixture",
                4_000_000_000_000,
                1
            )
            .unwrap()
    );
    let before = receipts();
    socket
        .send(ClientMessage::Binary(registration.clone().into()))
        .await
        .unwrap();
    let ClientMessage::Binary(ack) = socket.next().await.unwrap().unwrap() else {
        panic!("Expected acknowledgement")
    };
    let ack_plain = wire::open_ack(&key, &client, "/screen", &registration, &ack).unwrap();
    let ack_value: wire::Ack = plain_libs::serde_json::from_slice(&ack_plain).unwrap();
    assert_eq!(ack_value.server_nonce.len(), 64);
    assert!(
        !state
            .main_ws
            .connections
            .lock()
            .unwrap()
            .values()
            .any(|connection| connection.registered)
    );
    let mut stream =
        wire::Stream::new("/screen", &client, &registration, &ack, wire::Role::Client).unwrap();
    let confirmation = stream.encode(b"", &key).unwrap();
    socket
        .send(ClientMessage::Binary(confirmation.clone().into()))
        .await
        .unwrap();
    let json_control = br#"{"type":"screenMirrorControl","input":{"action":"BACK"}}"#;
    let touch = [0x54, 1, 0xff, 0xff, 0, 7, 0x34, 0x12, 0x78, 0x56, 0, 0];
    let mut last = Vec::new();
    for payload in [json_control.as_slice(), touch.as_slice()] {
        last = stream.encode(payload, &key).unwrap();
        socket
            .send(ClientMessage::Binary(last.clone().into()))
            .await
            .unwrap();
    }
    for payload in [json_control.as_slice(), touch.as_slice()] {
        let inputs = tokio::time::timeout(Duration::from_secs(3), received.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(inputs, Value::Array(controls(payload)));
    }
    socket
        .send(ClientMessage::Binary(last.into()))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(message) = socket.next().await {
            if let ClientMessage::Close(Some(close)) = message.unwrap() {
                assert_eq!(u16::from(close.code), 1008);
                assert_eq!(close.reason, "invalid_request");
                return;
            }
        }
        panic!("Replayed control must receive an explicit rejection");
    })
    .await
    .unwrap();
    assert!(received.try_recv().is_err());
    // Replaying the exact init gets a fresh server nonce, so neither old proof can register.
    let old_control = stream.encode(json_control, &key).unwrap();
    for proof in [confirmation, old_control] {
        let (mut next, _) = plain_libs::tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{}/screen?cid={client}",
            core.port
        ))
        .await
        .unwrap();
        next.send(ClientMessage::Binary(registration.clone().into()))
            .await
            .unwrap();
        let ClientMessage::Binary(next_ack) = next.next().await.unwrap().unwrap() else {
            panic!("Expected fresh acknowledgement")
        };
        let next_plain =
            wire::open_ack(&key, &client, "/screen", &registration, &next_ack).unwrap();
        let next_value: wire::Ack = plain_libs::serde_json::from_slice(&next_plain).unwrap();
        assert_ne!(ack_value.server_nonce, next_value.server_nonce);
        assert_ne!(ack, next_ack);
        next.send(ClientMessage::Binary(proof.into()))
            .await
            .unwrap();
        let rejected = tokio::time::timeout(Duration::from_secs(3), next.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(
            matches!(rejected, ClientMessage::Close(Some(frame)) if frame.reason == "invalid_request")
        );
        assert!(received.try_recv().is_err());
    }
    assert_eq!(receipts(), before);
    core.shutdown().await;
    state.host.disconnect(generation);
    tokio::time::timeout(Duration::from_secs(3), responder)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn host_replacement_restores_only_authenticated_online_clients() {
    let _discovery = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let session = state
        .subscription_sessions
        .open(
            state.clone(),
            crate::graphql::executor::RequestContext::authenticated_remote(
                state.clone(),
                "remote".into(),
            ),
            "remote".into(),
            Some(vec![1; 32]),
            plain_rs::infrastructure::host::request_scope::NativeRequestScope::default(),
        )
        .unwrap();
    session.mark_online().unwrap();
    let first = core.host_events().unwrap();
    let initial = host_online(&first, json!(["remote"])).await;
    assert_eq!(initial, json!(["remote"]));
    let replacement = core.host_events().unwrap();
    let restored = host_online(&replacement, json!(["remote"])).await;
    assert_eq!(restored, json!(["remote"]));
    state.subscription_sessions.close(session.id);
    host_online(&replacement, json!([])).await;
    core.stop_core().await;
}

#[tokio::test]
async fn saturated_host_recovers_online_clients_without_counting_native_sessions() {
    let _discovery = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let (_directory, core) = public_fixture();
    let state = core.runtime_state();
    let native = core.native_graphql().unwrap();
    let native_id = native
        .open_subscriptions(
            plain_rs::infrastructure::host::request_scope::NativeRequestScope::default(),
        )
        .unwrap();
    let host = core.host_events().unwrap();
    host_online(&host, json!([])).await;
    let status = host.next_request().await.unwrap();
    assert_eq!(status["method"], "httpServerStatusChanged");
    host.reply(json!({"id":status["id"],"result":null}))
        .unwrap();
    let clipboard = host.next_request().await.unwrap();
    assert_eq!(clipboard["method"], "clipboardHistoryRefresh");
    host.reply(json!({"id":clipboard["id"],"result":null}))
        .unwrap();
    let dlna = host.next_request().await.unwrap();
    assert_eq!(dlna["method"], "dlnaRendererRefresh");
    host.reply(json!({"id":dlna["id"],"result":null})).unwrap();
    let downloads = host.next_request().await.unwrap();
    assert_eq!(downloads["method"], "sharedFolderDownloadsRefresh");
    host.reply(json!({"id":downloads["id"],"result":null}))
        .unwrap();
    let transports = host.next_request().await.unwrap();
    assert_eq!(transports["method"], "peerTransportsChanged");
    host.reply(json!({"id":transports["id"],"result":null}))
        .unwrap();
    let update = host.next_request().await.unwrap();
    assert_eq!(update["method"], "appUpdateDownloadChanged");
    host.reply(json!({"id":update["id"],"result":null}))
        .unwrap();
    let sender = host.next_request().await.unwrap();
    assert_eq!(sender["method"], "dlnaSenderChanged");
    host.reply(json!({"id":sender["id"],"result":null}))
        .unwrap();
    let feeds = host.next_request().await.unwrap();
    assert_eq!(feeds["method"], "feedSyncChanged");
    host.reply(json!({"id":feeds["id"],"result":null})).unwrap();
    let audio = host.next_request().await.unwrap();
    assert_eq!(audio["method"], "audioPlaybackRefresh");
    host.reply(json!({"id":audio["id"],"result":null})).unwrap();
    let library = host.next_request().await.unwrap();
    assert_eq!(library["method"], "audioLibraryRefresh");
    host.reply(json!({"id":library["id"],"result":null}))
        .unwrap();
    let network = host.next_request().await.unwrap();
    assert_eq!(network["method"], "networkInterfacesChanged");
    host.reply(json!({"id":network["id"],"result":null}))
        .unwrap();
    for _ in 0..plain_rs::infrastructure::host::bridge::REQUEST_CAPACITY {
        state.host.notify("blocked", json!({})).unwrap();
    }
    let session = state
        .subscription_sessions
        .open(
            state.clone(),
            crate::graphql::executor::RequestContext::authenticated_remote(
                state.clone(),
                "remote".into(),
            ),
            "remote".into(),
            Some(vec![1; 32]),
            plain_rs::infrastructure::host::request_scope::NativeRequestScope::default(),
        )
        .unwrap();
    session.mark_online().unwrap();
    host_online(&host, json!(["remote"])).await;
    native.close_subscriptions(native_id);
    core.stop_core().await;
}

async fn host_online(host: &crate::host::events::HostEvents, expected: Value) -> Value {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let request = host.next_request().await.unwrap();
            host.reply(json!({"id":request["id"],"result":null}))
                .unwrap();
            if request["method"] == "onlineClientsChanged"
                && request["params"]["clientIds"] == expected
            {
                return request["params"]["clientIds"].clone();
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn online_clients_follow_authenticated_graphql_connections() {
    let _discovery = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    use futures_util::SinkExt;
    use plain_libs::tokio_tungstenite::tungstenite::{Message as Frame, client::IntoClientRequest};

    let (_directory, core) = public_fixture();
    let state = core.runtime_state();
    let client = state.native_client_id().unwrap();
    let key = plain_libs::utils::base64::base64_decode(&state.token);
    let native = core.native_graphql().unwrap();
    let native_id = native
        .open_subscriptions(
            plain_rs::infrastructure::host::request_scope::NativeRequestScope::default(),
        )
        .unwrap();
    let host = core.host_events().unwrap();
    host_online(&host, json!([])).await;
    assert!(state.subscription_sessions.snapshot().is_empty());
    let invalid_request = format!("ws://127.0.0.1:{}/graphql?cid={client}", core.port)
        .into_client_request()
        .unwrap();
    let (mut invalid, _) = plain_libs::tokio_tungstenite::connect_async(invalid_request)
        .await
        .unwrap();
    invalid
        .send(Frame::Binary(vec![0; 64].into()))
        .await
        .unwrap();
    let reply = tokio::time::timeout(Duration::from_secs(3), invalid.next())
        .await
        .unwrap();
    assert!(matches!(reply, Some(Ok(Frame::Close(_)))));
    assert!(state.subscription_sessions.snapshot().is_empty());
    let connect = |nonce: &str| {
        let key = key.clone();
        let client = client.clone();
        let nonce = nonce.to_owned();
        let port = core.port;
        async move {
            let request = format!("ws://127.0.0.1:{port}/graphql?cid={client}")
                .into_client_request()
                .unwrap();
            let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async(request)
                .await
                .unwrap();
            let init = plain_rs::infrastructure::protocols::websocket::seal_init(
                &key,
                &client,
                "/graphql",
                json!({"clientNonce": nonce.repeat(32)})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
            socket
                .send(Frame::Binary(init.clone().into()))
                .await
                .unwrap();
            let Frame::Binary(ack) = socket.next().await.unwrap().unwrap() else {
                panic!("Expected encrypted acknowledgement")
            };
            assert!(
                plain_rs::infrastructure::protocols::websocket::open_ack(
                    &key, &client, "/graphql", &init, &ack
                )
                .is_some()
            );
            let mut stream =
                wire::Stream::new("/graphql", &client, &init, &ack, wire::Role::Client).unwrap();
            socket.send(Frame::Binary(stream.encode(&[], &key).unwrap().into())).await.unwrap();
            socket
                .send(Frame::Binary(
                    stream.encode(br#"{"type":"ping"}"#, &key).unwrap().into(),
                ))
                .await
                .unwrap();
            let Frame::Binary(response) = socket.next().await.unwrap().unwrap() else {
                panic!("Expected encrypted pong")
            };
            let response: Value =
                plain_libs::serde_json::from_slice(&stream.decode(&response, &key).unwrap())
                    .unwrap();
            assert_eq!(response["type"], "pong");
            socket
        }
    };
    let mut first = connect("01").await;
    host_online(&host, json!([client.clone()])).await;
    let mut second = connect("02").await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while state.subscription_sessions.snapshot() != vec![client.clone()] {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    first.close(None).await.unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(3), first.next())
        .await
        .unwrap();
    assert_eq!(state.subscription_sessions.snapshot(), vec![client.clone()]);
    second.close(None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !state.subscription_sessions.snapshot().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    host_online(&host, json!([])).await;
    let mut third = connect("03").await;
    host_online(&host, json!([client.clone()])).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while state.subscription_sessions.snapshot() != vec![client.clone()] {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    state.subscription_sessions.close_client(Some(&client));
    assert!(state.subscription_sessions.snapshot().is_empty());
    let _ = third.close(None).await;
    native.close_subscriptions(native_id);
    core.shutdown().await;
}

#[tokio::test]
async fn remote_pairing_completion_omits_secret_key_and_channel_invites_keep_names() {
    let (_directory, core) = public_fixture();
    let state = core.runtime_state();
    let credentials = core
        .native_management()
        .unwrap()
        .create_session("Pairing subscription test".into())
        .unwrap();
    let client = credentials.client_id;
    let session = state
        .subscription_sessions
        .open(
            state.clone(),
            crate::graphql::executor::RequestContext::authenticated_remote(
                state.clone(),
                client.clone(),
            ),
            client,
            Some(plain_libs::utils::base64::base64_decode(&credentials.token)),
            plain_rs::infrastructure::host::request_scope::NativeRequestScope::default(),
        )
        .unwrap();
    session.send(r#"{"type":"subscribe","id":"public","payload":{"query":"subscription { pairingCompleted { deviceId deviceName ip } }"}}"#).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let response: Value =
                plain_libs::serde_json::from_str(&session.read().await.unwrap()).unwrap();
            if response["payload"]["extensions"]["subscriptionReady"] == true {
                break;
            }
            assert_ne!(response["type"], "error");
        }
    })
    .await
    .unwrap();
    state
        .events
        .send(WsEvent::broadcast(
            "PAIRING_COMPLETED",
            json!({"deviceId":"peer","deviceName":"Peer","ip":"host","key":"secret"}).to_string(),
        ))
        .unwrap();
    let message = tokio::time::timeout(Duration::from_secs(3), session.read())
        .await
        .unwrap()
        .unwrap();
    let response: Value = plain_libs::serde_json::from_str(&message).unwrap();
    assert_eq!(
        response["payload"]["data"]["pairingCompleted"],
        json!({"deviceId":"peer","deviceName":"Peer","ip":"host"})
    );
    assert!(!response.to_string().contains("secret"));
    session.send(r#"{"type":"subscribe","id":"channels","payload":{"query":"subscription { channelsChanged { changed invite { channelId channelName ownerPeerId ownerPeerName } cancel { channelId } } }"}}"#).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let response: Value =
                plain_libs::serde_json::from_str(&session.read().await.unwrap()).unwrap();
            if response["id"] == "channels"
                && response["payload"]["extensions"]["subscriptionReady"] == true
            {
                break;
            }
            assert_ne!(response["type"], "error");
        }
    })
    .await
    .unwrap();
    state.events.send(WsEvent::broadcast("CHANNELS_UPDATED", json!({"changed":true,"invite":{"channelId":"channel","channelName":"Channel","ownerPeerId":"peer","ownerPeerName":"Peer"},"cancel":null}).to_string())).unwrap();
    let message = tokio::time::timeout(Duration::from_secs(3), session.read())
        .await
        .unwrap()
        .unwrap();
    let response: Value = plain_libs::serde_json::from_str(&message).unwrap();
    assert_eq!(
        response["payload"]["data"]["channelsChanged"]["invite"]["channelName"],
        "Channel"
    );
    state.subscription_sessions.close(session.id);
    core.shutdown().await;
}

#[cfg(feature = "http_transport")]
struct PasswordKeys {
    key: [u8; 32],
    context: Vec<u8>,
    ke3: Vec<u8>,
}
#[cfg(feature = "http_transport")]
impl PasswordKeys {
    fn encrypt_request(&self, bytes: &[u8], _now: u64) -> plain_libs::anyhow::Result<Vec<u8>> {
        let mut frame = self.ke3.clone();
        frame.extend(
            plain_libs::crypto::xchacha_encrypt_raw_with_aad(
                &self.key,
                bytes,
                &plain_libs::auth::auth_aad(&self.context, false),
            )
            .unwrap(),
        );
        Ok(frame)
    }
    fn decrypt_response(&self, bytes: &[u8], _now: u64) -> plain_libs::anyhow::Result<Vec<u8>> {
        plain_libs::crypto::xchacha_decrypt_raw_with_aad(
            &self.key,
            bytes,
            &plain_libs::auth::auth_aad(&self.context, true),
        )
        .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid response"))
    }
}
#[cfg(feature = "http_transport")]
async fn password_keys(
    socket: &mut plain_libs::tokio_tungstenite::WebSocketStream<
        plain_libs::tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    state: &ServerState,
    client_id: &str,
) -> PasswordKeys {
    use futures_util::{SinkExt, StreamExt};
    use plain_libs::opaque::{ClientLogin, ClientLoginFinishParameters, CredentialResponse, Identifiers};
    use plain_libs::tokio_tungstenite::tungstenite::Message;
    let signing = plain_rs::features::prefs::security::signing_keypair(&state.prefs).unwrap();
    let context = plain_libs::auth::context(client_id, &signing[32..]);
    let client =
        ClientLogin::start(&mut rand_core::OsRng, b"secret").unwrap();
    socket
        .send(Message::Binary(client.message.serialize().to_vec().into()))
        .await
        .unwrap();
    let Message::Binary(ke2) = socket.next().await.unwrap().unwrap() else {
        panic!("KE2 expected")
    };
    let ksf = plain_libs::auth::ksf();
    let finish = client
        .state
        .finish(
            &mut rand_core::OsRng,
            b"secret",
            CredentialResponse::deserialize(&ke2).unwrap(),
            ClientLoginFinishParameters {
                context: Some(&context),
                identifiers: Identifiers {
                    client: Some(plain_libs::auth::CLIENT_IDENTITY),
                    server: Some(&signing[32..]),
                },
                ksf: Some(&ksf),
            },
        )
        .unwrap();
    PasswordKeys {
        key: plain_libs::auth::api_key(&finish.session_key[..], &context),
        context,
        ke3: finish.message.serialize().to_vec(),
    }
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn socket_registration_rejects_other_route_and_client_binding() {
    use futures_util::{SinkExt, StreamExt};
    use plain_libs::tokio_tungstenite::tungstenite::Message;
    let (_directory, core) = public_fixture();
    let state = core.runtime_state();
    let client = state.native_client_id().unwrap();
    let key = plain_libs::utils::base64::base64_decode(&core.runtime_state().token);
    for (path, cid) in [
        ("/screen", client.as_str()),
        ("/image_editor", "other-client"),
    ] {
        let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{}/image_editor?cid={client}",
            core.port
        ))
        .await
        .unwrap();
        let frame = wire::seal_init(
            &key,
            cid,
            path,
            json!({"clientNonce":"01".repeat(32)})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        socket.send(Message::Binary(frame.into())).await.unwrap();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(3), socket.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            Message::Close(_)
        ));
    }
    core.shutdown().await;
}
