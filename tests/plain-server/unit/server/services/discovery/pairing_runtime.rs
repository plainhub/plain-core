use plain_libs::protocols::http_client;
use plain_rs::features::discovery::nearby_wire::Message;
use plain_rs::features::peers::pairing::sessions::Target;
use plain_rs::features::services::discovery::pairing::Device;
use plain_rs::features::services::discovery::pairing_runtime::*;
use plain_libs::serde_json::json;

#[cfg(feature = "http_transport")]
fn fixture(id: &str) -> (plain_libs::temporary::TempDir, crate::server::ContentServer) {
    use std::sync::Arc;
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    let (key, _public) = plain_libs::crypto::ed25519_generate();
    prefs.set("client_id", id).unwrap();
    prefs
        .set(
            "signature_key_pair",
            plain_libs::utils::base64::base64_encode(&key),
        )
        .unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    (dir, server)
}
#[cfg(feature = "http_transport")]
fn device() -> Device {
    Device {
        name: "Fixture".into(),
        port: 2443,
        device_type: plain_rs::features::chat::enums::DeviceType::Phone,
        ips: vec!["127.0.0.1".into()],
        aware_supported: false,
    }
}
#[cfg(feature = "http_transport")]
#[tokio::test]
async fn public_nearby_endpoint_runs_pairing_parser_in_rust() {
    let (_dir, server) = fixture("706c6169-0001-8000-8000-000000000001");
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, https_port) = server
        .start_public(0, 0, cert_pem.clone(), cert_key.clone())
        .await
        .unwrap();
    let client = http_client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let response = client
        .post(format!("https://127.0.0.1:{https_port}/nearby"))
        .body("NOPE:{{}}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), http_client::StatusCode::BAD_REQUEST);
    assert_eq!(response.text().await.unwrap(), "unknown message type");
    let malformed = client
        .post(format!("https://127.0.0.1:{https_port}/nearby"))
        .body("PAIR_REQUEST:{}")
        .send()
        .await
        .unwrap();
    assert_eq!(malformed.status(), http_client::StatusCode::BAD_REQUEST);
    assert_eq!(malformed.text().await.unwrap(), "invalid nearby message");
    for body in [
        "DISCOVER:",
        r#"DISCOVER_REPLY:{"id":"peer","name":"Fixture","port":1234,"signaturePublicKey":"key","deviceType":"PHONE","version":"1","platform":"test"}"#,
    ] {
        let response = client
            .post(format!("https://127.0.0.1:{https_port}/nearby"))
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), http_client::StatusCode::BAD_REQUEST);
        assert_eq!(response.text().await.unwrap(), "unknown message type");
    }
    let health = client
        .get(format!("https://127.0.0.1:{https_port}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), http_client::StatusCode::OK);
    assert_eq!(health.text().await.unwrap(), "OK");
    let state = server.runtime_state();
    let recipient_key =
        plain_rs::features::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
    let (sender_key, sender_public_key) = plain_libs::crypto::ed25519_generate();
    let encode = plain_libs::utils::base64::base64_encode;
    let mut request = plain_rs::features::peers::pairing::protocol::PairingRequest {
        from_id: "peer".into(),
        to_id: "706c6169-0001-8000-8000-000000000001".into(),
        target_public_key: encode(&recipient_key[32..]),
        from_ip: String::new(),
        payload: plain_rs::features::peers::pairing::protocol::PairingRequestPayload {
            from_name: "Fixture".into(),
            port: 2443,
            device_type: plain_rs::features::chat::enums::DeviceType::Phone,
            ecdh_public_key: encode(&plain_libs::crypto::EcdhSession::generate().public_key_bytes),
            signature_public_key: encode(&sender_public_key),
            timestamp: plain_rs::features::peers::pairing::now_ms() - 300_001,
            ips: vec![],
            signature: String::new(),
            aware_supported: false,
        },
    };
    for (header, fresh, correct_target, discoverable, pinned, accepted) in [
        (None, true, true, true, None, false),
        (Some("another-peer"), true, true, true, None, false),
        (Some("peer"), false, true, true, None, false),
        (Some("peer"), true, false, true, None, false),
        (Some("peer"), true, true, false, None, false),
        (Some("peer"), true, true, true, Some(encode(&recipient_key[32..])), false),
        (Some("peer"), true, true, false, Some(encode(&sender_public_key)), true),
    ] {
        state.prefs.set("nearby_discoverable", discoverable).unwrap();
        request.payload.timestamp =
            plain_rs::features::peers::pairing::now_ms() - if fresh { 0 } else { 300_001 };
        request.target_public_key = if correct_target {
            encode(&recipient_key[32..])
        } else {
            encode(&sender_public_key)
        };
        request.payload.signature =
            plain_libs::crypto::ed25519_sign(&sender_key, request.signature_data().as_bytes());
        if let Some(public_key) = pinned {
            let mut pinned = plain_rs::infrastructure::db::DPeer::new(
                &request.from_id, "Fixture", "192.0.2.1", None,
                plain_rs::features::chat::enums::DeviceType::Phone,
            );
            pinned.public_key = public_key;
            plain_rs::infrastructure::db::chat_store::peers::save(
                &state.db, &[pinned], plain_rs::infrastructure::db::chat_store::SaveMode::Upsert,
            ).unwrap();
        }
        let body = Message::PairRequest(request.payload.clone())
            .wire()
            .unwrap();
        let mut post = client
            .post(format!("https://127.0.0.1:{https_port}/nearby"))
            .body(body);
        if let Some(header) = header {
            post = post
                .header("c-id", header)
                .header("x-forwarded-for", "192.0.2.20");
        }
        let reply = post.send().await.unwrap();
        assert_eq!(
            reply.status(),
            if header.is_some() {
                http_client::StatusCode::OK
            } else {
                http_client::StatusCode::BAD_REQUEST
            },
        );
        let incoming = state
            .pairing
            .incoming_target("peer", &request.payload.signature);
        if accepted {
            assert_eq!(incoming.unwrap().device_ip, "127.0.0.1");
        } else {
            assert!(incoming.is_none());
        }
        assert!(
            state
                .pairing
                .incoming_target("another-peer", &request.payload.signature)
                .is_none()
        );
    }
    server.stop_public().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn lan_runtime_sends_signed_tls_requests_and_cancel_cannot_remove_replacement() {
    use plain_libs::axum::{Router, body::Bytes, extract::State, http::HeaderMap, routing::post};
    async fn receive(
        State(sender): State<tokio::sync::mpsc::Sender<Message>>,
        headers: HeaderMap,
        body: Bytes,
    ) -> &'static str {
        let message = Message::parse(std::str::from_utf8(&body).unwrap()).unwrap();
        if matches!(&message, Message::PairRequest(_)) {
            assert_eq!(headers["c-id"], "706c6169-0001-8000-8000-000000000001");
        }
        sender.send(message).await.unwrap();
        "1"
    }
    let (_dir, server) = fixture("706c6169-0001-8000-8000-000000000001");
    let state = server.runtime_state();
    let (send, mut received) = tokio::sync::mpsc::channel(8);
    let router = Router::new()
        .route("/health", plain_libs::axum::routing::get(crate::http::health::health))
        .route("/nearby", post(receive))
        .with_state(send);
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let listeners = crate::http::listeners::HttpListeners::start(
        router,
        0,
        0,
        cert_pem.clone(),
        cert_key.clone(),
    )
    .await
    .unwrap();
    let target = Target {
        device_id: "remote".into(),
        device_name: "Fixture".into(),
        device_ip: "stale".into(),
        device_port: listeners.https_port,
        device_public_key: plain_libs::utils::base64::base64_encode(
            &plain_libs::crypto::ed25519_generate().1,
        ),
    };
    let start = |method| Request::Start {
        methods: vec![
            method,
            plain_rs::features::discovery::nearby_devices::DiscoveryMethod::Ble,
        ],
        ble: true,
        target: target.clone(),
        ips: vec!["invalid".into(), "127.0.0.1".into()],
        device: device(),
        generation: None,
    };
    let first = execute(
        &state,
        start(plain_rs::features::discovery::nearby_devices::DiscoveryMethod::Lan),
    )
    .await
    .unwrap();
    assert_eq!(first["sent"], true);
    assert_eq!(first["ticket"]["deviceIp"], "127.0.0.1");
    let Message::PairRequest(payload) =
        tokio::time::timeout(std::time::Duration::from_secs(5), received.recv())
            .await
            .unwrap()
            .unwrap()
    else {
        panic!()
    };
    let request = plain_rs::features::peers::pairing::protocol::PairingRequest {
        to_id: target.device_id.clone(),
        target_public_key: target.device_public_key.clone(),
        from_id: "706c6169-0001-8000-8000-000000000001".into(),
        from_ip: String::new(),
        payload,
    };
    assert!(plain_rs::features::peers::pairing::security::verify_request(&request));
    let wire = plain_libs::serde_json::to_value(request.payload).unwrap();
    for removed in ["fromId", "fromIp", "toId", "targetPublicKey"] {
        assert!(
            wire.get(removed).is_none(),
            "{removed} leaked into the body"
        );
    }
    assert!(
        received.try_recv().is_err(),
        "The first pairing frame contains the full request"
    );
    let second = execute(
        &state,
        start(plain_rs::features::discovery::nearby_devices::DiscoveryMethod::Lan),
    )
    .await
    .unwrap();
    assert_eq!(second["sent"], true);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), received.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        received.try_recv().is_err(),
        "The first pairing frame contains the full request"
    );
    assert!(
        execute(
            &state,
            Request::Cancel {
                id: "remote".into(),
                generation: Some(first["ticket"]["generation"].as_str().unwrap().into())
            }
        )
        .await
        .unwrap()
        .is_null()
    );
    assert_eq!(
        state.pairing.tickets()[0].generation,
        second["ticket"]["generation"].as_str().unwrap()
    );
    let canceled = execute(
        &state,
        Request::Cancel {
            id: "remote".into(),
            generation: None,
        },
    )
    .await
    .unwrap();
    assert!(!canceled.is_null());
    let Message::PairCancel(cancel) =
        tokio::time::timeout(std::time::Duration::from_secs(5), received.recv())
            .await
            .unwrap()
            .unwrap()
    else {
        panic!()
    };
    assert_eq!(cancel.to_id, "remote");
    assert_eq!(cancel.from_id, "706c6169-0001-8000-8000-000000000001");
    assert!(state.pairing.tickets().is_empty());
    listeners.shutdown().await;
    let failed = execute(
        &state,
        start(plain_rs::features::discovery::nearby_devices::DiscoveryMethod::Lan),
    )
    .await
    .unwrap();
    assert_eq!(failed["sent"], false);
    assert!(state.pairing.tickets().is_empty());
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn native_main_pairing_queries_are_read_only_and_reject_stale_request_identity() {
    let (_dir, server) = fixture("706c6169-1dfd-8b72-82bb-3ad00a9928ca");
    let native = server.native_graphql().unwrap();
    let response = native
        .execute(plain_graphql::Request::new(
            r#"{ peers { id } }"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["peers"], json!([]));
    assert!(
        !server
            .native_management()
            .unwrap()
            .is_pairing_request_current("absent".into(), "stale".into())
            .unwrap()
    );
    let response = native
        .execute(plain_graphql::Request::new(
            r#"mutation { cancelPairing(peerId:"absent") }"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["cancelPairing"], true);
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn unavailable_nearby_transport_reports_a_generic_pairing_failure() {
    let (_dir, server) = fixture("706c6169-0001-8000-8000-000000000001");
    let state = server.runtime_state();
    let mut events = state.events.subscribe();
    execute(
        &state,
        Request::Start {
            target: Target {
                device_id: "fixture-peer".into(),
                device_name: "Fixture".into(),
                device_ip: String::new(),
                device_port: 0,
                device_public_key: String::new(),
            },
            ips: vec![],
            methods: vec![],
            ble: false,
            device: device(),
            generation: None,
        },
    ).await.unwrap();
    let failure = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| event.event_type == "PAIRING_FAILED")
        .unwrap();
    let payload: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&failure.payload).unwrap();
    assert_eq!(payload["error"], "INTERNAL_ERROR");
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn first_nearby_chat_pairing_waits_for_matching_code_approval_without_qr() {
    use plain_rs::features::chat::events::{WS_PAIRING_COMPLETED, WS_PAIRING_REQUESTED, WS_PAIRING_STARTED};
    use plain_rs::features::discovery::nearby_devices::{Device as NearbyDevice, DiscoveryMethod};
    use plain_rs::infrastructure::db::chat_store::peers;
    use plain_rs::infrastructure::events::WsEvent;
    use tokio::sync::broadcast;
    use tokio::time::{Duration, timeout};

    async fn next_event(events: &mut broadcast::Receiver<WsEvent>, kind: &str) -> plain_libs::serde_json::Value {
        timeout(Duration::from_secs(5), async {
            loop {
                let event = events.recv().await.unwrap();
                if event.event_type == kind {
                    return plain_libs::serde_json::from_str(&event.payload).unwrap();
                }
            }
        }).await.unwrap()
    }

    let a_id = "706c6169-0001-8000-8000-000000000001";
    let b_id = "706c6169-0002-8000-8000-000000000002";
    let (_a_dir, a) = fixture(a_id);
    let (_b_dir, b) = fixture(b_id);
    let (cert, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    a.start_public(0, 0, cert.clone(), cert_key.clone()).await.unwrap();
    let (_, port) = b.start_public(0, 0, cert, cert_key).await.unwrap();
    let a_state = a.runtime_state();
    let b_state = b.runtime_state();
    let b_key = plain_rs::features::services::peers::peer_wire::signing_keypair(&b_state.prefs).unwrap();
    a_state.nearby_devices.observe(NearbyDevice {
        id: b_id.into(),
        name: "Nearby fixture".into(),
        ips: vec!["127.0.0.1".into()],
        port,
        remote_access: Some(false),
        signature_public_key: plain_libs::utils::base64::base64_encode(&b_key[32..]),
        device_type: device().device_type,
        version: "test".into(),
        platform: "test".into(),
        last_seen_at: plain_libs::utils::dbtime::now_iso(),
        discovery_methods: vec![DiscoveryMethod::Lan],
    }).unwrap();
    assert!(peers::get(&a_state.db, b_id).unwrap().is_none());
    assert!(peers::get(&b_state.db, a_id).unwrap().is_none());

    let mut a_events = a_state.events.subscribe();
    let mut b_events = b_state.events.subscribe();
    let response = a.native_graphql().unwrap().execute(format!(
        r#"mutation {{ pairPeer(input: {{peerId:"{b_id}",mode:CHAT}}) }}"#
    ).into()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["pairPeer"], true);
    let sent = next_event(&mut a_events, WS_PAIRING_STARTED).await;
    let received = next_event(&mut b_events, WS_PAIRING_REQUESTED).await;
    assert_eq!(sent["deviceId"], b_id);
    assert_eq!(received["fromId"], a_id);
    assert_eq!(sent["verificationCode"].as_str().map(str::len), Some(8));
    assert_eq!(sent["verificationCode"], received["verificationCode"]);
    assert!(peers::get(&a_state.db, b_id).unwrap().is_none());
    assert!(peers::get(&b_state.db, a_id).unwrap().is_none());

    let response = b.native_graphql().unwrap().execute(format!(
        r#"mutation {{ respondToPairing(peerId:"{a_id}",accepted:true) }}"#
    ).into()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["respondToPairing"], true);
    next_event(&mut a_events, WS_PAIRING_COMPLETED).await;
    next_event(&mut b_events, WS_PAIRING_COMPLETED).await;
    let saved_a = peers::get(&b_state.db, a_id).unwrap().unwrap();
    let saved_b = peers::get(&a_state.db, b_id).unwrap().unwrap();
    assert!(saved_a.is_paired() && saved_b.is_paired());
    assert!(!saved_a.key.is_empty());
    assert_eq!(saved_a.key, saved_b.key);
    a.shutdown().await;
    b.shutdown().await;
}
