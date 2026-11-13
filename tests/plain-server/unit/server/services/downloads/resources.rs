#![cfg(feature = "http_transport")]

use plain_graphql::{Request, Variables};
use plain_libs::serde_json::{Value, json};
use plain_rs::runtime::Runtime as ServerState;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Notify;

async fn tls() -> (
    u16,
    tokio::task::JoinHandle<()>,
    Arc<Notify>,
    tokio::sync::watch::Sender<bool>,
) {
    use plain_libs::axum::{
        Router,
        body::{Body, Bytes},
        extract::Query,
        response::IntoResponse,
        routing::get,
    };
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let config =
        plain_rs::infrastructure::tls_identity::config(cert_pem.as_slice(), cert_key.as_slice())
            .unwrap();
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    let started = Arc::new(Notify::new());
    let (released, release) = tokio::sync::watch::channel(false);
    let observed = started.clone();
    let router = Router::new().route(
        "/fs",
        get(move |Query(params): Query<HashMap<String, String>>| {
            let started = observed.clone();
            let released = release.clone();
            async move {
                let (scope, payload) =
                    plain_rs::features::files::resource_link::parse(&params["id"]).unwrap();
                assert_eq!(
                    scope,
                    plain_rs::features::files::resource_link::Scope::Share("fixture".into())
                );
                let resource =
                    plain_rs::features::files::resource_link::open(&scope, &[8; 32], payload)
                        .unwrap();
                let path = resource.id;
                if params.get("action").map(String::as_str) == Some("describe") {
                    return plain_libs::axum::Json(json!({"entry":{"url":format!("/fs?id={}&action=download", params["id"]),"name":format!("{path}.bin"),"isDir":false,"sizeBytes":3},"children":[]})).into_response();
                }
                Body::from_stream(futures_util::stream::unfold(0, move |step| {
                    let path = path.clone();
                    let started = started.clone();
                    let mut released = released.clone();
                    async move {
                        let chunk = match (path.as_str(), step) {
                            ("slow", 0) => {
                                started.notify_one();
                                Bytes::from_static(b"a")
                            }
                            ("slow", 1) => {
                                if !*released.borrow_and_update() {
                                    released.changed().await.unwrap();
                                }
                                Bytes::from_static(b"bc")
                            }
                            ("short", 0) => Bytes::from_static(b"ab"),
                            ("long", 0) => Bytes::from_static(b"abcd"),
                            ("exact", 0) => Bytes::from_static(b"abc"),
                            _ => return None,
                        };
                        Some((Ok::<_, std::io::Error>(chunk), step + 1))
                    }
                })).into_response()
            }
        }),
    );
    let task = tokio::spawn(async move {
        let (_stop, stopped) = tokio::sync::watch::channel(false);
        plain_libs::protocols::https::serve(socket, router, config, stopped)
            .await
            .unwrap();
    });
    (port, task, started, released)
}
fn fixture() -> (
    plain_libs::temporary::TempDir,
    crate::server::ContentServer,
    ServerState,
) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(
        plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    prefs.set("client_id", "fixture").unwrap();
    let server = crate::server::ContentServer::start(
        directory.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
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
    let link = plain_rs::features::files::shares::client::Link::new(
        "127.0.0.1",
        port,
        "fixture",
        &plain_libs::utils::base64::base64_encode(&[7; 32]),
    )
    .unwrap();
    let url = format!(
        "{}&action=download",
        link.file_url(
            &plain_libs::utils::base64::base64_encode(&[8; 32]),
            path,
            false
        )
        .unwrap()
    );
    let response = server.native_graphql().unwrap().execute(Request::new("mutation($input:DownloadInput!){createDownload(input:$input){id}}").variables(Variables::from_json(json!({"input":{"resources":{"urls":[url],"targetDir":target.to_str().unwrap()}}})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()["createDownload"]["id"]
        .as_str()
        .unwrap()
        .into()
}

async fn terminal(state: &ServerState, id: &str) -> Value {
    let mut changed = state.downloads.changed.subscribe();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let task = state
                .downloads
                .snapshot()
                .into_iter()
                .find(|task| task.id == id)
                .unwrap();
            if task.status.terminal() {
                return plain_libs::serde_json::to_value(task).unwrap();
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
    assert_eq!(completed["downloadedBytes"], 3);
    assert_eq!(
        tokio::fs::read(target.join("exact.bin")).await.unwrap(),
        b"abc"
    );
    for path in ["short", "long"] {
        let id = enqueue(&server, port, path, &target).await;
        assert_eq!(terminal(&state, &id).await["status"], "FAILED");
        assert!(!target.join(format!("{path}.bin")).exists());
    }
    let response = server
        .native_graphql()
        .unwrap()
        .execute(Request::new(
            "{ downloads { id status downloadedBytes failures { path error } } }",
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["downloads"]
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
            Request::new("mutation($id: ID!) { removeDownload(id:$id) }")
                .variables(Variables::from_json(json!({"id":id}))),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["removeDownload"], true);
    assert!(state.downloads.snapshot().iter().all(|task| task.id != id));
    assert_eq!(std::fs::read(target.join("exact.bin")).unwrap(), b"abc");
    server.shutdown().await;
    tls.abort();
}

#[tokio::test]
async fn pause_and_resume_restart_the_unfinished_file_without_changing_the_task_or_leaving_staging()
{
    let (directory, server, state) = fixture();
    let (port, tls, started, released) = tls().await;
    let target = directory.path().join("downloads");
    let id = enqueue(&server, port, "slow", &target).await;
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    let response = server
        .native_graphql()
        .unwrap()
        .execute(
            Request::new("mutation($id: ID!) { pauseDownload(id:$id) }")
                .variables(Variables::from_json(json!({"id":id}))),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["pauseDownload"], true);
    let paused = &state.downloads.snapshot()[0];
    assert_eq!(paused.id, id);
    assert_eq!(paused.status, plain_rs::features::downloads::Status::Paused);
    assert_eq!((paused.done_files, paused.downloaded_bytes), (0, 0));
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
    let response = server
        .native_graphql()
        .unwrap()
        .execute(
            Request::new("mutation($id: ID!) { resumeDownload(id:$id) }")
                .variables(Variables::from_json(json!({"id":id}))),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["resumeDownload"], true);
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    released.send_replace(true);
    let completed = terminal(&state, &id).await;
    assert_eq!(completed["status"], "COMPLETED");
    assert_eq!(completed["downloadedBytes"], 3);
    assert_eq!(std::fs::read(target.join("slow.bin")).unwrap(), b"abc");
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 1);
    assert_eq!(state.downloads.snapshot().len(), 1);
    assert_eq!(state.downloads.snapshot()[0].id, id);
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
    let response = server
        .native_graphql()
        .unwrap()
        .execute(
            Request::new("mutation($id: ID!) { cancelDownload(id:$id) }")
                .variables(Variables::from_json(json!({"id":id}))),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["cancelDownload"], true);
    released.send_replace(true);
    assert_eq!(
        state.downloads.snapshot()[0].status,
        plain_rs::features::downloads::Status::Canceled
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
