use super::*;
use std::sync::Arc;
use std::time::Duration;
fn fixture() -> (tempfile::TempDir, crate::server::ContentServer) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "actor").unwrap();
    prefs.set("background_service_enabled", true).unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    (dir, server)
}
#[tokio::test]
async fn removed_ble_websocket_does_not_upgrade() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let (_dir, server) = fixture();
    let mut request = format!("ws://127.0.0.1:{}/chat/ble-incoming", server.port)
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "authorization",
        format!(
            "Bearer {}",
            plain_rs::utils::base64::base64_encode(&[3; 32])
        )
        .parse()
        .unwrap(),
    );
    assert!(tokio_tungstenite::connect_async(request).await.is_err());
    server.shutdown().await;
}

#[tokio::test]
async fn outgoing_completes_in_one_host_call() {
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    let (generation, mut requests) = state.host.connect();
    let task_state = state.clone();
    let peer = plain_rs::db::DPeer::new(
        "peer",
        "peer",
        "",
        Some(443),
        plain_rs::chat::enums::DeviceType::Phone,
    );
    let task = tokio::spawn(async move {
        task_state
            .ble_transport
            .exchange(
                &task_state,
                &peer,
                Request::PeerGraphql {
                    client_id: "actor".into(),
                    channel_id: "频道".into(),
                    body: vec![0, 255],
                }
                .encode()
                .unwrap(),
            )
            .await
            .unwrap()
    });
    // Rust asks the host for the whole exchange in a single call, carrying the
    // encoded request — there is no websocket token/handshake in between.
    let call = requests.recv().await.unwrap();
    assert_eq!(call["method"], "peerTransportBleExchange");
    assert!(call["params"].get("token").is_none());
    let request =
        plain_rs::utils::base64::base64_decode(call["params"]["request"].as_str().unwrap());
    assert_eq!(
        Request::decode(&request).unwrap(),
        Request::PeerGraphql {
            client_id: "actor".into(),
            channel_id: "频道".into(),
            body: vec![0, 255]
        }
    );
    let reply = ble::response(403, &[255, 0]).unwrap();
    state
        .host
        .reply(
            generation,
            serde_json::json!({"id":call["id"],"result":{"response":plain_rs::utils::base64::base64_encode(&reply)}}),
        )
        .unwrap();
    assert_eq!(task.await.unwrap(), reply);
    state.host.disconnect(generation);
    server.shutdown().await;
}
#[tokio::test]
async fn incoming_preserves_file_bytes_without_host_bridge() {
    let (dir, server) = fixture();
    let state = server.runtime_state();
    let payload = (0..8192).map(|n| n as u8).collect::<Vec<_>>();
    let file = plain_rs::chat::attachments::app_file_store::import_bytes(
        &state.db,
        dir.path(),
        &payload,
        "image/jpeg",
    )
    .unwrap();
    let id = plain_rs::chat::messages::content::make_file_id(
        &format!("fid:{}", file.fid_suffix),
        &plain_rs::prefs::ensure_url_token(&state.prefs),
    );
    let bytes = incoming_exchange(
        &server,
        ble::HTTP_CHAR_UUID,
        "actual MAC",
        Request::FileChunk {
            client_id: "actor".into(),
            file_id: id,
            offset: 0,
            length: 8192,
        }
        .encode()
        .unwrap(),
    )
    .await
    .unwrap();
    let (status, body) = ble::decode_response(&bytes).unwrap();
    assert_eq!(status, 200);
    assert_eq!(body, payload);
    assert!(!state.host.connected());
    server.shutdown().await;
}
async fn incoming_exchange(
    server: &crate::server::ContentServer,
    characteristic: &str,
    address: &str,
    bytes: Vec<u8>,
) -> Option<Vec<u8>> {
    crate::native::management::NativeManagement::new(server.runtime_state())
        .ble_incoming(address.to_owned(), characteristic.to_owned(), bytes)
        .await
        .ok()
}

