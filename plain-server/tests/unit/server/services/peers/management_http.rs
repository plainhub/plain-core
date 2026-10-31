use crate::server::ContentServer;
use axum::{Router, body::Bytes, http::HeaderMap, routing::post};
use plain_rs::services::peers::management_http::*;
use serde_json::json;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn a_captured_response_cannot_answer_a_fresh_encrypted_request() {
    let remote_dir = tempfile::tempdir().unwrap();
    let remote = ContentServer::open(remote_dir.path()).unwrap();
    let remote_state = remote.runtime_state().core;
    remote_state.prefs.set("device_name", "First name").unwrap();
    let session = remote
        .native_management()
        .unwrap()
        .create_session("Response fixture".into())
        .unwrap();
    remote_state.prefs.set("http_port", 0).unwrap();
    remote_state.prefs.set("https_port", 0).unwrap();
    let (_, port) = remote.start_configured_public().await.unwrap();
    let client = remote_state.lan.client.clone();
    let captured = Arc::new(Mutex::new(None::<Vec<u8>>));
    let requests = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let (saved, observed) = (captured.clone(), requests.clone());
    let router = Router::new().route(
        "/graphql",
        post(move |headers: HeaderMap, body: Bytes| {
            let client = client.clone();
            let saved = saved.clone();
            let observed = observed.clone();
            async move {
                assert!(!headers.contains_key("authorization"));
                observed.lock().unwrap().push(body.to_vec());
                let response = client
                    .post(format!("https://127.0.0.1:{port}/graphql"))
                    .header("c-id", headers.get("c-id").unwrap())
                    .header("content-type", "application/octet-stream")
                    .body(body)
                    .send()
                    .await
                    .unwrap();
                assert!(response.status().is_success());
                let bytes = response.bytes().await.unwrap().to_vec();
                let mut captured = saved.lock().unwrap();
                captured.get_or_insert(bytes).clone()
            }
        }),
    );
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let config = axum_server::tls_rustls::RustlsConfig::from_pem(
        cert.cert.pem().into_bytes(),
        cert.key_pair.serialize_pem().into_bytes(),
    )
    .await
    .unwrap();
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_port = socket.local_addr().unwrap().port();
    let proxy = tokio::spawn(async move {
        axum_server::from_tcp_rustls(socket, config)
            .serve(router.into_make_service())
            .await
            .unwrap();
    });
    let local_dir = tempfile::tempdir().unwrap();
    let local = ContentServer::open(local_dir.path()).unwrap();
    let key = plain_rs::utils::base64::base64_decode(&session.token);
    let url = reqwest::Url::parse(&format!("https://127.0.0.1:{proxy_port}")).unwrap();
    let request = json!({"query":"{ app { deviceName } }"});
    let first = query(
        &local.runtime_state().core,
        &url,
        &session.client_id,
        &key,
        request.clone(),
        65536,
    )
    .await
    .unwrap();
    assert_eq!(first["app"]["deviceName"], "First name");
    remote_state
        .prefs
        .set("device_name", "Second name")
        .unwrap();
    let replayed = query(
        &local.runtime_state().core,
        &url,
        &session.client_id,
        &key,
        request.clone(),
        65536,
    )
    .await;
    let direct = reqwest::Url::parse(&format!("https://127.0.0.1:{port}")).unwrap();
    let fresh = query(
        &local.runtime_state().core,
        &direct,
        &session.client_id,
        &key,
        request,
        65536,
    )
    .await
    .unwrap();
    assert_eq!(fresh["app"]["deviceName"], "Second name");
    {
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_ne!(requests[0], requests[1]);
        let envelopes: Vec<_> = requests
            .iter()
            .map(|body| {
                String::from_utf8(plain_rs::crypto::xchacha_decrypt_raw(&key, body).unwrap())
                    .unwrap()
            })
            .collect();
        let nonces: Vec<_> = envelopes
            .iter()
            .map(|body| body.splitn(3, '|').nth(1).unwrap())
            .collect();
        assert_ne!(nonces[0], nonces[1]);
        for nonce in nonces {
            assert!(uuid::Uuid::parse_str(nonce).is_ok());
        }
    }
    proxy.abort();
    let _ = proxy.await;
    local.stop_core().await;
    remote.stop_core().await;
    assert!(
        replayed.is_err(),
        "A captured response was accepted for a fresh request: {replayed:?}"
    );
}
