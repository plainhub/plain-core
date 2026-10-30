use super::*;

pub(super) fn root() -> (tempfile::TempDir, ContentServer, String) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "local").unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[4; 32]);
    let server = ContentServer::start(dir.path(), &token, prefs).unwrap();
    (dir, server, token)
}
pub(super) fn card(port: u16) -> Card {
    serde_json::from_value(json!({"shareId":"fixture","urlToken":plain_rs::utils::base64::base64_encode_url_safe(&[7;32]),"peerInfo":{"id":"peer","ip":"127.0.0.1","port":port},"name":"old","itemCount":2,"totalSize":8,"expiresAt":"2030-01-01T00:00:00.000Z"})).unwrap()
}
pub(super) fn save_card(state: &ServerState, card: &Card) -> DChat {
    let row = DChat::new(
        "peer",
        "me",
        "",
        &json!({"type":"SHARE","value":card}).to_string(),
    );
    messages::save(&state.db, &[row.clone()], SaveMode::Insert).unwrap();
    row
}
pub(super) async fn tls() -> (
    u16,
    tokio::task::JoinHandle<()>,
    Arc<AtomicUsize>,
    Arc<tokio::sync::Notify>,
    Arc<tokio::sync::Notify>,
) {
    use axum::{
        Router,
        body::Bytes,
        extract::Query,
        http::HeaderMap,
        routing::{get, post},
    };
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let config = axum_server::tls_rustls::RustlsConfig::from_pem(
        cert.cert.pem().into_bytes(),
        cert.key_pair.serialize_pem().into_bytes(),
    )
    .await
    .unwrap();
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    let count = Arc::new(AtomicUsize::new(0));
    let requested = Arc::new(tokio::sync::Notify::new());
    let released = Arc::new(tokio::sync::Notify::new());
    let seen = count.clone();
    let started = requested.clone();
    let release = released.clone();
    let router = Router::new().route("/guest_graphql", post(move |headers: HeaderMap, body: Bytes| {
        let count = seen.clone();
        let started = started.clone();
        let release = release.clone();
        async move {
            assert_eq!(headers["c-id"], "fixture");
            let plain = plain_rs::crypto::xchacha_decrypt_raw(&[7;32], &body).unwrap();
            let plain = String::from_utf8(plain).unwrap();
            let parts: Vec<_> = plain.splitn(3, '|').collect();
            assert!(uuid::Uuid::parse_str(parts[1]).is_ok());
            let request: Value = serde_json::from_str(parts[2]).unwrap();
            count.fetch_add(1, Ordering::SeqCst);
            if request["variables"]["virtualPath"] == "slow" {
                started.notify_one();
                release.notified().await;
            }
            let info = json!({"data":{"sharedInfo":{"name":"current","readOnly":true,"requiresPassword":false,"expiresAt":null,"urlToken":plain_rs::utils::base64::base64_encode(&[8;32]),"entries":[{"name":"a 中文.txt","virtualPath":"folder/ +?#中文%","isDir":false,"size":8,"mimeType":"text/plain","hasThumb":false}]}}});
            plain_rs::transport::http_response::Context::new("/guest_graphql", &body).encrypt(&[7;32], info.to_string().as_bytes()).unwrap()
        }
    })).route("/fs", get(|Query(params): Query<std::collections::HashMap<String, String>>| async move {
        assert_eq!(params["sid"], "fixture");
        if params.contains_key("w") {
            assert_eq!(params["w"], "200");
            assert_eq!(params["h"], "200");
            assert_eq!(params.get("cc").map(String::as_str), Some("false"));
        }
        let plain = plain_rs::crypto::xchacha_decrypt_raw(&[8;32], &plain_rs::utils::base64::base64_decode(&params["id"])).unwrap();
        let decoded: Value = serde_json::from_slice(&plain).unwrap();
        assert_eq!(decoded["virtualPath"], "folder/ +?#中文%");
        "download"
    }));
    let task = tokio::spawn(async move {
        axum_server::from_tcp_rustls(socket, config)
            .serve(router.into_make_service())
            .await
            .unwrap();
    });
    (port, task, count, requested, released)
}
