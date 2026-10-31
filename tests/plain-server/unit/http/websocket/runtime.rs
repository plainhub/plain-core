use super::*;
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
            pending: Some("request".into()),
            outgoing,
            closing,
        },
    );
    runtime.reject("request");
    assert_eq!(*receiver.borrow(), Some((1013, "rejected")));
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn auth_confirmation_uses_host_callbacks_and_returns_the_hosts_decision_to_the_browser() {
    use futures_util::SinkExt;
    use sha2::Digest;
    use tokio_tungstenite::tungstenite::Message as ClientMessage;
    let dir = tempfile::tempdir().unwrap();
    let prefs =
        std::sync::Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("background_service_enabled", true).unwrap();
    prefs.set("desktop_access", true).unwrap();
    prefs.set("login_require_approval", true).unwrap();
    prefs.set("password", "secret").unwrap();
    prefs.set("password_type", "FIXED").unwrap();
    prefs.set("client_id", "phone").unwrap();
    let (private, _) = plain_rs::crypto::ed25519_generate();
    prefs
        .set(
            "signature_key_pair",
            plain_rs::utils::base64::base64_encode(&private),
        )
        .unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
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
    let digest = plain_rs::utils::hex::bytes_to_hex(&sha2::Sha512::digest(b"secret"));
    let client = plain_rs::crypto::EcdhSession::generate();
    for (cid, allowed) in [("rejected-browser", false), ("accepted-browser", true)] {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{port}/auth?cid={cid}&auth=1"
        ))
        .await
        .unwrap();
        let keys = password_keys(&mut socket, &server.runtime_state(), cid).await;
        let request = json!({
            "password":digest, "loginContext":keys.context,
            "browserName":"Test", "browserVersion":"1", "osName":"Fixture", "osVersion":"1",
            "isMobile":false, "ecdhPublicKey":plain_rs::utils::base64::base64_encode(&client.public_key_bytes),
            "peer":null,
        });
        let inner = plain_rs::crypto::xchacha_encrypt_raw(
            &digest.as_bytes()[..32],
            &serde_json::to_vec(&request).unwrap(),
        )
        .unwrap();
        let frame = keys
            .encrypt_request(&inner, chrono::Utc::now().timestamp_millis() as u64)
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
        assert_eq!(payload["request"]["browserName"], "Test");
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
                .decrypt_response(&frame, chrono::Utc::now().timestamp_millis() as u64)
                .unwrap();
            let decrypted =
                plain_rs::crypto::xchacha_decrypt_raw(&digest.as_bytes()[..32], &inner).unwrap();
            let result: Value = serde_json::from_slice(&decrypted).unwrap();
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

fn public_fixture() -> (tempfile::TempDir, crate::server::ContentServer) {
    let directory = tempfile::tempdir().unwrap();
    let prefs = std::sync::Arc::new(
        plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    prefs.set("desktop_access", true).unwrap();
    let core = crate::server::ContentServer::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[4; 32]),
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
    use sha2::Digest;
    use tokio_tungstenite::tungstenite::Message as ClientMessage;

    let (_directory, core) = public_fixture();
    let state = core.runtime_state();
    state.prefs.set("password", "secret").unwrap();
    state.prefs.set("password_type", "FIXED").unwrap();
    state.prefs.set("login_require_approval", false).unwrap();
    let digest = plain_rs::utils::hex::bytes_to_hex(&sha2::Sha512::digest(b"secret"));
    let client = plain_rs::crypto::EcdhSession::generate();
    for attempt in 0..7 {
        let cid = format!("login-limit-{attempt}");
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{}/auth?cid={cid}&auth=1",
            core.port
        ))
        .await
        .unwrap();
        if attempt >= 5 {
            use plain_rs::services::confirmation_channel::{Client, ConfirmationMode};
            let (_, hello) = Client::start(&cid, ConfirmationMode::Password).unwrap();
            socket
                .send(ClientMessage::Text(
                    serde_json::to_string(&hello).unwrap().into(),
                ))
                .await
                .unwrap();
        } else {
            let keys = password_keys(&mut socket, &state, &cid).await;
            let request = json!({
                "password":digest, "loginContext":keys.context,
                "browserName":"Test", "browserVersion":"1", "osName":"Fixture", "osVersion":"1",
                "isMobile":false, "ecdhPublicKey":plain_rs::utils::base64::base64_encode(&client.public_key_bytes),
                "peer":null,
            });
            let inner = if attempt == 0 {
                plain_rs::crypto::xchacha_encrypt_raw(
                    &digest.as_bytes()[..32],
                    &serde_json::to_vec(&request).unwrap(),
                )
                .unwrap()
            } else {
                b"invalid".to_vec()
            };
            let frame = keys
                .encrypt_request(&inner, chrono::Utc::now().timestamp_millis() as u64)
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
                    .decrypt_response(&frame, chrono::Utc::now().timestamp_millis() as u64)
                    .unwrap();
                let plaintext =
                    plain_rs::crypto::xchacha_decrypt_raw(&digest.as_bytes()[..32], &inner)
                        .unwrap();
                let result: Value = serde_json::from_slice(&plaintext).unwrap();
                assert_eq!(result["status"], "COMPLETED");
                assert_eq!(result["loginContext"], keys.context);
                assert!(state.db.session_get(&cid).unwrap().is_some());
            } else {
                assert!(
                    matches!(response, ClientMessage::Close(Some(frame)) if frame.reason == "invalid_password")
                );
                assert!(state.db.session_get(&cid).unwrap().is_none());
            }
            socket.close(None).await.ok();
            assert_eq!(state.db.session_list().unwrap().len(), 1);
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
        assert_eq!(state.db.session_list().unwrap().len(), 1);
    }
    core.shutdown().await;
}

