use plain_rs::discovery::nearby_http::Message;
use plain_rs::peers::pairing::sessions::Target;
use plain_rs::services::discovery::pairing::Device;
use plain_rs::services::discovery::pairing_runtime::*;
use serde_json::json;

#[cfg(feature = "http_transport")]
fn fixture(id: &str) -> (tempfile::TempDir, crate::server::ContentServer) {
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
    let server = crate::server::ContentServer::start(
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
