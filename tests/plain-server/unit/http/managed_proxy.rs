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
    let (a_http, a_port) = a
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
    a_state
        .prefs
        .set(pref_keys::API_PERMISSIONS, Vec::<AccessFeature>::new())
        .unwrap();
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
    peer.token = management.token.clone();
    peers::save(&a_state.db, &[peer], SaveMode::Insert).unwrap();
    let http = http_client::builder()
        .no_proxy()
        .add_root_certificate(http_client::Certificate::from_pem(&cert).unwrap())
        .build()
        .unwrap();
    let native_key = decode_key(&management.token).unwrap();
    let native_body =
        http_request::encrypt(&native_key, r#"{"query":"{ app { clientId urlToken } }"}"#).unwrap();
    let native_url = format!("http://127.0.0.1:{a_http}/proxy/{b_id}/graphql");
    let request = || {
        http.post(&native_url)
            .header(header::ORIGIN, "tauri://localhost")
            .body(native_body.clone())
    };
    assert_eq!(
        request().send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request()
            .header("c-id", &browser.client_id)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = request().header("c-id", &a_id).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let decoded = http_request::decrypt_response(
        &native_key,
        "/graphql",
        &native_body,
        &response.bytes().await.unwrap(),
    )
    .unwrap();
    let data: serde_json::Value = serde_json::from_slice(&decoded).unwrap();
    assert_eq!(data["data"]["app"]["clientId"], b_id);
    assert_eq!(data["data"]["app"]["urlToken"], management.url_token);
    assert_eq!(
        http.post(format!("http://127.0.0.1:{a_http}/proxy/unknown/graphql"))
            .header("c-id", &a_id)
            .body(native_body)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    let origin = format!("https://localhost:{a_port}");
    let proxy = format!("{origin}/proxy/{b_id}");
    let path = "/health";
    let proof = resource_link::encrypt(
        &Scope::Session(browser.client_id.clone()),
        &format!("relay:{b_id}:GET:{path}"),
        &decode_key(&browser.url_token).unwrap(),
        &serde_json::to_vec(&chrono::Utc::now().timestamp_millis()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        http.get(format!("{proxy}{path}?relay={proof}"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
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
    tokio::time::timeout(
        Duration::from_secs(12),
        subscription(
            &format!("ws://127.0.0.1:{a_http}/proxy/{b_id}/graphql?cid={a_id}"),
            &a_id,
            &native_key,
            pin,
        ),
    )
    .await
    .unwrap();
    for url in [
        format!("ws://127.0.0.1:{a_http}/proxy/{b_id}/graphql"),
        format!(
            "ws://127.0.0.1:{a_http}/proxy/{b_id}/graphql?cid={}",
            browser.client_id
        ),
        format!("ws://127.0.0.1:{a_http}/proxy/unknown/graphql?cid={a_id}"),
    ] {
        assert!(tokio_tungstenite::connect_async(url).await.is_err());
    }
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
    let response = post(chunks.clone()).send().await.unwrap();
    let plaintext =
        http_request::decrypt_response(&key, "/graphql", &chunks, &response.bytes().await.unwrap())
            .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&plaintext).unwrap()["errors"][0]["message"],
        "no_permission"
    );
    a.stop_core().await;
    b.stop_core().await;
}

#[tokio::test]
async fn first_login_and_chat_range_use_proxy_without_a_management_token() {
    use plain_rs::features::{
        prefs::security,
        services::{
            confirmation_channel::{Client, ConfirmationMode},
            peers::peer_wire,
        },
    };
    let dir = plain_libs::temporary::tempdir().unwrap();
    let a = start(&dir.path().join("a"), "device-a");
    let b = start(&dir.path().join("b"), "device-b");
    let (a_cert, a_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (a_http, a_port) = a.start_public(0, 0, a_cert.clone(), a_key).await.unwrap();
    let (b_cert, b_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, b_port) = b.start_public(0, 0, b_cert, b_key).await.unwrap();
    let a_state = a.runtime_state();
    let b_state = b.runtime_state();
    let b_id = b_state.native_client_id().unwrap();
    let a_id = a_state.native_client_id().unwrap();
    let browser = login(&a_state, "browser");
    security::configure_password_login(&b_state.prefs).unwrap();
    security::setup_password(&b_state.prefs, "fixture-password").unwrap();
    let pair = peer_wire::signing_keypair(&b_state.prefs).unwrap();
    let mut peer = DPeer::new(
        &b_id,
        "Fixture B",
        "127.0.0.1",
        Some(b_port),
        DeviceType::Phone,
    );
    peer.status = PeerStatus::Paired;
    peer.public_key = plain_libs::utils::base64::base64_encode(&pair[32..]);
    peers::save(&a_state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let key = decode_key(&browser.url_token).unwrap();
    let scope = Scope::Session(browser.client_id.clone());
    let init_proof = resource_link::encrypt(
        &scope,
        &format!("relay:{b_id}:POST:/init"),
        &key,
        &serde_json::to_vec(&chrono::Utc::now().timestamp_millis()).unwrap(),
    )
    .unwrap();
    let http = http_client::builder()
        .no_proxy()
        .add_root_certificate(http_client::Certificate::from_pem(&a_cert).unwrap())
        .build()
        .unwrap();
    let origin = format!("https://localhost:{a_port}");
    let response = http
        .post(format!("{origin}/proxy/{b_id}/init?relay={init_proof}"))
        .header("c-id", &browser.client_id)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let init: serde_json::Value = serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();
    assert_eq!(init["signaturePublicKey"], peer.public_key);
    assert_eq!(init["authenticated"], false);

    let socket_path = format!("/proxy/{b_id}/auth?cid={}&auth=1", browser.client_id);
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
    let tls = http_client::pinned_tls_config(pin).unwrap();
    let connect = || {
        tokio_tungstenite::connect_async_tls_with_config(
            format!("wss://localhost:{a_port}{socket_path}"),
            None,
            false,
            Some(tokio_tungstenite::Connector::Rustls(Arc::new(tls.clone()))),
        )
    };
    let session_key = decode_key(&browser.token).unwrap();
    let init_frame = |path: &str| {
        graphql::seal_init_for(
            &session_key,
            &browser.client_id,
            path,
            &serde_json::to_vec(&graphql::Init {
                timestamp_millis: chrono::Utc::now().timestamp_millis(),
                client_nonce: plain_libs::utils::hex::bytes_to_hex(
                    &plain_libs::crypto::random_bytes(32),
                ),
            })
            .unwrap(),
        )
        .unwrap()
    };
    let (mut rejected, _) = connect().await.unwrap();
    rejected
        .send(Message::Binary(
            init_frame(&socket_path.replace("auth=1", "auth=2")).into(),
        ))
        .await
        .unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), rejected.next())
            .await
            .unwrap(),
        Some(Ok(Message::Close(_)))
    ));
    assert!(
        b_state
            .db
            .session_get(&browser.client_id)
            .unwrap()
            .is_none()
    );

    let (mut socket, _) = connect().await.unwrap();
    let registration = init_frame(&socket_path);
    socket
        .send(Message::Binary(registration.clone().into()))
        .await
        .unwrap();
    let Message::Binary(ack) = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    else {
        panic!("A must acknowledge the session before forwarding B frames");
    };
    assert!(
        graphql::open_ack_for(
            &session_key,
            &browser.client_id,
            &socket_path,
            &registration,
            &ack
        )
        .is_some()
    );
    let (mut replayed, _) = connect().await.unwrap();
    replayed
        .send(Message::Binary(registration.into()))
        .await
        .unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), replayed.next())
            .await
            .unwrap(),
        Some(Ok(Message::Close(_)))
    ));
    let (client, hello) = Client::start(&browser.client_id, ConfirmationMode::Password).unwrap();
    socket
        .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
        .await
        .unwrap();
    let Message::Text(hello) = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    else {
        panic!("Signed B hello required");
    };
    let now = || chrono::Utc::now().timestamp_millis() as u64;
    let (channel, reveal) = client
        .accept(
            &serde_json::from_str(&hello).unwrap(),
            &peer.public_key,
            &b_id,
            now(),
        )
        .unwrap();
    socket
        .send(Message::Text(
            serde_json::to_string(&reveal).unwrap().into(),
        ))
        .await
        .unwrap();
    let ecdh = plain_libs::crypto::EcdhSession::generate();
    let request = json!({
        "password":plain_libs::crypto::sha512_hex("fixture-password"), "loginContext":channel.context,
        "appName":"Fixture", "appVersion":"1", "osName":"Fixture", "osVersion":"1", "isMobile":false,
        "ecdhPublicKey":plain_libs::utils::base64::base64_encode(&ecdh.public_key_bytes),
    });
    let encrypted = channel
        .encrypt_request(&serde_json::to_vec(&request).unwrap(), now())
        .unwrap();
    socket
        .send(Message::Binary(encrypted.into()))
        .await
        .unwrap();
    let Message::Binary(response) = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    else {
        panic!("Encrypted B login result required");
    };
    let plaintext = channel.decrypt_response(&response, now()).unwrap();
    let response: serde_json::Value = serde_json::from_slice(&plaintext).unwrap();
    assert_eq!(response["status"], "COMPLETED", "{response}");
    assert!(
        b_state
            .db
            .session_get(&browser.client_id)
            .unwrap()
            .is_some()
    );
    assert!(a_state.db.session_get("browser").unwrap().is_some());
    assert!(
        peers::get(&a_state.db, &b_id)
            .unwrap()
            .unwrap()
            .token
            .is_empty()
    );
    socket.close(None).await.unwrap();

    let file = dir.path().join("chat.txt");
    std::fs::write(&file, b"chat attachment").unwrap();
    let b_key = decode_key(
        &b_state
            .prefs
            .get::<String>(pref_keys::URL_TOKEN)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let file_id = resource_link::seal(
        &Scope::Global,
        &b_key,
        &Resource::reference(file.to_str().unwrap()),
    )
    .unwrap();
    let reference = format!("fsid:{file_id}");
    let attachment = resource_link::encrypt(
        &scope,
        &format!("chat-attachment:{b_id}"),
        &key,
        reference.as_bytes(),
    )
    .unwrap();
    let url = format!("{origin}/proxy/{b_id}/fs?id={attachment}&action=preview");
    let wrong_peer = url.replace(&format!("/proxy/{b_id}/"), "/proxy/other-device/");
    assert_eq!(
        http.get(wrong_peer).send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.get(url.replace("?id=", "?attachment="))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let response = http
        .get(&url)
        .header(header::RANGE, "bytes=0-3")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(&response.bytes().await.unwrap()[..], b"chat");

    let native_scope = Scope::Session(a_id.clone());
    let ticket = resource_link::encrypt(
        &native_scope,
        "native-proxy",
        &a_state.native_file_url_key,
        &serde_json::to_vec(&json!({"base":format!("https://127.0.0.1:{b_port}")}))
            .unwrap(),
    )
    .unwrap();
    let native_base = format!("http://127.0.0.1:{a_http}/proxy/{ticket}");
    let response = http
        .get(format!("{native_base}/fs?id={file_id}&action=preview"))
        .header(header::ORIGIN, "tauri://localhost")
        .header(header::RANGE, "bytes=0-3")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "tauri://localhost"
    );
    assert_eq!(&response.bytes().await.unwrap()[..], b"chat");
    assert_eq!(
        http.get(format!("{native_base}/health"))
            .header(header::ORIGIN, "https://other.example.test")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    let native_attachment = resource_link::encrypt(
        &native_scope,
        &format!("chat-attachment:{b_id}"),
        &a_state.native_file_url_key,
        reference.as_bytes(),
    )
    .unwrap();
    let response = http
        .get(format!(
            "http://localhost:{a_http}/proxy/{b_id}/fs?id={native_attachment}&action=preview"
        ))
        .header(header::ORIGIN, "tauri://localhost")
        .header(header::RANGE, "bytes=0-3")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(&response.bytes().await.unwrap()[..], b"chat");

    let store = plain_rs::features::files::app_files::FileStore::new(
        a_state.db.clone(),
        a_state.directory.clone(),
    );
    let imported = store
        .import_file(&file, "chat.txt", "text/plain", false)
        .unwrap();
    let local_ref = Resource::reference(&format!("fid:{}", imported.id));
    let local_id =
        resource_link::seal(&native_scope, &a_state.native_file_url_key, &local_ref).unwrap();
    let response = http
        .get(format!(
            "http://localhost:{a_http}/fs?id={local_id}&action=download"
        ))
        .header(header::ORIGIN, "tauri://localhost")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(&response.bytes().await.unwrap()[..], b"chat attachment");

    let response = http
        .get(url.replace("action=preview", "action=download"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(&response.bytes().await.unwrap()[..], b"chat attachment");

    let mut request =
        format!("ws://127.0.0.1:{a_http}/proxy/{ticket}/auth?cid=native-webview&auth=1")
            .into_client_request()
            .unwrap();
    request
        .headers_mut()
        .insert(header::ORIGIN, "tauri://localhost".parse().unwrap());
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let (_, hello) = Client::start("native-webview", ConfirmationMode::Password).unwrap();
    socket
        .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
        .await
        .unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .unwrap(),
        Some(Ok(Message::Text(_)))
    ));
    socket.close(None).await.unwrap();

    a_state.db.session_delete(&browser.client_id).unwrap();
    assert_eq!(
        http.get(&url).send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    a.stop_core().await;
    b.stop_core().await;
}

#[tokio::test]
async fn native_proxy_streams_rest_through_the_main_listener() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let a = start(&dir.path().join("a"), "device-a");
    let (cert, key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (port, _) = a.start_public(0, 0, cert, key).await.unwrap();
    let state = a.runtime_state();
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = upstream.local_addr().unwrap();
    let serve = tokio::spawn(async move {
        let router = axum::Router::new().fallback(
            |uri: axum::http::Uri, headers: HeaderMap, body: Bytes| async move {
                if uri.path() == "/auth" {
                    return StatusCode::BAD_REQUEST.into_response();
                }
                assert_eq!(
                    uri.path_and_query().unwrap().as_str(),
                    "/arbitrary/a%2Bb?raw=%2F+x"
                );
                assert!(!headers.contains_key(header::ORIGIN));
                assert_eq!(headers["c-id"], "native-device");
                assert_eq!(&body[..], b"chunk one chunk two");
                (
                    [
                        ("cache-control", "private, max-age=60"),
                        ("set-cookie", "fixture=1"),
                        ("clear-site-data", "\"cookies\""),
                    ],
                    body,
                )
                    .into_response()
            },
        );
        axum::serve(upstream, router).await.unwrap();
    });
    let scope = Scope::Session(state.native_client_id().unwrap());
    let ticket = resource_link::encrypt(
        &scope,
        "native-proxy",
        &state.native_file_url_key,
        &serde_json::to_vec(&json!({"base":format!("http://{address}")})).unwrap(),
    )
    .unwrap();
    let http = http_client::builder().no_proxy().build().unwrap();
    let url = format!("http://localhost:{port}/proxy/{ticket}/arbitrary/a%2Bb?raw=%2F+x");
    let body = http_client::Body::wrap_stream(futures_util::stream::iter([
        Ok::<_, std::io::Error>(Bytes::from_static(b"chunk one ")),
        Ok(Bytes::from_static(b"chunk two")),
    ]));
    let response = http
        .request(Method::PATCH, &url)
        .header(header::ORIGIN, "tauri://localhost")
        .header("c-id", "native-device")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "private, max-age=60"
    );
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "tauri://localhost"
    );
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert!(!response.headers().contains_key("clear-site-data"));
    assert_eq!(&response.bytes().await.unwrap()[..], b"chunk one chunk two");
    let response = http
        .get(format!(
            "http://localhost:{port}/proxy/c.device-a.invalid/health"
        ))
        .header(header::ORIGIN, "tauri://localhost")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let mut request = format!("ws://localhost:{port}/proxy/{ticket}/auth")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert(header::ORIGIN, "tauri://localhost".parse().unwrap());
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let Message::Close(Some(frame)) = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    else {
        panic!("Expected close frame")
    };
    assert_eq!(u16::from(frame.code), 1011);
    serve.abort();
    a.stop_core().await;
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