#[tokio::test]
async fn root_serves_the_spa_and_old_ws_path_never_upgrades() {
    let (_directory, core) = public_fixture();
    let client = reqwest::Client::new();
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
        let denied =
            tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}{path}", core.port)).await;
        match denied {
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
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
async fn encrypted_timed_controls_forward_exact_json_and_touch_inputs_once_and_refuse_replay() {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::Message as ClientMessage;
    let (_directory, core) = public_fixture();
    let state = core.runtime_state();
    let key = plain_rs::utils::base64::base64_decode(&plain_rs::prefs::identity::ensure_url_token(
        &state.prefs,
    ));
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
    let (mut socket, _) = tokio_tungstenite::connect_async(format!(
        "ws://127.0.0.1:{}/screen?cid={client}",
        core.port
    ))
    .await
    .unwrap();
    socket
        .send(ClientMessage::Binary(
            plain_rs::crypto::xchacha_encrypt_raw(
                &key,
                chrono::Utc::now().timestamp_millis().to_string().as_bytes(),
            )
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
    let json_control = br#"{"type":"screenMirrorControl","input":{"action":"BACK"}}"#;
    let touch = [0x54, 1, 0xff, 0xff, 0, 7, 0x34, 0x12, 0x78, 0x56, 0, 0];
    let mut last = Vec::new();
    for payload in [json_control.as_slice(), touch.as_slice()] {
        let mut plain = chrono::Utc::now().timestamp_millis().to_be_bytes().to_vec();
        plain.extend_from_slice(payload);
        last = plain_rs::crypto::xchacha_encrypt_raw(&key, &plain).unwrap();
        socket
            .send(ClientMessage::Binary(last.clone().into()))
            .await
            .unwrap();
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
                assert_eq!(u16::from(close.code), 1013);
                assert_eq!(close.reason, "invalid_request");
                return;
            }
        }
        panic!("Replayed control must receive an explicit rejection");
    })
    .await
    .unwrap();
    assert!(received.try_recv().is_err());
    core.shutdown().await;
    state.host.disconnect(generation);
    tokio::time::timeout(Duration::from_secs(3), responder)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn host_replacement_restores_only_authenticated_online_clients() {
    let directory = tempfile::tempdir().unwrap();
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
            plain_rs::host::request_scope::NativeRequestScope::default(),
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
    let (_directory, core) = public_fixture();
    let state = core.runtime_state();
    let native = core.native_graphql().unwrap();
    let native_id = native
        .open_subscriptions(plain_rs::host::request_scope::NativeRequestScope::default())
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
    for _ in 0..plain_rs::host::bridge::REQUEST_CAPACITY {
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
            plain_rs::host::request_scope::NativeRequestScope::default(),
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
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::{Message as Frame, client::IntoClientRequest};

    let (_directory, core) = public_fixture();
    let state = core.runtime_state();
    let client = state.native_client_id().unwrap();
    let key = plain_rs::utils::base64::base64_decode(&plain_rs::prefs::identity::ensure_url_token(
        &state.prefs,
    ));
    let native = core.native_graphql().unwrap();
    let native_id = native
        .open_subscriptions(plain_rs::host::request_scope::NativeRequestScope::default())
        .unwrap();
    let host = core.host_events().unwrap();
    host_online(&host, json!([])).await;
    assert!(state.subscription_sessions.snapshot().is_empty());
    let mut invalid_request = format!("ws://127.0.0.1:{}/graphql?cid={client}", core.port)
        .into_client_request()
        .unwrap();
    invalid_request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        "plain-graphql-ws-v1".parse().unwrap(),
    );
    let (mut invalid, _) = tokio_tungstenite::connect_async(invalid_request)
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
            let mut request = format!("ws://127.0.0.1:{port}/graphql?cid={client}")
                .into_client_request()
                .unwrap();
            request.headers_mut().insert(
                "Sec-WebSocket-Protocol",
                "plain-graphql-ws-v1".parse().unwrap(),
            );
            let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
            let init = plain_rs::protocols::graphql::seal_init(&key, &client, json!({"timestampMillis": chrono::Utc::now().timestamp_millis(), "clientNonce": nonce.repeat(32)}).to_string().as_bytes()).unwrap();
            socket
                .send(Frame::Binary(init.clone().into()))
                .await
                .unwrap();
            let Frame::Binary(ack) = socket.next().await.unwrap().unwrap() else {
                panic!("Expected encrypted acknowledgement")
            };
            assert!(plain_rs::protocols::graphql::open_ack(&key, &client, &init, &ack).is_some());
            socket.send(Frame::Ping(vec![1].into())).await.unwrap();
            assert!(matches!(
                socket.next().await.unwrap().unwrap(),
                Frame::Pong(_)
            ));
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
            Some(plain_rs::base64_decode(&credentials.token)),
            plain_rs::host::request_scope::NativeRequestScope::default(),
        )
        .unwrap();
    session.send(r#"{"type":"subscribe","id":"public","payload":{"query":"subscription { pairingCompleted { deviceId deviceName ip } }"}}"#).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let response: Value = serde_json::from_str(&session.read().await.unwrap()).unwrap();
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
            "PAIRING_SUCCESS",
            json!({"deviceId":"peer","deviceName":"Peer","ip":"host","key":"secret"}).to_string(),
        ))
        .unwrap();
    let message = tokio::time::timeout(Duration::from_secs(3), session.read())
        .await
        .unwrap()
        .unwrap();
    let response: Value = serde_json::from_str(&message).unwrap();
    assert_eq!(
        response["payload"]["data"]["pairingCompleted"],
        json!({"deviceId":"peer","deviceName":"Peer","ip":"host"})
    );
    assert!(!response.to_string().contains("secret"));
    session.send(r#"{"type":"subscribe","id":"channels","payload":{"query":"subscription { channelsChanged { changed invite { channelId channelName ownerPeerId ownerPeerName } cancel { channelId } } }"}}"#).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let response: Value = serde_json::from_str(&session.read().await.unwrap()).unwrap();
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
    let response: Value = serde_json::from_str(&message).unwrap();
    assert_eq!(
        response["payload"]["data"]["channelsChanged"]["invite"]["channelName"],
        "Channel"
    );
    state.subscription_sessions.close(session.id);
    core.shutdown().await;
}

