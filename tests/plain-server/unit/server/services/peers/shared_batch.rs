#![cfg(feature = "http_transport")]

use async_graphql::{Request, Variables};
use plain_rs::runtime::Runtime as ServerState;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Notify;

async fn tls() -> (u16, tokio::task::JoinHandle<()>, Arc<Notify>, Arc<Notify>) {
    use axum::{
        Router,
        body::{Body, Bytes},
        extract::Query,
        routing::get,
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
    let started = Arc::new(Notify::new());
    let released = Arc::new(Notify::new());
    let observed = started.clone();
    let release = released.clone();
    let router = Router::new().route(
        "/fs",
        get(move |Query(params): Query<HashMap<String, String>>| {
            let started = observed.clone();
            let released = release.clone();
            async move {
                let (scope, payload) =
                    plain_rs::files::resource_link::parse(&params["id"]).unwrap();
                assert_eq!(
                    scope,
                    plain_rs::files::resource_link::Scope::Share("fixture".into())
                );
                let resource =
                    plain_rs::files::resource_link::open(&scope, &[8; 32], payload).unwrap();
                let path = resource.id;
                Body::from_stream(futures_util::stream::unfold(0, move |step| {
                    let path = path.clone();
                    let started = started.clone();
                    let released = released.clone();
                    async move {
                        let chunk = match (path.as_str(), step) {
                            ("slow", 0) => {
                                started.notify_one();
                                Bytes::from_static(b"a")
                            }
                            ("slow", 1) => {
                                released.notified().await;
                                Bytes::from_static(b"bc")
                            }
                            ("short", 0) => Bytes::from_static(b"ab"),
                            ("long", 0) => Bytes::from_static(b"abcd"),
                            ("exact", 0) => Bytes::from_static(b"abc"),
                            _ => return None,
                        };
                        Some((Ok::<_, std::io::Error>(chunk), step + 1))
                    }
                }))
            }
        }),
    );
    let task = tokio::spawn(async move {
        axum_server::from_tcp_rustls(socket, config)
            .serve(router.into_make_service())
            .await
            .unwrap();
    });
    (port, task, started, released)
}
fn fixture() -> (tempfile::TempDir, crate::server::ContentServer, ServerState) {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    prefs.set("client_id", "fixture").unwrap();
    let server = crate::server::ContentServer::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state().core;
    (directory, server, state)
}
async fn enqueue(
    server: &crate::server::ContentServer,
    port: u16,
    path: &str,
    target: &std::path::Path,
) -> String {
    let response = server.native_graphql().unwrap().execute(Request::new("mutation($input: SharedDownloadInput!) { enqueueSharedDownload(input: $input) }").variables(Variables::from_json(json!({"input":{
        "messageId":format!("message-{path}"),"kind":"FILE","link":{"host":"127.0.0.1","port":port,"sharedId":"fixture","token":plain_rs::base64_encode(&[7;32]),"pageUrl":""},
        "urlToken":plain_rs::base64_encode(&[8;32]),"entries":[{"name":format!("{path}.bin"),"relativePath":path,"isDir":false,"size":3,"mimeType":"application/octet-stream","hasThumb":false}],
        "targetDir":target.to_str().unwrap(),"zipName":""
    }})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()["enqueueSharedDownload"]
        .as_str()
        .unwrap()
        .to_owned()
}
async fn terminal(state: &ServerState, id: &str) -> Value {
    let mut changed = state.shared_batches.queue.changed.subscribe();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let value = state.shared_batches.queue.snapshot()["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|task| task["id"] == id)
                .unwrap()
                .clone();
            if matches!(
                value["status"].as_str(),
                Some("COMPLETED" | "PARTIAL" | "FAILED" | "CANCELED")
            ) {
                return value;
            }
            changed.changed().await.unwrap();
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn main_queue_downloads_exact_bytes_without_platform_transfer_operations_and_rejects_invalid_lengths()
 {
    let (directory, server, state) = fixture();
    let (port, tls, _, _) = tls().await;
    let target = directory.path().join("downloads");
    let id = enqueue(&server, port, "exact", &target).await;
    let completed = terminal(&state, &id).await;
    assert_eq!(completed["status"], "COMPLETED");
    assert_eq!(completed["downloadedSize"], 3);
    assert_eq!(
        tokio::fs::read(target.join("exact.bin")).await.unwrap(),
        b"abc"
    );
    for path in ["short", "long"] {
        let id = enqueue(&server, port, path, &target).await;
        assert_eq!(terminal(&state, &id).await["status"], "FAILED");
        assert!(!target.join(format!("{path}.bin")).exists());
    }
    let response=server.native_graphql().unwrap().execute(Request::new("{ sharedDownloadQueue { runtimeId revision tasks { id type status downloadedSize entries { name size } failures { path error } } } }")).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["sharedDownloadQueue"]["tasks"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(std::fs::read_dir(&target).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".part")
    }));
    let response = server
        .native_graphql()
        .unwrap()
        .execute(
            Request::new("mutation($id: ID!) { removeSharedDownload(id:$id) }")
                .variables(Variables::from_json(json!({"id":id}))),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["removeSharedDownload"],
        true
    );
    assert!(
        state.shared_batches.queue.snapshot()["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|task| task["id"] != id)
    );
    server.shutdown().await;
    tls.abort();
}
#[tokio::test]
async fn cancel_invalidates_the_generation_and_late_tls_bytes_cannot_commit_a_file() {
    let (directory, server, state) = fixture();
    let (port, tls, started, released) = tls().await;
    let target = directory.path().join("downloads");
    let id = enqueue(&server, port, "slow", &target).await;
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    let generation = state.shared_batches.queue.snapshot()["tasks"][0]["generation"]
        .as_str()
        .unwrap()
        .to_owned();
    let response = server
        .native_graphql()
        .unwrap()
        .execute(
            Request::new("mutation($id: ID!) { cancelSharedDownload(id:$id) }")
                .variables(Variables::from_json(json!({"id":id}))),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["cancelSharedDownload"],
        true
    );
    released.notify_waiters();
    assert_eq!(
        state.shared_batches.queue.snapshot()["tasks"][0]["status"],
        "CANCELED"
    );
    assert!(
        !state
            .shared_batches
            .queue
            .progress(&id, &generation, "stale", 3)
            .unwrap()
    );
    assert!(!target.join("slow.bin").exists());
    assert!(std::fs::read_dir(&target).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".part")
    }));
    server.shutdown().await;
    tls.abort();
}
