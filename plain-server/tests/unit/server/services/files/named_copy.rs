use crate::server::ContentServer;
use async_graphql::{Request, Response, Variables};
use serde_json::json;
use std::sync::Arc;

async fn execute(
    core: &ContentServer,
    source: &str,
    target: &std::path::Path,
    name: &str,
) -> Response {
    core.native_graphql().unwrap().execute(Request::new(
        "mutation($source:String!,$directory:String!,$name:String!){copyFileToDirectory(src:$source,directory:$directory,name:$name)}",
    ).variables(Variables::from_json(json!({"source":source,"directory":target,"name":name})))).await
}

async fn resource_request(
    state: &crate::server::ServerState,
    generation: u64,
    requests: &mut tokio::sync::mpsc::Receiver<serde_json::Value>,
) -> serde_json::Value {
    loop {
        let request = requests.recv().await.unwrap();
        if request["method"] == "systemCapabilities" {
            state
                .host
                .reply(generation, json!({"id":request["id"],"result":[]}))
                .unwrap();
            continue;
        }
        assert_eq!(request["method"], "fileResourceStream");
        return request;
    }
}

#[tokio::test]
async fn provider_failure_never_publishes_partial_output_and_releases_staging() {
    for succeeds in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let core = Arc::new(ContentServer::open(&dir.path().join("data")).unwrap());
        let state = core.runtime_state();
        let (generation, mut requests) = state.host.connect();
        let target = dir.path().join("exports");
        let running = {
            let core = core.clone();
            let target = target.clone();
            tokio::spawn(async move {
                execute(&core, "content://fixture/picked", &target, "picked.txt").await
            })
        };
        let request = resource_request(&state, generation, &mut requests).await;
        let stream = state
            .resources
            .attach_native(request["params"]["id"].as_str().unwrap(), generation)
            .unwrap();
        state
            .host
            .reply(generation, json!({"id":request["id"],"result":true}))
            .unwrap();
        let metadata: serde_json::Value =
            serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
        assert_eq!(metadata["path"], "content://fixture/picked");
        stream.write(b"first".to_vec()).await.unwrap();
        if succeeds {
            stream.write(b" second".to_vec()).await.unwrap();
            stream.finish().await.unwrap();
        } else {
            stream.fail().await.unwrap();
        }
        let result = running.await.unwrap();
        if succeeds {
            assert!(result.errors.is_empty(), "{:?}", result.errors);
            assert_eq!(
                std::fs::read(target.join("picked.txt")).unwrap(),
                b"first second"
            );
        } else {
            assert!(!result.errors.is_empty());
            assert!(!target.exists());
        }
        assert_eq!(
            std::fs::read_dir(state.cache.root())
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().unwrap().is_file())
                .count(),
            0
        );
        core.stop_core().await;
    }
}

#[tokio::test]
async fn cache_clear_waits_for_owned_copy_after_the_graphql_waiter_is_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let core = Arc::new(ContentServer::open(&dir.path().join("data")).unwrap());
    let state = core.runtime_state();
    let (generation, mut requests) = state.host.connect();
    let target = dir.path().join("exports");
    let running = {
        let core = core.clone();
        let target = target.clone();
        tokio::spawn(
            async move { execute(&core, "ph://fixture/picked", &target, "picked.txt").await },
        )
    };
    let request = resource_request(&state, generation, &mut requests).await;
    let stream = state
        .resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    state
        .host
        .reply(generation, json!({"id":request["id"],"result":true}))
        .unwrap();
    stream.write(b"owned".to_vec()).await.unwrap();
    assert!(state.cache_changes.try_write().is_err());
    running.abort();
    assert!(running.await.unwrap_err().is_cancelled());
    let clearing = {
        let state = state.clone();
        tokio::spawn(async move { plain_rs::services::cache::clear_application(&state).await })
    };
    assert!(!target.exists());
    stream.write(b" copy".to_vec()).await.unwrap();
    stream.finish().await.unwrap();
    clearing.await.unwrap().unwrap();
    assert_eq!(
        std::fs::read(target.join("picked.txt")).unwrap(),
        b"owned copy"
    );
    core.stop_core().await;
}

#[tokio::test]
async fn invalid_destinations_are_rejected_before_provider_reads_or_network_work() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let fetched = Arc::new(AtomicUsize::new(0));
    let observed = fetched.clone();
    let routes = axum::Router::new().route(
        "/file",
        axum::routing::get(move || {
            let observed = observed.clone();
            async move {
                observed.fetch_add(1, Ordering::SeqCst);
                "unused"
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes).await.unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&dir.path().join("data")).unwrap();
    let state = core.runtime_state();
    let (generation, mut requests) = state.host.connect();
    for source in [
        "content://fixture/picked".to_owned(),
        format!("http://{address}/file"),
    ] {
        for name in ["", "..", "../escape", "bad\\name"] {
            let response = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                execute(&core, &source, &dir.path().join("exports"), name),
            )
            .await
            .unwrap();
            assert!(!response.errors.is_empty());
        }
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            execute(&core, &source, std::path::Path::new("relative"), "file.txt"),
        )
        .await
        .unwrap();
        assert!(!response.errors.is_empty());
    }
    while let Ok(request) = requests.try_recv() {
        assert_eq!(request["method"], "systemCapabilities");
        state
            .host
            .reply(generation, json!({"id":request["id"],"result":[]}))
            .unwrap();
    }
    assert_eq!(fetched.load(Ordering::SeqCst), 0);
    assert!(!dir.path().join("exports").exists());
    core.stop_core().await;
    server.abort();
}