#[cfg(feature = "http_transport")]
async fn password_keys(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    state: &ServerState,
    client_id: &str,
) -> plain_rs::services::confirmation_channel::Channel {
    use futures_util::{SinkExt, StreamExt};
    use plain_rs::services::confirmation_channel::{Client, ConfirmationMode};
    use tokio_tungstenite::tungstenite::Message;
    let (client, hello) = Client::start(client_id, ConfirmationMode::Password).unwrap();
    socket
        .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
        .await
        .unwrap();
    let response = tokio::time::timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let Message::Text(response) = response else {
        panic!("signed password challenge required")
    };
    let signing = plain_rs::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
    let (keys, reveal) = client
        .accept(
            &serde_json::from_str(&response).unwrap(),
            &plain_rs::utils::base64::base64_encode(&signing[32..]),
            &state.native_client_id().unwrap(),
            chrono::Utc::now().timestamp_millis() as u64,
        )
        .unwrap();
    socket
        .send(Message::Text(
            serde_json::to_string(&reveal).unwrap().into(),
        ))
        .await
        .unwrap();
    keys
}

#[test]
fn unauthenticated_connection_limits_release_only_the_owning_source_slot() {
    let runtime = Arc::new(Runtime::default());
    let first: IpAddr = "192.0.2.1".parse().unwrap();
    let second: IpAddr = "192.0.2.2".parse().unwrap();
    let mut permits = Vec::new();
    for _ in 0..MAX_UNAUTHENTICATED_CONNECTIONS_PER_IP {
        permits.push(runtime.reserve_unauthenticated(first).unwrap());
    }
    assert!(runtime.reserve_unauthenticated(first).is_none());
    let other = runtime.reserve_unauthenticated(second).unwrap();
    drop(permits.pop());
    let replacement = runtime.reserve_unauthenticated(first).unwrap();
    assert_eq!(
        runtime.unauthenticated.lock().unwrap().get(&second),
        Some(&1)
    );
    drop((permits, other, replacement));
    assert!(runtime.unauthenticated.lock().unwrap().is_empty());
}

