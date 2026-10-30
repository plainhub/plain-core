use super::*;
#[test]
fn ble_associations_match_signature_and_cleanup_without_consuming_replacements() {
    let runtime = Runtime::default();
    let mut request:PairingRequest=serde_json::from_value(json!({"fromId":"peer","fromName":"fixture","port":2443,"deviceType":"PHONE","ecdhPublicKey":"","signaturePublicKey":"","timestamp":1,"ips":[],"signature":"first","awareSupported":false})).unwrap();
    runtime.remember(&request, "mac-first").unwrap();
    request.signature = "second".into();
    runtime.remember(&request, "mac-second").unwrap();
    assert!(runtime.take("peer", "first").is_none());
    assert_eq!(runtime.take("peer", "second").unwrap(), "mac-second");
    assert!(runtime.take("peer", "second").is_none());
    runtime.remember(&request, "mac-second").unwrap();
    runtime.forget("peer");
    assert!(runtime.take("peer", "second").is_none());
}

#[cfg(feature = "http_transport")]
fn fixture(id: &str) -> (tempfile::TempDir, crate::assembly::server::ContentServer) {
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    let (key, _public) = plain_rs::crypto::ed25519_generate();
    prefs.set("client_id", id).unwrap();
    prefs
        .set(
            "signature_key_pair",
            plain_rs::utils::base64::base64_encode(&key),
        )
        .unwrap();
    let server = crate::assembly::server::ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
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
        device_type: "PHONE".into(),
        ips: vec!["127.0.0.1".into()],
        aware_supported: false,
    }
}
#[cfg(feature = "http_transport")]
#[tokio::test]
async fn expired_incoming_request_rejects_both_decisions_without_pairing() {
    let (_local_dir, local) = fixture("local");
    let (_remote_dir, remote) = fixture("remote");
    let state = local.runtime_state();
    let remote_state = remote.runtime_state();
    let target = Target {
        device_id: "local".into(),
        device_name: "Local".into(),
        device_ip: "127.0.0.1".into(),
        device_port: 2443,
    };
    let started =
        pairing::start(&remote_state.prefs, &remote_state.pairing, target, device()).unwrap();
    let request: PairingRequest = serde_json::from_value(started["request"].clone()).unwrap();
    execute(
        &state,
        Request::ReceiveRequest {
            request: request.clone(),
            address: "127.0.0.1".into(),
            ble: false,
        },
    )
    .await
    .unwrap();
    super::super::pairing_confirmation::tests::verify_incoming(&state, &request);
    state.pairing.age_incoming_for_test(
        &request.from_id,
        plain_rs::peers::pairing::sessions::RESPONSE_TIMEOUT,
    );
    for accepted in [true, false] {
        let error = execute(
            &state,
            Request::Respond {
                request: request.clone(),
                accepted,
                device: device(),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Pairing request expired or no longer available"
        );
    }
    assert!(plain_rs::db::chat_store::peers::all(&state.db).unwrap().is_empty());
    local.shutdown().await;
    remote.shutdown().await;
}
#[cfg(feature = "http_transport")]
#[tokio::test]
async fn public_nearby_endpoint_runs_pairing_parser_in_rust() {
    let (_dir, server) = fixture("local");
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let (_, https_port) = server
        .start_public(
            0,
            0,
            cert.cert.pem().into_bytes(),
            cert.key_pair.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let response = client
        .post(format!("https://127.0.0.1:{https_port}/nearby"))
        .body("NOPE:{{}}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(response.text().await.unwrap(), "unknown message type");
    let malformed = client
        .post(format!("https://127.0.0.1:{https_port}/nearby"))
        .body("PAIR_REQUEST:{}")
        .send()
        .await
        .unwrap();
    assert_eq!(malformed.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(malformed.text().await.unwrap(), "invalid nearby message");
    let request = client
        .post(format!("https://127.0.0.1:{https_port}/nearby"))
        .body("PAIR_REQUEST:{\"fromId\":\"peer\",\"fromName\":\"fixture\",\"port\":2443,\"deviceType\":\"PHONE\",\"ecdhPublicKey\":\"\",\"signaturePublicKey\":\"\",\"timestamp\":1,\"ips\":[],\"signature\":\"invalid\",\"awareSupported\":false}")
        .send()
        .await
        .unwrap();
    assert_eq!(request.status(), reqwest::StatusCode::OK);
    assert_eq!(request.text().await.unwrap(), "1");
    server.stop_public().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn lan_runtime_sends_signed_tls_requests_and_cancel_cannot_remove_replacement() {
    use axum::{Router, body::Bytes, extract::State, routing::post};
    async fn receive(
        State(sender): State<tokio::sync::mpsc::Sender<Message>>,
        body: Bytes,
    ) -> &'static str {
        let message = Message::parse(std::str::from_utf8(&body).unwrap()).unwrap();
        sender.send(message).await.unwrap();
        "1"
    }
    let (_dir, server) = fixture("local");
    let state = server.runtime_state();
    let (send, mut received) = tokio::sync::mpsc::channel(8);
    let router = Router::new()
        .route("/nearby", post(receive))
        .with_state(send);
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let listeners = crate::http::listeners::HttpListeners::start(
        router,
        0,
        0,
        cert.cert.pem().into_bytes(),
        cert.key_pair.serialize_pem().into_bytes(),
    )
    .await
    .unwrap();
    let target = Target {
        device_id: "remote".into(),
        device_name: "Fixture".into(),
        device_ip: "stale".into(),
        device_port: listeners.https_port,
    };
    let start = || Request::Start {
        methods: vec!["LAN".into(), "BLE".into()],
        ble: true,
        target: target.clone(),
        ips: vec!["invalid".into(), "127.0.0.1".into()],
        device: device(),
        generation: None,
    };
    let first = execute(&state, start()).await.unwrap();
    assert_eq!(first["sent"], true);
    assert_eq!(first["ticket"]["deviceIp"], "127.0.0.1");
    let Message::PairRequest(request) = received.recv().await.unwrap() else {
        panic!()
    };
    assert!(plain_rs::peers::pairing::security::verify_request(&request));
    assert_eq!(request.from_id, "local");
    assert!(matches!(
        received.recv().await.unwrap(),
        Message::PairVerification(_)
    ));
    let second = execute(&state, start()).await.unwrap();
    let _ = received.recv().await.unwrap();
    assert!(matches!(
        received.recv().await.unwrap(),
        Message::PairVerification(_)
    ));
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
    let Message::PairCancel(cancel) = received.recv().await.unwrap() else {
        panic!()
    };
    assert_eq!(cancel.to_id, "remote");
    assert_eq!(cancel.from_id, "local");
    assert!(state.pairing.tickets().is_empty());
    listeners.shutdown().await;
    let failed = execute(&state, start()).await.unwrap();
    assert_eq!(failed["sent"], false);
    assert!(state.pairing.tickets().is_empty());
    server.shutdown().await;
}
#[cfg(feature = "http_transport")]
#[tokio::test]
async fn incoming_ble_response_uses_matching_address_and_shared_handshake_exactly_once() {
    let (_a, local) = fixture("local");
    let (_b, remote) = fixture("remote");
    let state = local.runtime_state();
    let remote_state = remote.runtime_state();
    let target = Target {
        device_id: "local".into(),
        device_name: "Local".into(),
        device_ip: String::new(),
        device_port: 2443,
    };
    let started =
        pairing::start(&remote_state.prefs, &remote_state.pairing, target, device()).unwrap();
    let request: PairingRequest = serde_json::from_value(started["request"].clone()).unwrap();
    assert_eq!(
        execute(
            &state,
            Request::ReceiveRequest {
                request: request.clone(),
                address: "fixture-mac".into(),
                ble: true
            }
        )
        .await
        .unwrap(),
        true
    );
    assert_eq!(
        execute(
            &state,
            Request::ReceiveRequest {
                request: request.clone(),
                address: "fixture-mac".into(),
                ble: true
            }
        )
        .await
        .unwrap(),
        false
    );
    assert!(
        accept_report(
            &state,
            "PAIRING_REQUEST_REPORTED",
            &json!({"request":request,"address":"fixture-mac"}).to_string()
        )
        .await
        .unwrap()
    );
    assert!(
        state
            .pairing
            .incoming_target(&request.from_id, &request.signature)
            .is_some()
    );
    assert!(
        !accept_report(&state, "UNRELATED_EVENT", "not-json")
            .await
            .unwrap()
    );
    assert!(
        accept_report(
            &state,
            "PAIRING_REQUEST_REPORTED",
            &json!({"request":request,"address":"fixture-mac","ble":false}).to_string()
        )
        .await
        .is_err()
    );
    super::super::pairing_confirmation::tests::verify_incoming(&state, &request);
    let (generation, mut calls) = state.host.connect();
    let host = state.host.clone();
    let responder = tokio::spawn(async move {
        let call = calls.recv().await.unwrap();
        assert_eq!(call["method"], "pairingNotification");
        assert_eq!(call["params"]["address"], "fixture-mac");
        let Message::PairResponse(response) =
            Message::parse(call["params"]["body"].as_str().unwrap()).unwrap()
        else {
            panic!()
        };
        host.reply(generation, json!({"id":call["id"],"result":true}))
            .unwrap();
        response
    });
    let accepted = execute(
        &state,
        Request::Respond {
            request: request.clone(),
            accepted: true,
            device: device(),
        },
    )
    .await
    .unwrap();
    let response = responder.await.unwrap();
    let completed = pairing::complete(
        &remote_state.db,
        &remote_state.prefs,
        &remote_state.pairing,
        response,
        "",
    )
    .unwrap();
    assert_eq!(accepted["peer"]["key"], completed["peer"]["key"]);
    for accepted in [true, false] {
        let error = execute(
            &state,
            Request::Respond {
                request: request.clone(),
                accepted,
                device: device(),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Pairing request expired or no longer available"
        );
    }
    assert!(
        state
            .pairing_runtime
            .take("remote", &request.signature)
            .is_none()
    );
    local.shutdown().await;
    remote.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn native_main_pairing_queries_are_read_only_and_reject_stale_request_identity() {
    let (_dir, server) = fixture("fixture");
    let native = server.native_graphql().unwrap();
    let response = native
        .execute(async_graphql::Request::new(
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
        .execute(async_graphql::Request::new(
            r#"mutation { cancelPairing(generation:"absent") }"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["cancelPairing"], false);
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn raw_ble_reports_are_validated_in_rust_before_platform_calls() {
    let (_directory, server) = fixture("local");
    let state = server.runtime_state();
    let (generation, mut calls) = state.host.connect();
    for (kind, payload) in [
        ("PAIRING_REQUEST_REPORTED", "{}"),
        (
            "PAIRING_REQUEST_REPORTED",
            r#"{"request":{},"address":"adapter"}"#,
        ),
        (
            "PAIRING_RESPONSE_REPORTED",
            r#"{"fromId":"peer","accepted":true}"#,
        ),
        ("PAIRING_CANCEL_REPORTED", r#"{"fromId":"peer"}"#),
        ("PAIRING_RESPONSE_REPORTED", "not-json"),
        ("PAIRING_CANCEL_REPORTED", "null"),
    ] {
        assert!(
            accept_report(&state, kind, payload).await.is_err(),
            "{kind}: {payload}"
        );
    }
    assert!(calls.try_recv().is_err());
    state.host.disconnect(generation);
    server.stop_core().await;
}

#[tokio::test]
async fn pairing_key_goes_only_to_the_host_and_is_redelivered_after_disconnect() {
    let (_directory, server) = fixture("local");
    let state = server.runtime_state();
    // Repeat reconnections to cover both ready-branch orders in select!.
    for _ in 0..256 {
        let mut events = state.events.subscribe();
        let (generation, mut requests) = state.host.connect();
        success(
            &state,
            &json!({"id":"peer","name":"Phone","key":"fixture-secret"}),
            "127.0.0.1",
        );
        let public = events.recv().await.unwrap();
        assert_eq!(
            public.event_type,
            plain_rs::chat::events::WS_PAIRING_SUCCESS
        );
        let payload: Value = serde_json::from_str(&public.payload).unwrap();
        assert!(payload.get("key").is_none());
        let request = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let request = requests.recv().await.unwrap();
                if request["method"] == "pairingSucceeded" {
                    break request;
                }
                state
                    .host
                    .reply(generation, json!({"id":request["id"],"result":null}))
                    .unwrap();
            }
        })
        .await
        .unwrap();
        assert_eq!(
            request["params"],
            json!({"deviceId":"peer","key":"fixture-secret"})
        );
        state.host.disconnect(generation);
        let (replacement, mut requests) = state.host.connect();
        let repeated = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let request = requests.recv().await.unwrap();
                if request["method"] == "pairingSucceeded" {
                    break request;
                }
                state
                    .host
                    .reply(replacement, json!({"id":request["id"],"result":null}))
                    .unwrap();
            }
        })
        .await
        .unwrap();
        assert_eq!(repeated["params"], request["params"]);
        state
            .host
            .reply(replacement, json!({"id":repeated["id"],"result":null}))
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if state.pairing_runtime.pending_success.lock().unwrap().requests.is_empty() {
                    break;
                }
                tokio::select! {
                    request = requests.recv() => {
                        let request = request.unwrap();
                        assert_ne!(
                            request["method"], "pairingSucceeded", "Acknowledged pairing callback was sent again"
                        );
                        state.host.reply(replacement, json!({"id":request["id"],"result":null})).unwrap();
                    },
                    _ = tokio::task::yield_now() => {},
                }
            }
        })
        .await
        .unwrap();
        state.host.disconnect(replacement);
    }
    server.stop_core().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn incoming_ble_message_succeeds_when_lan_address_is_invalid() {
    let (_directory, server) = fixture("local");
    let state = server.runtime_state();
    let request: PairingRequest = serde_json::from_value(json!({"fromId":"peer","fromName":"Fixture","port":2443,"deviceType":"PHONE","ecdhPublicKey":"","signaturePublicKey":"","timestamp":1,"ips":[],"signature":"request","awareSupported":false})).unwrap();
    state
        .pairing_runtime
        .remember(&request, "fixture-ble")
        .unwrap();
    let target = Target {
        device_id: "peer".into(),
        device_name: "Fixture".into(),
        device_ip: "invalid host".into(),
        device_port: 2443,
    };
    let (generation, mut calls) = state.host.connect();
    let host = state.host.clone();
    let reply = tokio::spawn(async move {
        let call = calls.recv().await.unwrap();
        assert_eq!(call["method"], "pairingNotification");
        assert_eq!(call["params"]["address"], "fixture-ble");
        host.reply(generation, json!({"id":call["id"],"result":true}))
            .unwrap();
    });
    send_incoming(
        &state,
        &request,
        &target,
        Message::PairRequest(request.clone()),
    )
    .await
    .unwrap();
    reply.await.unwrap();
    server.shutdown().await;
}
