use super::*;
use plain_libs::serde_json::json;
use plain_rs::infrastructure::db::DChat;
use plain_rs::infrastructure::db::chat_store::SaveMode;
use plain_rs::infrastructure::db::chat_store::messages;

pub(super) fn root() -> (plain_libs::temporary::TempDir, ContentServer, String) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "local").unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[4; 32]);
    let server = ContentServer::start(dir.path(), &token, prefs).unwrap();
    (dir, server, token)
}
pub(super) fn card(port: u16) -> Card {
    plain_libs::serde_json::from_value(json!({"shareId":"fixture","urlToken":plain_libs::utils::base64::base64_encode_url_safe(&[7;32]),"peerInfo":{"id":"peer","ip":"127.0.0.1","port":port},"name":"old","itemCount":2,"totalSize":8,"expiresAt":"2030-01-01T00:00:00.000Z"})).unwrap()
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
    use plain_libs::axum::{
        Router,
        body::Bytes,
        extract::Query,
        http::HeaderMap,
        response::IntoResponse,
        routing::{get, post},
    };
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let config =
        plain_rs::infrastructure::tls_identity::config(cert_pem.as_slice(), cert_key.as_slice())
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
            let plain = plain_rs::infrastructure::transport::http_request::decrypt_raw(&[7;32], "POST", "/guest_graphql", &body).unwrap();
            let plain = String::from_utf8(plain).unwrap();
            let request: Value = plain_libs::serde_json::from_str(&plain).unwrap();
            assert!(plain_rs::infrastructure::transport::graphql_request::RequestId::parse(request["requestId"].as_str().unwrap()).is_some());
            count.fetch_add(1, Ordering::SeqCst);
            if request["variables"]["relativePath"] == "slow" {
                started.notify_one();
                release.notified().await;
            }
            let info = json!({"data":{"sharedInfo":{"name":"current","readOnly":true,"requiresPassword":false,"expiresAt":null,"urlToken":plain_libs::utils::base64::base64_encode(&[8;32]),"entries":[{"name":"a 中文.txt","relativePath":"folder/ +?#中文%","isDir":false,"size":8,"mimeType":"text/plain","hasThumb":false,"createdAt":null,"updatedAt":"2026-01-01T00:00:00.000Z"}]}}});
            plain_rs::infrastructure::transport::http_response::Context::new(&body).encrypt(&[7;32], info.to_string().as_bytes()).unwrap()
        }
    })).route("/fs", get(|Query(params): Query<std::collections::HashMap<String, String>>| async move {
        let (scope,payload)=plain_rs::features::files::resource_link::parse(&params["id"]).unwrap();
        assert_eq!(scope,plain_rs::features::files::resource_link::Scope::Share("fixture".into()));
        if let Some(size) = params.get("s") {
            assert_eq!(size, "sm");
        }
        assert!(!params.contains_key("w"));
        assert!(!params.contains_key("h"));
        assert!(!params.contains_key("cc"));
        let resource=plain_rs::features::files::resource_link::open(&scope,&[8;32],payload).unwrap();
        if params.get("action").map(String::as_str) == Some("describe") {
            let root = resource.id == "root";
            let child_id = plain_rs::features::files::resource_link::seal(&scope, &[8;32], &plain_rs::features::files::resource_link::Resource { r#type: plain_rs::features::files::resource_link::ResourceType::SharedFile, id: "folder/ +?#中文%".into() }).unwrap();
            let child = json!({"url":plain_rs::features::services::files::links::download_url(&child_id),"name":"a 中文.txt","isDir":false,"sizeBytes":8});
            let entry = if root { json!({"url":plain_rs::features::services::files::links::download_url(&params["id"]),"name":"Root","isDir":true,"sizeBytes":0}) } else { child.clone() };
            return plain_libs::axum::Json(json!({"entry":entry,"children":if root { vec![child] } else { vec![] }})).into_response();
        }
        assert_eq!(resource.id,"folder/ +?#中文%");
        "download".into_response()
    }));
    let task = tokio::spawn(async move {
        let (_stop, stopped) = tokio::sync::watch::channel(false);
        plain_libs::protocols::https::serve(socket, router, config, stopped)
            .await
            .unwrap();
    });
    (port, task, count, requested, released)
}