#[tokio::test]
async fn nearby_discovery_uses_the_existing_raw_channel_and_shared_identity() {
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    state.prefs.set("device_name", "BLE fixture").unwrap();
    let bytes = incoming_exchange(
        &server,
        ble::NEARBY_CHAR_UUID,
        "radio-source",
        ble::message(&[], b"DISCOVER:").unwrap(),
    )
    .await
    .unwrap();
    let (metadata, body) = ble::parts(&bytes).unwrap();
    assert!(metadata.is_empty());
    let value: serde_json::Value = serde_json::from_slice(body).unwrap();
    assert_eq!(value["id"], "actor");
    assert_eq!(value["name"], "BLE fixture");
    assert_eq!(value["deviceType"], "COMPUTER");
    assert_eq!(value["port"], 0);
    assert!(!state.host.connected());
    server.shutdown().await;
}

#[tokio::test]
async fn incoming_characteristics_are_allowlisted_and_cannot_cross_protocols() {
    let (_dir, server) = fixture();
    let discover = ble::message(&[], b"DISCOVER:").unwrap();
    assert!(
        incoming_exchange(
            &server,
            "not-a-characteristic",
            "radio-source",
            discover.clone()
        )
        .await
        .is_none()
    );
    assert!(
        incoming_exchange(&server, ble::HTTP_CHAR_UUID, "radio-source", discover)
            .await
            .is_none()
    );
    let request = Request::FileChunk {
        client_id: "actor".into(),
        file_id: "fixture".into(),
        offset: 0,
        length: 1,
    }
    .encode()
    .unwrap();
    assert!(
        incoming_exchange(&server, ble::NEARBY_CHAR_UUID, "radio-source", request)
            .await
            .is_none()
    );
    for body in ["UNKNOWN:{}", "PAIR_REQUEST:{}", "DISCOVER_REPLY:{}"] {
        let bytes = incoming_exchange(
            &server,
            ble::NEARBY_CHAR_UUID,
            "radio-source",
            ble::message(&[], body.as_bytes()).unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(bytes, ble::message(&[], &[]).unwrap());
    }
    server.shutdown().await;
}

fn signed_nearby_request() -> (plain_rs::peers::pairing::protocol::PairingRequest, [u8; 64]) {
    let (key, public) = plain_rs::crypto::ed25519_generate();
    let mut request = plain_rs::peers::pairing::protocol::PairingRequest {
        from_id: "nearby-peer".into(),
        from_name: "Nearby fixture".into(),
        port: 2443,
        device_type: "PHONE".into(),
        ecdh_public_key: plain_rs::utils::base64::base64_encode(
            &plain_rs::crypto::EcdhSession::generate().public_key_bytes,
        ),
        signature_public_key: plain_rs::utils::base64::base64_encode(&public),
        timestamp: plain_rs::peers::pairing::now_ms(),
        ips: vec![],
        signature: String::new(),
        aware_supported: false,
        from_ip: String::new(),
    };
    request.signature = plain_rs::crypto::ed25519_sign(&key, request.signature_data().as_bytes());
    (request, key)
}

#[tokio::test]
async fn nearby_pairing_preserves_signatures_and_the_actual_radio_return_address() {
    use plain_rs::discovery::nearby_wire::Message;
    use plain_rs::services::discovery::pairing;
    use plain_rs::services::discovery::pairing_runtime;
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    let (local_key, _) = plain_rs::crypto::ed25519_generate();
    state
        .prefs
        .set(
            "signature_key_pair",
            plain_rs::utils::base64::base64_encode(&local_key),
        )
        .unwrap();
    let (request, _) = signed_nearby_request();
    let mut invalid = request.clone();
    invalid.from_name.push_str("tampered");
    let mut events = state.events.subscribe();
    let body = Message::PairRequest(invalid).wire().unwrap();
    let ack = incoming_exchange(
        &server,
        ble::NEARBY_CHAR_UUID,
        "untrusted-route",
        ble::message(&[], body.as_bytes()).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(ble::nearby_body(&ack).unwrap(), b"1");
    assert!(
        state
            .pairing
            .incoming_target(&request.from_id, &request.signature)
            .is_none()
    );
    assert!(events.try_recv().is_err());
    let body = Message::PairRequest(request.clone()).wire().unwrap();
    let ack = incoming_exchange(
        &server,
        ble::NEARBY_CHAR_UUID,
        "actual-radio-source",
        ble::message(&[], body.as_bytes()).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(ble::nearby_body(&ack).unwrap(), b"1");
    assert!(events.try_recv().is_err());
    let verification_code =
        plain_rs::services::discovery::pairing_confirmation::test_fixture::verify_incoming(
            &state, &request,
        );
    let event = events.try_recv().unwrap();
    assert_eq!(event.event_type, "PAIRING_REQUEST_RECEIVED");
    let delivered: plain_rs::peers::pairing::protocol::PairingRequest =
        serde_json::from_str(&event.payload).unwrap();
    assert_eq!(delivered.signature, request.signature);
    let notice: serde_json::Value = serde_json::from_str(&event.payload).unwrap();
    assert_eq!(notice["verificationCode"], verification_code);
    assert_eq!(delivered.from_ip, "");
    assert!(plain_rs::peers::pairing::security::verify_request(
        &delivered
    ));
    let (generation, mut calls) = state.host.connect();
    let responding = state.clone();
    let expected_signature = request.signature.clone();
    let task = tokio::spawn(async move {
        pairing_runtime::execute(
            &responding,
            pairing_runtime::Request::Respond {
                request,
                accepted: false,
                device: pairing::Device {
                    name: "Local fixture".into(),
                    port: 2443,
                    device_type: "COMPUTER".into(),
                    ips: vec![],
                    aware_supported: false,
                },
            },
        )
        .await
        .unwrap()
    });
    let call = tokio::time::timeout(Duration::from_secs(2), calls.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(call["method"], "pairingNotification");
    assert_eq!(call["params"]["address"], "actual-radio-source");
    let Message::PairResponse(response) =
        Message::parse(call["params"]["body"].as_str().unwrap()).unwrap()
    else {
        panic!("Expected pairing response")
    };
    assert_eq!(response.request_signature, expected_signature);
    assert!(!response.accepted);
    assert!(plain_rs::peers::pairing::security::verify_response(
        &response,
        "nearby-peer",
        "actor"
    ));
    state
        .host
        .reply(
            generation,
            serde_json::json!({"id":call["id"],"result":true}),
        )
        .unwrap();
    assert!(
        !task.await.unwrap()["response"]["accepted"]
            .as_bool()
            .unwrap()
    );
    assert!(
        plain_rs::db::chat_store::peers::get(&state.db, "nearby-peer")
            .unwrap()
            .is_none()
    );
    state.host.disconnect(generation);
    server.shutdown().await;
}

#[tokio::test]
async fn nearby_cancel_requires_the_matching_signed_request() {
    use plain_rs::discovery::nearby_wire::Message;
    use plain_rs::peers::pairing::protocol::PairingCancel;
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    let (request, key) = signed_nearby_request();
    let body = Message::PairRequest(request.clone()).wire().unwrap();
    let ack = incoming_exchange(
        &server,
        ble::NEARBY_CHAR_UUID,
        "radio-source",
        ble::message(&[], body.as_bytes()).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(ble::nearby_body(&ack).unwrap(), b"1");
    let mut cancel = PairingCancel {
        from_id: request.from_id.clone(),
        to_id: "actor".into(),
        request_signature: "another-request".into(),
        signature_public_key: request.signature_public_key.clone(),
        timestamp: plain_rs::peers::pairing::now_ms(),
        signature: String::new(),
    };
    for matching in [false, true] {
        if matching {
            cancel.request_signature = request.signature.clone();
        }
        cancel.signature = plain_rs::crypto::ed25519_sign(&key, cancel.signature_data().as_bytes());
        let body = Message::PairCancel(cancel.clone()).wire().unwrap();
        let ack = incoming_exchange(
            &server,
            ble::NEARBY_CHAR_UUID,
            "radio-source",
            ble::message(&[], body.as_bytes()).unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(ble::nearby_body(&ack).unwrap(), b"1");
        assert_eq!(
            state
                .pairing
                .incoming_target(&request.from_id, &request.signature)
                .is_none(),
            matching
        );
    }
    assert!(
        plain_rs::db::chat_store::peers::get(&state.db, "nearby-peer")
            .unwrap()
            .is_none()
    );
    server.shutdown().await;
}
