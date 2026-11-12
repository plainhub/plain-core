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
    let (_dir, server) = fixture("local");
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
        to_id: "local".into(),
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
    for (header, fresh, correct_target, pinned, accepted) in [
        (None, true, true, false, false),
        (Some("another-peer"), true, true, false, false),
        (Some("peer"), false, true, false, false),
        (Some("peer"), true, false, false, false),
        (Some("peer"), true, true, false, false),
        (Some("peer"), true, true, true, true),
    ] {
        request.payload.timestamp =
            plain_rs::features::peers::pairing::now_ms() - if fresh { 0 } else { 300_001 };
        request.target_public_key = if correct_target {
            encode(&recipient_key[32..])
        } else {
            encode(&sender_public_key)
        };
        request.payload.signature =
            plain_libs::crypto::ed25519_sign(&sender_key, request.signature_data().as_bytes());
        if pinned {
            let mut pinned = plain_rs::infrastructure::db::DPeer::new(
                &request.from_id, "Fixture", "192.0.2.1", None,
                plain_rs::features::chat::enums::DeviceType::Phone,
            );
            pinned.public_key = request.payload.signature_public_key.clone();
            plain_rs::infrastructure::db::chat_store::peers::save(
                &state.db, &[pinned], plain_rs::infrastructure::db::chat_store::SaveMode::Insert,
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
            assert_eq!(headers["c-id"], "local");
        }
        sender.send(message).await.unwrap();
        "1"
    }
    let (_dir, server) = fixture("local");
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
        from_id: "local".into(),
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
        start(plain_rs::features::discovery::nearby_devices::DiscoveryMethod::DnsSd),
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
    assert_eq!(cancel.from_id, "local");
    assert!(state.pairing.tickets().is_empty());
    listeners.shutdown().await;
    let failed = execute(
        &state,
        start(plain_rs::features::discovery::nearby_devices::DiscoveryMethod::DnsSd),
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
    let (_dir, server) = fixture("fixture");
    let native = server.native_graphql().unwrap();
    let response = native
        .execute(plain_graphql::Request::new(
            r#"{ pairingStates { deviceId generation phase } }"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["pairingStates"], json!([]));
    assert!(
        !server
            .native_management()
            .unwrap()
            .is_pairing_request_current("absent".into(), "stale".into())
            .unwrap()
    );
    let response = native
        .execute(plain_graphql::Request::new(
            r#"mutation { cancelPairing(generation:"absent") }"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["cancelPairing"], false);
    server.shutdown().await;
}