#[tokio::test]
async fn remote_copy_follows_redirects_fetches_fresh_bytes_and_leaves_no_output_on_http_failure() {
    use axum::{Router, http::StatusCode, response::Redirect, routing::get};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let routes = Router::new()
        .route("/redirect", get(|| async { Redirect::temporary("/file") }))
        .route(
            "/file",
            get(move || {
                let count = count.clone();
                async move { format!("bytes-{}", count.fetch_add(1, Ordering::SeqCst)) }
            }),
        )
        .route("/missing", get(|| async { StatusCode::NOT_FOUND }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, routes).await.unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&dir.path().join("data")).unwrap();
    let target = dir.path().join("exports");
    for (suffix, expected) in [("", b"bytes-0".as_slice()), (" (1)", b"bytes-1".as_slice())] {
        let result = execute(
            &core,
            &format!("http://{address}/redirect"),
            &target,
            "download.txt",
        )
        .await;
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(
            std::fs::read(target.join(format!("download{suffix}.txt"))).unwrap(),
            expected
        );
    }
    assert_eq!(requests.load(Ordering::SeqCst), 2);
    let result = execute(
        &core,
        &format!("http://{address}/missing"),
        &target,
        "missing.txt",
    )
    .await;
    assert!(!result.errors.is_empty());
    assert!(!target.join("missing.txt").exists());
    assert_eq!(
        std::fs::read_dir(core.cache_paths().root().join("remote_files"))
            .unwrap()
            .count(),
        0
    );
    core.stop_core().await;
    server.abort();
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn public_url_copy_rejects_untrusted_tls_even_after_a_peer_cache_client_was_used() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let config = axum_server::tls_rustls::RustlsConfig::from_pem(
        certificate.cert.pem().into_bytes(),
        certificate.key_pair.serialize_pem().into_bytes(),
    )
    .await
    .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let router = axum::Router::new().route(
        "/file",
        axum::routing::get(move || {
            let calls = observed.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                "fixture bytes"
            }
        }),
    );
    let server = tokio::spawn(async move {
        axum_server::from_tcp_rustls(listener, config)
            .serve(router.into_make_service())
            .await
            .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&dir.path().join("data")).unwrap();
    let state = core.runtime_state();
    let url = format!("https://localhost:{port}/file");
    let legacy = state
        .remote_cache
        .download(&url, "peer.bin", None, state.stop.clone(), None)
        .await
        .unwrap();
    assert_eq!(std::fs::read(legacy).unwrap(), b"fixture bytes");
    let target = dir.path().join("exports");
    let response = execute(&core, &url, &target, "public.bin").await;
    assert!(!response.errors.is_empty());
    assert!(!target.exists());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    core.stop_core().await;
    server.abort();
}

#[tokio::test]
async fn native_provider_uri_copy_is_not_exposed_to_remote_filesystem_callers() {
    let dir = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&dir.path().join("data")).unwrap();
    let state = core.runtime_state();
    state
        .prefs
        .set("api_permissions", json!(["WRITE_EXTERNAL_STORAGE"]))
        .unwrap();
    let (generation, mut requests) = state.host.connect();
    let target = dir.path().join("exports");
    for source in ["content://fixture/picked", "ph://fixture/picked"] {
        let request = Request::new(
            "mutation($source:String!,$directory:String!){copyFileToDirectory(src:$source,directory:$directory,name:\"picked.txt\")}",
        ).variables(Variables::from_json(json!({"source":source,"directory":target})));
        let response = crate::graphql::executor::execute_graphql(
            &state.public,
            request,
            crate::graphql::executor::RequestContext::remote(Some(state.clone())),
        )
        .await;
        assert_eq!(response.errors.len(), 1);
        assert_eq!(response.errors[0].message, "no_permission");
    }
    while let Ok(request) = requests.try_recv() {
        assert_eq!(request["method"], "systemCapabilities");
        state
            .host
            .reply(generation, json!({"id":request["id"],"result":[]}))
            .unwrap();
    }
    assert!(!target.exists());
    core.stop_core().await;
}

#[tokio::test]
async fn copies_logical_image_references_without_changing_the_source() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let core = ContentServer::open(&data).unwrap();
    let source = data.join("note-images/source.png");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, b"original image").unwrap();
    let target = dir.path().join("exports");
    for reference in [
        "app://note-images/source.png".to_owned(),
        source.to_string_lossy().into_owned(),
    ] {
        let response = execute(&core, &reference, &target, "saved.png").await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let saved = response.data.into_json().unwrap()["copyFileToDirectory"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(std::path::Path::new(&saved).starts_with(&target));
        assert_eq!(std::fs::read(saved).unwrap(), b"original image");
        assert_eq!(std::fs::read(&source).unwrap(), b"original image");
    }
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 2);
    let rejected = dir.path().join("rejected");
    let response = execute(&core, "app://../outside.png", &rejected, "escape.png").await;
    assert!(!response.errors.is_empty());
    assert!(!rejected.exists());
    core.stop_core().await;
}