#[test]
fn unauthenticated_connections_have_a_global_limit_across_sources() {
    let runtime = Arc::new(Runtime::default());
    let mut permits = Vec::new();
    for source in 0..MAX_UNAUTHENTICATED_CONNECTIONS {
        let ip = IpAddr::V4(std::net::Ipv4Addr::new(192, 0, 2, source as u8));
        permits.push(runtime.reserve_unauthenticated(ip).unwrap());
    }
    let next: IpAddr = "192.0.2.254".parse().unwrap();
    assert!(runtime.reserve_unauthenticated(next).is_none());
    drop(permits.pop());
    let permit = runtime.reserve_unauthenticated(next).unwrap();
    drop((permits, permit));
    assert!(runtime.unauthenticated.lock().unwrap().is_empty());
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn idle_auth_screen_and_editor_connections_expire_before_authentication() {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;
    let (_directory, server) = public_fixture();
    let mut sockets = Vec::new();
    for path in [
        "auth?cid=idle&auth=2",
        "screen?cid=idle",
        "image_editor?cid=idle",
    ] {
        let (socket, _) =
            tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/{path}", server.port,))
                .await
                .unwrap();
        sockets.push(socket);
    }
    for socket in &mut sockets {
        let message = tokio::time::timeout(
            INITIAL_MESSAGE_TIMEOUT + Duration::from_secs(2),
            socket.next(),
        )
        .await
        .unwrap()
        .unwrap()
        .unwrap();
        assert!(matches!(message, Message::Close(Some(frame)) if frame.reason == "timeout"));
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        while !server
            .runtime_state()
            .main_ws
            .unauthenticated
            .lock()
            .unwrap()
            .is_empty()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn authenticated_editor_connections_release_unauthenticated_slots() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let (_directory, server) = public_fixture();
    let state = server.runtime_state();
    let client_id = state.native_client_id().unwrap();
    let key = plain_rs::utils::base64::base64_decode(&plain_rs::prefs::identity::ensure_url_token(
        &state.prefs,
    ));
    let mut sockets = Vec::new();
    for _ in 0..=MAX_UNAUTHENTICATED_CONNECTIONS_PER_IP {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{}/image_editor?cid={client_id}",
            server.port,
        ))
        .await
        .unwrap();
        let proof = plain_rs::crypto::xchacha_encrypt_raw(
            &key,
            chrono::Utc::now().timestamp_millis().to_string().as_bytes(),
        )
        .unwrap();
        socket.send(Message::Binary(proof.into())).await.unwrap();
        let response = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(response, Message::Binary(_)));
        sockets.push(socket);
    }
    assert!(state.main_ws.unauthenticated.lock().unwrap().is_empty());
    for mut socket in sockets {
        socket.close(None).await.unwrap();
    }
    server.shutdown().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn non_mobile_native_subscriptions_reject_sms_fields() {
    let (_directory, core) = public_fixture();
    let native = core.native_graphql().unwrap();
    let id = native
        .open_subscriptions(plain_rs::host::request_scope::NativeRequestScope::default())
        .unwrap();
    let session = native.subscription_session(id).unwrap();
    session.send(r#"{"type":"subscribe","id":"sms","payload":{"query":"subscription { smsSendResult { requestId } }"}}"#).unwrap();
    let response: Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(3), session.read())
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(response["type"], "error");
    assert_eq!(
        response["payload"][0]["message"],
        "Unknown subscription field"
    );
    native.close_subscriptions(id);
    core.stop_core().await;
}
