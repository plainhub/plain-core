use super::*;
use crate::server::ContentServer;
use futures_util::{SinkExt, StreamExt};
use plain_libs::axum::body::Bytes;
use plain_libs::serde_json::json;
use plain_libs::tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use plain_rs::infrastructure::db::{DPeer, chat_store::peers};
use plain_rs::{
    features::{
        chat::enums::{DeviceType, PeerStatus},
        prefs::{Prefs, pref_keys},
        services::sessions,
    },
    infrastructure::{
        db::chat_store::SaveMode, protocols::websocket as graphql, transport::http_request,
    },
};
use std::time::Duration;

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
    a_state.management.shutdown().await;
    b_state.management.shutdown().await;
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
    let executor = a.native_graphql().unwrap();
    let response = executor
        .execute_peer_scoped(
            &b_id,
            plain_graphql::Request::new("{ app { clientId urlToken } }"),
            Default::default(),
            std::time::Duration::from_secs(30),
        )
        .await
        .unwrap();
    let data = plain_libs::serde_json::to_value(response).unwrap();
    assert_eq!(data["data"]["app"]["clientId"], b_id);
    assert_eq!(data["data"]["app"]["urlToken"], management.url_token);
    for caller in ["", browser.client_id.as_str(), a_id.as_str()] {
        let response = http
            .post(format!("http://127.0.0.1:{a_http}/proxy/{b_id}/graphql"))
            .header(header::ORIGIN, "tauri://localhost")
            .header("c-id", caller)
            .body("retired native GraphQL")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    let origin = format!("https://localhost:{a_port}");
    let proxy = format!("{origin}/proxy/{b_id}");
    for (method, path) in [
        (Method::GET, "/health"),
        (Method::POST, "/init"),
        (Method::POST, "/nearby"),
        (Method::PATCH, "/arbitrary/a%2Bb?raw=%2F+x"),
        (Method::DELETE, "/graphql"),
        (Method::PUT, "/fs"),
        (Method::GET, "/upload"),
        (Method::POST, "/auth"),
    ] {
        let proof = resource_link::encrypt(
            &Scope::Session(browser.client_id.clone()),
            &format!("relay:{b_id}:{method}:{path}"),
            &decode_key(&browser.url_token).unwrap(),
            &plain_libs::serde_json::to_vec(&plain_libs::chrono::Utc::now().timestamp_millis())
                .unwrap(),
        )
        .unwrap();
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        for relay in [
            format!("relay={proof}"),
            "relay=invalid".to_owned(),
            "".to_owned(),
        ] {
            let response = http
                .request(method.clone(), format!("{proxy}{path}?{relay}&{query}"))
                .header("c-id", &browser.client_id)
                .header(header::ORIGIN, &origin)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
        }
    }
    let key = decode_key(&browser.token).unwrap();
    let body =
        http_request::encrypt(&key, "/graphql", r#"{"query":"{ app { clientId } }"}"#).unwrap();
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
        http_request::decrypt_response(&key, &body, &response.bytes().await.unwrap()).unwrap();
    assert_eq!(
        plain_libs::serde_json::from_slice::<plain_libs::serde_json::Value>(&response).unwrap()["data"]
            ["app"]["clientId"],
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

    for path in ["auth", "unavailable-socket"] {
        let (mut socket, response) = plain_libs::tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{a_http}/proxy/{b_id}/{path}?cid={a_id}"
        ))
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
        if path == "auth" {
            socket.close(None).await.unwrap();
        } else {
            assert!(matches!(
                tokio::time::timeout(Duration::from_secs(5), socket.next())
                    .await
                    .unwrap(),
                Some(Ok(Message::Close(Some(frame))))
                    if frame.code == 1011u16.into() && frame.reason == "upstream unavailable"
            ));
        }
    }

    let secret = http_request::encrypt(
        &key,
        "/graphql",
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
        &plain_libs::serde_json::to_vec(&Resource::reference(file.to_str().unwrap())).unwrap(),
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
    let archive_dir = dir.path().join("archive");
    std::fs::create_dir(&archive_dir).unwrap();
    std::fs::write(archive_dir.join("entry.txt"), b"archive fixture").unwrap();
    let archive_id = resource_link::encrypt(
        &Scope::Session(browser.client_id.clone()),
        &format!("managed:{b_id}:zip/dir:download"),
        &decode_key(&browser.url_token).unwrap(),
        &plain_libs::serde_json::to_vec(&Resource::reference(archive_dir.to_str().unwrap()))
            .unwrap(),
    )
    .unwrap();
    let response = http
        .get(format!("{proxy}/zip/dir"))
        .query(&[("id", archive_id.as_str()), ("action", "download")])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/zip");
    let mut archive =
        plain_libs::zip::ZipArchive::new(std::io::Cursor::new(response.bytes().await.unwrap()))
            .unwrap();
    assert_eq!(archive.len(), 1);
    let mut entry = Vec::new();
    std::io::Read::read_to_end(&mut archive.by_name("entry.txt").unwrap(), &mut entry).unwrap();
    assert_eq!(entry, b"archive fixture");

    let data = b"managed upload fixture";
    let upload = |path, metadata: String| {
        let metadata = http_request::encrypt(&key, &format!("/{path}"), &metadata).unwrap();
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
        "/graphql",
        r#"{"query":"{ uploadedChunks(fileId: \"fixture\") }"}"#,
    )
    .unwrap();
    let response = post(chunks.clone()).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let plaintext =
        http_request::decrypt_response(&key, &chunks, &response.bytes().await.unwrap()).unwrap();
    assert_eq!(
        plain_libs::serde_json::from_slice::<plain_libs::serde_json::Value>(&plaintext).unwrap()["data"]
            ["uploadedChunks"],
        json!([format!("0:{}", data.len())])
    );
    let clean = http_request::encrypt(
        &key,
        "/graphql",
        r#"{"query":"mutation { deleteChunks(fileId: \"fixture\") }"}"#,
    )
    .unwrap();
    let response = post(clean.clone()).send().await.unwrap();
    let plaintext =
        http_request::decrypt_response(&key, &clean, &response.bytes().await.unwrap()).unwrap();
    assert_eq!(
        plain_libs::serde_json::from_slice::<plain_libs::serde_json::Value>(&plaintext).unwrap()["data"]
            ["deleteChunks"],
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
    assert!(b_state.subscription_sessions.snapshot().is_empty());
    for command in [
        json!({"type":"ping"}),
        json!({"type":"subscribe","id":"early","payload":{"query":"subscription { accessUpdated }"}}),
    ] {
        tokio::time::timeout(
            Duration::from_secs(12),
            rejected_subscription_confirmation(&socket, &browser.client_id, &key, pin, command),
        )
        .await
        .unwrap();
        assert!(b_state.subscription_sessions.snapshot().is_empty(), "invalid confirmation must not open an upstream subscription");
    }
    tokio::time::timeout(
        Duration::from_secs(12),
        subscription(&socket, &browser.client_id, &key, pin),
    )
    .await
    .unwrap();
    let scope = plain_rs::infrastructure::host::request_scope::NativeRequestScope::default();
    let handle = executor
        .open_managed_peer_subscriptions(&b_id, scope.clone())
        .await
        .unwrap();
    let session = executor.subscription_session(handle).unwrap();
    session.send(r#"{"type":"subscribe","id":"access","payload":{"query":"subscription { accessUpdated }"}}"#).unwrap();
    let ready = tokio::time::timeout(Duration::from_secs(12), session.read())
        .await
        .unwrap()
        .unwrap();
    let ready: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&ready).unwrap();
    assert_eq!(ready["payload"]["extensions"]["subscriptionReady"], true);
    scope.close();
    assert!(
        tokio::time::timeout(Duration::from_secs(2), session.read())
            .await
            .unwrap()
            .is_err()
    );
    executor.close_subscriptions(handle);
    for url in [
        format!("ws://127.0.0.1:{a_http}/proxy/{b_id}/graphql?cid={a_id}"),
        format!("ws://127.0.0.1:{a_http}/proxy/{b_id}/graphql"),
        format!(
            "ws://127.0.0.1:{a_http}/proxy/{b_id}/graphql?cid={}",
            browser.client_id
        ),
        format!("ws://127.0.0.1:{a_http}/proxy/unknown/graphql?cid={a_id}"),
    ] {
        assert!(
            plain_libs::tokio_tungstenite::connect_async(url)
                .await
                .is_err()
        );
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
        "/graphql",
        r#"{"query":"{ uploadedChunks(fileId: \"fixture\") }"}"#,
    )
    .unwrap();
    let response = post(chunks.clone()).send().await.unwrap();
    let plaintext =
        http_request::decrypt_response(&key, &chunks, &response.bytes().await.unwrap()).unwrap();
    assert_eq!(
        plain_libs::serde_json::from_slice::<plain_libs::serde_json::Value>(&plaintext).unwrap()["errors"]
            [0]["message"],
        "no_permission"
    );
    a.stop_core().await;
    b.stop_core().await;
}

#[tokio::test]
async fn first_login_and_chat_range_use_proxy_without_a_management_token() {
    use plain_rs::features::{prefs::security, services::peers::peer_wire};
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
    security::set_password_login(&b_state.prefs, "fixture-password").unwrap();
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
    let http = http_client::builder()
        .no_proxy()
        .add_root_certificate(http_client::Certificate::from_pem(&a_cert).unwrap())
        .build()
        .unwrap();
    let origin = format!("https://localhost:{a_port}");
    let socket_path = format!("/proxy/{b_id}/auth?cid={}", browser.client_id);
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
        plain_libs::tokio_tungstenite::connect_async_tls_with_config(
            format!("wss://localhost:{a_port}{socket_path}"),
            None,
            false,
            Some(plain_libs::tokio_tungstenite::Connector::Rustls(Arc::new(
                tls.clone(),
            ))),
        )
    };
    let session_key = decode_key(&browser.token).unwrap();
    let init_frame = |path: &str| {
        graphql::seal_init(
            &session_key,
            &browser.client_id,
            path,
            &plain_libs::serde_json::to_vec(&graphql::Init {
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
            init_frame(&socket_path.replace(&browser.client_id, "wrong-client")).into(),
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
        graphql::open_ack(
            &session_key,
            &browser.client_id,
            &socket_path,
            &registration,
            &ack
        )
        .is_some()
    );
    let mut binding = graphql::Stream::new(
        &socket_path,
        &browser.client_id,
        &registration,
        &ack,
        graphql::Role::Client,
    )
    .unwrap();
    let confirmation = binding.encode(&[], &session_key).unwrap();
    socket
        .send(Message::Binary(confirmation.clone().into()))
        .await
        .unwrap();
    let (mut replayed, _) = connect().await.unwrap();
    replayed
        .send(Message::Binary(registration.clone().into()))
        .await
        .unwrap();
    let Message::Binary(replay_ack) = replayed.next().await.unwrap().unwrap() else {
        panic!("Replayed init must get a fresh connection challenge");
    };
    assert_ne!(ack, replay_ack);
    replayed
        .send(Message::Binary(confirmation.into()))
        .await
        .unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), replayed.next())
            .await
            .unwrap(),
        Some(Ok(Message::Close(_)))
    ));
    let (mut reconnected, _) = connect().await.unwrap();
    reconnected
        .send(Message::Binary(registration.clone().into()))
        .await
        .unwrap();
    let Message::Binary(reconnect_ack) = reconnected.next().await.unwrap().unwrap() else {
        panic!("Reconnect must receive a fresh challenge");
    };
    let mut binding = graphql::Stream::new(
        &socket_path,
        &browser.client_id,
        &registration,
        &reconnect_ack,
        graphql::Role::Client,
    )
    .unwrap();
    reconnected
        .send(Message::Binary(
            binding.encode(&[], &session_key).unwrap().into(),
        ))
        .await
        .unwrap();
    socket.close(None).await.unwrap();
    let mut socket = reconnected;
    let password = b"fixture-password";
    let login =
        opaque_ke::ClientLogin::<plain_libs::auth::Suite>::start(&mut rand_core::OsRng, password)
            .unwrap();
    socket
        .send(Message::Binary(login.message.serialize().to_vec().into()))
        .await
        .unwrap();
    let Message::Binary(ke2) = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    else {
        panic!("OPAQUE KE2 required");
    };
    let identity = plain_libs::utils::base64::base64_decode(&peer.public_key);
    let context = plain_libs::auth::context(&browser.client_id, &identity);
    let ksf = plain_libs::auth::ksf();
    let finish = login
        .state
        .finish(
            &mut rand_core::OsRng,
            password,
            opaque_ke::CredentialResponse::<plain_libs::auth::Suite>::deserialize(&ke2).unwrap(),
            opaque_ke::ClientLoginFinishParameters::new(
                Some(&context),
                opaque_ke::Identifiers {
                    client: Some(plain_libs::auth::CLIENT_IDENTITY),
                    server: Some(&identity),
                },
                Some(&ksf),
            ),
        )
        .unwrap();
    let login_key = plain_libs::auth::api_key(&finish.session_key, &context);
    let request = json!({"appName":"Fixture", "appVersion":"1", "osName":"Fixture", "osVersion":"1"});
    let encrypted = plain_libs::crypto::xchacha_encrypt_raw_with_aad(
        &login_key,
        &plain_libs::serde_json::to_vec(&request).unwrap(),
        &plain_libs::auth::auth_aad(&context, false),
    )
    .unwrap();
    let mut frame = finish.message.serialize().to_vec();
    frame.extend(encrypted);
    socket.send(Message::Binary(frame.into())).await.unwrap();
    let Message::Binary(response) = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    else {
        panic!("Encrypted B login result required");
    };
    let plaintext = plain_libs::crypto::xchacha_decrypt_raw_with_aad(
        &login_key,
        &response,
        &plain_libs::auth::auth_aad(&context, true),
    )
    .unwrap();
    let response: plain_libs::serde_json::Value =
        plain_libs::serde_json::from_slice(&plaintext).unwrap();
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
    a_state.prefs.set("remote_access", false).unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .unwrap(),
        Some(Ok(Message::Close(Some(frame))))
            if frame.code == 1008u16.into() && frame.reason == "proxy_auth_failed"
    ));
    a_state.prefs.set("remote_access", true).unwrap();

    let file = dir.path().join("chat.txt");
    std::fs::write(&file, b"chat attachment").unwrap();
    let b_key = plain_rs::features::sessions::token_key(&b_state.native_url_token().unwrap())
        .unwrap()
        .to_vec();
    let file_id = resource_link::seal(
        &Scope::Session(b_state.native_client_id().unwrap()),
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
        &plain_rs::features::sessions::token_key(&a_state.native_url_token().unwrap()).unwrap(),
        &plain_libs::serde_json::to_vec(&json!({"base":format!("https://127.0.0.1:{b_port}")}))
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
        &plain_rs::features::sessions::token_key(&a_state.native_url_token().unwrap()).unwrap(),
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
    let local_id = resource_link::seal(
        &native_scope,
        &plain_rs::features::sessions::token_key(&a_state.native_url_token().unwrap()).unwrap(),
        &local_ref,
    )
    .unwrap();
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
        let router = plain_libs::axum::Router::new().fallback(
            |uri: plain_libs::axum::http::Uri, headers: HeaderMap, body: Bytes| async move {
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
        plain_libs::axum::serve(upstream, router).await.unwrap();
    });
    let scope = Scope::Session(state.native_client_id().unwrap());
    let ticket = resource_link::encrypt(
        &scope,
        "native-proxy",
        &plain_rs::features::sessions::token_key(&state.native_url_token().unwrap()).unwrap(),
        &plain_libs::serde_json::to_vec(&json!({"base":format!("http://{address}")})).unwrap(),
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
    assert!(
        plain_libs::tokio_tungstenite::connect_async(request)
            .await
            .is_err()
    );
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

async fn rejected_subscription_confirmation(
    url: &str,
    client: &str,
    key: &[u8],
    pin: [u8; 32],
    command: plain_libs::serde_json::Value,
) {
    let tls = http_client::pinned_tls_config(pin).unwrap();
    let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async_tls_with_config(
        url,
        None,
        false,
        Some(plain_libs::tokio_tungstenite::Connector::Rustls(Arc::new(tls))),
    )
    .await
    .unwrap();
    let init = graphql::seal_init(
        key,
        client,
        "/graphql",
        json!({"clientNonce":"ab".repeat(32)}).to_string().as_bytes(),
    )
    .unwrap();
    socket.send(Message::Binary(init.clone().into())).await.unwrap();
    let Message::Binary(ack) = socket.next().await.unwrap().unwrap() else {
        panic!("Missing challenge")
    };
    assert!(graphql::open_ack(key, client, "/graphql", &init, &ack).is_some());
    let mut cipher = graphql::Stream::new("/graphql", client, &init, &ack, graphql::Role::Client).unwrap();
    let frame = cipher.encode(command.to_string().as_bytes(), key).unwrap();
    assert_eq!(u64::from_be_bytes(frame[..8].try_into().unwrap()), 1);
    socket.send(Message::Binary(frame.into())).await.unwrap();
    let Message::Close(Some(close)) = socket.next().await.unwrap().unwrap() else {
        panic!("Nonempty confirmation must close the proxy connection")
    };
    assert_eq!(u16::from(close.code), 1008);
}

async fn subscription(url: &str, client: &str, key: &[u8], pin: [u8; 32]) {
    let request = url.into_client_request().unwrap();
    let tls = http_client::pinned_tls_config(pin).unwrap();
    let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async_tls_with_config(
        request,
        None,
        false,
        Some(plain_libs::tokio_tungstenite::Connector::Rustls(Arc::new(
            tls,
        ))),
    )
    .await
    .unwrap();
    let init = graphql::seal_init(
        key,
        client,
        "/graphql",
        &plain_libs::serde_json::to_vec(&graphql::Init {
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
    assert!(graphql::open_ack(key, client, "/graphql", &init, &ack).is_some());
    let mut cipher =
        graphql::Stream::new("/graphql", client, &init, &ack, graphql::Role::Client).unwrap();
    socket
        .send(Message::Binary(cipher.encode(&[], key).unwrap().into()))
        .await
        .unwrap();
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
        let message: plain_libs::serde_json::Value =
            plain_libs::serde_json::from_slice(&received).unwrap();
        assert_eq!(message["type"], expected, "{message}");
        if expected == "next" {
            assert_eq!(message["payload"]["extensions"]["subscriptionReady"], true);
        }
    }
    socket.close(None).await.unwrap();
}

#[tokio::test]
async fn auth_proxy_requires_fresh_confirmation_before_opening_upstream() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let core = start(dir.path(), "proxy-device");
    core.runtime_state().management.shutdown().await;
    let (cert, key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, port) = core.start_public(0, 0, cert, key).await.unwrap();
    let state = core.runtime_state();
    let browser = login(&state, "proxy-browser");
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = DPeer::new(
        "proxy-target",
        "Fixture",
        "127.0.0.1",
        Some(upstream.local_addr().unwrap().port()),
        DeviceType::Phone,
    );
    peers::save(&state.db, &[target], SaveMode::Insert).unwrap();
    let key = decode_key(&browser.token).unwrap();
    let path = format!("/proxy/proxy-target/auth?cid={}", browser.client_id);
    let pin = decode_pin(
        &state
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
        plain_libs::tokio_tungstenite::connect_async_tls_with_config(
            format!("wss://localhost:{port}{path}"),
            None,
            false,
            Some(plain_libs::tokio_tungstenite::Connector::Rustls(Arc::new(
                tls.clone(),
            ))),
        )
    };
    let init = graphql::seal_init(
        &key,
        &browser.client_id,
        &path,
        &plain_libs::serde_json::to_vec(&graphql::Init {
            client_nonce: "ab".repeat(32),
        })
        .unwrap(),
    )
    .unwrap();
    let (mut first, _) = connect().await.unwrap();
    first
        .send(Message::Binary(init.clone().into()))
        .await
        .unwrap();
    let Message::Binary(ack) = tokio::time::timeout(Duration::from_secs(2), first.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    else {
        panic!("Missing challenge before upstream connection");
    };
    let mut binding = graphql::Stream::new(
        &path,
        &browser.client_id,
        &init,
        &ack,
        graphql::Role::Client,
    )
    .unwrap();
    let old_confirmation = binding.encode(&[], &key).unwrap();
    first.close(None).await.unwrap();
    let (mut replayed, _) = connect().await.unwrap();
    replayed
        .send(Message::Binary(init.clone().into()))
        .await
        .unwrap();
    let Message::Binary(new_ack) = replayed.next().await.unwrap().unwrap() else {
        panic!("Missing fresh challenge")
    };
    assert_ne!(ack, new_ack);
    replayed
        .send(Message::Binary(old_confirmation.into()))
        .await
        .unwrap();
    assert!(matches!(replayed.next().await, Some(Ok(Message::Close(_)))));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), upstream.accept())
            .await
            .is_err(),
        "An init or stale confirmation must never open the upstream"
    );
    let (mut current, _) = connect().await.unwrap();
    current
        .send(Message::Binary(init.clone().into()))
        .await
        .unwrap();
    let Message::Binary(ack) = current.next().await.unwrap().unwrap() else {
        panic!("Missing reconnect challenge")
    };
    let mut binding = graphql::Stream::new(
        &path,
        &browser.client_id,
        &init,
        &ack,
        graphql::Role::Client,
    )
    .unwrap();
    current
        .send(Message::Binary(binding.encode(&[], &key).unwrap().into()))
        .await
        .unwrap();
    let (connection, _) = tokio::time::timeout(Duration::from_secs(2), upstream.accept())
        .await
        .unwrap()
        .unwrap();
    drop(connection);
    core.stop_core().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn idle_peer_subscription_serializes_concurrent_commands_over_tls() {
    use plain_rs::features::services::peers::subscriptions::PeerSubscription;
    use plain_rs::infrastructure::protocols::graphql::PeerSubscriptionTarget;
    let dir = plain_libs::temporary::tempdir().unwrap();
    let local = start(&dir.path().join("local"), "idle-local");
    let remote = start(&dir.path().join("remote"), "idle-remote");
    let local_state = local.runtime_state();
    let remote_state = remote.runtime_state();
    local_state.management.shutdown().await;
    remote_state.management.shutdown().await;
    let (cert, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, port) = remote.start_public(0, 0, cert, cert_key).await.unwrap();
    let client = local_state.native_client_id().unwrap();
    let remote_id = remote_state.native_client_id().unwrap();
    let login = login(&remote_state, &client);
    let mut peer = DPeer::new(
        &remote_id,
        "Fixture",
        "127.0.0.1",
        Some(port),
        DeviceType::Phone,
    );
    peer.status = PeerStatus::Paired;
    peer.public_key = plain_libs::utils::base64::base64_encode(&[7; 32]);
    peer.token = login.token;
    peers::save(&local_state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let pin = decode_pin(
        &remote_state
            .public_server
            .lock()
            .await
            .as_ref()
            .unwrap()
            .tls_certificate_sha256,
    )
    .unwrap();
    assert!(
        !remote_state
            .subscription_sessions
            .snapshot()
            .contains(&client)
    );
    let scope = plain_rs::infrastructure::host::request_scope::NativeRequestScope::default();
    let subscription = PeerSubscription::open_with_pin(
        local_state.core.clone(),
        PeerSubscriptionTarget {
            peer_id: remote_id,
            public_key: peer.public_key,
        },
        scope.clone(),
        Some(pin),
    )
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !remote_state
            .subscription_sessions
            .snapshot()
            .contains(&client)
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("Idle open must authenticate before any command, not wait for the heartbeat");
    subscription.send(r#"{"type":"subscribe","id":"access","payload":{"query":"subscription { accessUpdated }"}}"#).unwrap();
    let ready = tokio::time::timeout(Duration::from_secs(5), subscription.read())
        .await
        .unwrap()
        .unwrap();
    let ready: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&ready).unwrap();
    assert_eq!(ready["payload"]["extensions"]["subscriptionReady"], true);
    for batch in 0..4 {
        let ids = (0..16)
            .map(|index| format!("concurrent-{batch}-{index}"))
            .collect::<std::collections::HashSet<_>>();
        for operation in ["subscribe", "complete"] {
            let barrier = Arc::new(tokio::sync::Barrier::new(ids.len() + 1));
            let mut senders = tokio::task::JoinSet::new();
            for id in &ids {
                let subscription = subscription.clone();
                let barrier = barrier.clone();
                let message = if operation == "subscribe" {
                    json!({"type":"subscribe","id":id,"payload":{"query":"subscription { accessUpdated }"}})
                } else {
                    json!({"type":"complete","id":id})
                }
                .to_string();
                senders.spawn(async move {
                    barrier.wait().await;
                    subscription.send(&message).unwrap();
                });
            }
            barrier.wait().await;
            while let Some(result) = senders.join_next().await {
                result.unwrap();
            }
            if operation == "subscribe" {
                tokio::time::timeout(Duration::from_secs(10), async {
                    let mut remaining = ids.clone();
                    while !remaining.is_empty() {
                        let message: plain_libs::serde_json::Value =
                            plain_libs::serde_json::from_str(&subscription.read().await.unwrap())
                                .unwrap();
                        assert_eq!(message["type"], "next", "{message}");
                        assert_eq!(message["payload"]["extensions"]["subscriptionReady"], true);
                        assert!(
                            remaining.remove(message["id"].as_str().unwrap()),
                            "{message}"
                        );
                    }
                })
                .await
                .unwrap();
            }
        }
    }
    subscription.send(r#"{"type":"subscribe","id":"after-concurrent","payload":{"query":"subscription { accessUpdated }"}}"#).unwrap();
    let ready = tokio::time::timeout(Duration::from_secs(5), subscription.read())
        .await
        .unwrap()
        .unwrap();
    let ready: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&ready).unwrap();
    assert_eq!(ready["id"], "after-concurrent");
    assert_eq!(ready["payload"]["extensions"]["subscriptionReady"], true);
    scope.close();
    subscription.close();
    local.stop_core().await;
    remote.stop_core().await;
}
