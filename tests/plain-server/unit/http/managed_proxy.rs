use super::*;
use crate::server::ContentServer;
use axum::body::Bytes;
use futures_util::{SinkExt, StreamExt};
use plain_rs::{
    features::{
        chat::enums::{DeviceType, PeerStatus},
        prefs::{Prefs, pref_keys},
        services::sessions,
    },
    infrastructure::{db::chat_store::SaveMode, protocols::graphql, transport::http_request},
};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

fn start(path: &std::path::Path, id: &str) -> ContentServer {
    std::fs::create_dir_all(path).unwrap();
    let prefs = Arc::new(Prefs::load(&path.join("prefs.json")).unwrap());
    prefs
        .set(pref_keys::API_PERMISSIONS, [AccessFeature::Files])
        .unwrap();
    prefs.set("client_id", id).unwrap();
    ContentServer::start(path, &plain_libs::crypto::gen_token(), prefs).unwrap()
}

fn login(state: &ServerState, id: &str) -> SessionRow {
    let mut row = sessions::create(state, id.into()).unwrap();
    row.client_id = id.into();
    row.r#type = plain_rs::features::sessions::SessionType::Login;
    state.db.session_save(&row).unwrap();
    row
}

#[tokio::test]
async fn managed_proxy_keeps_the_browser_session_and_streams_a_peer_range() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let a = start(&dir.path().join("a"), "device-a");
    let b = start(&dir.path().join("b"), "device-b");
    let (cert, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, a_port) = a
        .start_public(0, 0, cert.clone(), cert_key.clone())
        .await
        .unwrap();
    let (b_cert, b_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, b_port) = b.start_public(0, 0, b_cert, b_key).await.unwrap();
    let a_state = a.runtime_state();
    let b_state = b.runtime_state();
    let a_id = a_state.native_client_id().unwrap();
    let b_id = b_state.native_client_id().unwrap();
    let browser = login(&a_state, "browser");
    let management = login(&b_state, &a_id);
    let mut peer = DPeer::new(
        &b_id,
        "Archive",
        "127.0.0.1",
        Some(b_port),
        DeviceType::Phone,
    );
    peer.status = PeerStatus::Paired;
    peer.public_key = plain_libs::utils::base64::base64_encode(&[7; 32]);
    peer.token = management.token;
    peers::save(&a_state.db, &[peer], SaveMode::Insert).unwrap();
    let http = http_client::builder()
        .no_proxy()
        .add_root_certificate(http_client::Certificate::from_pem(&cert).unwrap())
        .build()
        .unwrap();
    let origin = format!("https://localhost:{a_port}");
    let proxy = format!("{origin}/proxy/{b_id}");
    let key = decode_key(&browser.token).unwrap();
    let body = http_request::encrypt(&key, r#"{"query":"{ app { clientId } }"}"#).unwrap();
    let response = http
        .post(format!("{proxy}/graphql"))
        .header("c-id", &browser.client_id)
        .header(header::ORIGIN, &origin)
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response =
        http_request::decrypt_response(&key, "/graphql", &body, &response.bytes().await.unwrap())
            .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&response).unwrap()["data"]["app"]["clientId"],
        b_id
    );
    let first = a_state.managed_proxy.route(&a_state, &b_id).await.unwrap();
    let second = a_state.managed_proxy.route(&a_state, &b_id).await.unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert!(first.supports_managed_resources);
    assert!(
        first
            .client
            .get(format!("{origin}/health"))
            .send()
            .await
            .is_err()
    );

    let secret = http_request::encrypt(
        &key,
        r#"{"query":"{ app { ...Fields } } fragment Fields on App { alias: urlToken }"}"#,
    )
    .unwrap();
    let response = http
        .post(format!("{proxy}/graphql"))
        .header("c-id", &browser.client_id)
        .body(secret)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let file = dir.path().join("range.bin");
    std::fs::write(&file, b"0123456789").unwrap();
    let id = resource_link::encrypt(
        &Scope::Session(browser.client_id.clone()),
        &format!("managed:{b_id}:fs:preview"),
        &decode_key(&browser.url_token).unwrap(),
        &serde_json::to_vec(&Resource::reference(file.to_str().unwrap())).unwrap(),
    )
    .unwrap();
    let get = |action| {
        http.get(format!("{proxy}/fs"))
            .query(&[("id", id.as_str()), ("action", action)])
    };
    let response = get("preview")
        .header(header::RANGE, "bytes=2-5")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 2-5/10");
    assert!(!response.headers().contains_key(MANAGED_RESOURCE));
    assert_eq!(&response.bytes().await.unwrap()[..], b"2345");
    assert_eq!(
        get("download").send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get("preview")
            .header(header::ORIGIN, "https://other.example.test")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let data = b"managed upload fixture";
    let upload = |path, metadata: String| {
        let metadata = http_request::encrypt(&key, &metadata).unwrap();
        http.post(format!("{proxy}/{path}"))
            .header("c-id", &browser.client_id)
            .header(
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=browser",
            )
            .body(upload_body(&metadata, data))
    };
    let response = upload(
        "upload",
        json!({"dir": dir.path(), "size": data.len()}).to_string(),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        std::fs::read(dir.path().join("uploaded.txt")).unwrap(),
        data
    );
    let response = upload(
        "upload_chunk",
        json!({"fileId":"fixture", "index":0, "size":data.len()}).to_string(),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let post = |body| {
        http.post(format!("{proxy}/graphql"))
            .header("c-id", &browser.client_id)
            .body(body)
    };
    let chunks = http_request::encrypt(
        &key,
        r#"{"query":"{ uploadedChunks(fileId: \"fixture\") }"}"#,
    )
    .unwrap();
    let response = post(chunks.clone()).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let plaintext =
        http_request::decrypt_response(&key, "/graphql", &chunks, &response.bytes().await.unwrap())
            .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&plaintext).unwrap()["data"]["uploadedChunks"],
        json!([format!("0:{}", data.len())])
    );
    let clean = http_request::encrypt(
        &key,
        r#"{"query":"mutation { deleteChunks(fileId: \"fixture\") }"}"#,
    )
    .unwrap();
    let response = post(clean.clone()).send().await.unwrap();
    let plaintext =
        http_request::decrypt_response(&key, "/graphql", &clean, &response.bytes().await.unwrap())
            .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&plaintext).unwrap()["data"]["deleteChunks"],
        true
    );

    let pin = decode_pin(
        &a_state
            .public_server
            .lock()
            .await
            .as_ref()
            .unwrap()
            .tls_certificate_sha256,
    )
    .unwrap();
    let socket = format!(
        "wss://localhost:{a_port}/proxy/{b_id}/graphql?cid={}",
        browser.client_id
    );
    tokio::time::timeout(
        Duration::from_secs(12),
        subscription(&socket, &browser.client_id, &key, pin),
    )
    .await
    .unwrap();
    b_state
        .prefs
        .set(pref_keys::API_PERMISSIONS, Vec::<AccessFeature>::new())
        .unwrap();
    assert_eq!(
        get("preview").send().await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let chunks = http_request::encrypt(
        &key,
        r#"{"query":"{ uploadedChunks(fileId: \"fixture\") }"}"#,
    )
    .unwrap();
    assert_eq!(
        post(chunks).send().await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    a.stop_core().await;
    b.stop_core().await;
}

fn upload_body(metadata: &[u8], data: &[u8]) -> http_client::Body {
    let mut multipart =
        b"--browser\r\nContent-Disposition: form-data; name=\"info\"\r\n\r\n".to_vec();
    multipart.extend_from_slice(metadata);
    multipart.extend_from_slice(b"\r\n--browser\r\nContent-Disposition: form-data; name=\"file\"; filename=\"uploaded.txt\"\r\n\r\n");
    multipart.extend_from_slice(data);
    multipart.extend_from_slice(b"\r\n--browser--\r\n");
    let chunks = multipart
        .chunks(7)
        .map(Bytes::copy_from_slice)
        .collect::<Vec<_>>();
    http_client::Body::wrap_stream(futures_util::stream::iter(
        chunks.into_iter().map(Ok::<_, std::io::Error>),
    ))
}

async fn subscription(url: &str, client: &str, key: &[u8], pin: [u8; 32]) {
    let mut request = url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert("sec-websocket-protocol", graphql::PROTOCOL.parse().unwrap());
    let tls = http_client::pinned_tls_config(pin).unwrap();
    let (mut socket, _) = tokio_tungstenite::connect_async_tls_with_config(
        request,
        None,
        false,
        Some(tokio_tungstenite::Connector::Rustls(Arc::new(tls))),
    )
    .await
    .unwrap();
    let init = graphql::seal_init(
        key,
        client,
        &serde_json::to_vec(&graphql::Init {
            timestamp_millis: chrono::Utc::now().timestamp_millis(),
            client_nonce: "ab".repeat(32),
        })
        .unwrap(),
    )
    .unwrap();
    socket
        .send(Message::Binary(init.clone().into()))
        .await
        .unwrap();
    let Message::Binary(ack) = socket.next().await.unwrap().unwrap() else {
        panic!("Missing acknowledgement")
    };
    assert!(graphql::open_ack(key, client, &init, &ack).is_some());
    let mut cipher = graphql::Stream::new(client, &init, &ack, graphql::Role::Client).unwrap();
    for (message, expected) in [
        (json!({"type":"ping"}), "pong"),
        (
            json!({"type":"subscribe","id":"access","payload":{"query":"subscription { accessUpdated }"}}),
            "next",
        ),
    ] {
        let frame = cipher.encode(message.to_string().as_bytes(), key).unwrap();
        socket.send(Message::Binary(frame.into())).await.unwrap();
        let Message::Binary(frame) = socket.next().await.unwrap().unwrap() else {
            panic!("Missing subscription frame")
        };
        let received = cipher.decode(&frame, key).unwrap();
        let message: serde_json::Value = serde_json::from_slice(&received).unwrap();
        assert_eq!(message["type"], expected, "{message}");
        if expected == "next" {
            assert_eq!(message["payload"]["extensions"]["subscriptionReady"], true);
        }
    }
    socket.close(None).await.unwrap();
}
