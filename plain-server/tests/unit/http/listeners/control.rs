use super::*;
use plain_rs::services::http_status::*;
use serde_json::json;

#[test]
fn restart_receipts_are_serial_and_keep_live_ports_on_preflight_failure() {
    let runtime = Runtime::default();
    runtime.started((8080, 8443));
    let receipt = runtime.begin().unwrap();
    assert!(receipt.restarting);
    assert_eq!(receipt.revision.0, 1);
    assert!(runtime.begin().is_err());
    runtime.finish(receipt.revision, Err("TLS identity invalid".into()));
    let failed = runtime.snapshot();
    assert!(!failed.restarting);
    assert_eq!((failed.http_port, failed.https_port), (8080, 8443));
    assert_eq!(failed.error, "TLS identity invalid");
    let next = runtime.begin().unwrap();
    assert_eq!(next.revision.0, 2);
    assert!(next.error.is_empty());
    runtime.finish(receipt.revision, Ok((1, 2)));
    assert!(runtime.snapshot().restarting);
    runtime.stopped();
    runtime.started((8180, 8543));
    assert!(runtime.snapshot().restarting);
    runtime.finish(next.revision, Ok((8180, 8543)));
    let completed = runtime.snapshot();
    assert!(!completed.restarting);
    assert_eq!((completed.http_port, completed.https_port), (8180, 8543));
}
#[cfg(feature = "http_transport")]
async fn complete(runtime: &Runtime) -> HttpServerState {
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        runtime.wait_finished(runtime.snapshot().revision),
    )
    .await
    .unwrap()
    .unwrap()
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn restart_service_acknowledges_before_the_shutdown_lock_and_rejects_duplicates() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let lock = state.shutdown.lock().await;
    let (_generation, mut requests) = state.host.connect();
    let receipt = restart(&state, Some(0), Some(0)).unwrap();
    assert!(receipt.restarting);
    assert_eq!(receipt.revision.0, 1);
    assert!(restart(&state, None, None).is_err());
    assert!(requests.try_recv().is_err());
    drop(lock);
    let result = complete(&state.http_server).await;
    assert!(result.error.is_empty(), "{}", result.error);
    assert!(result.http_port > 0);
    assert!(result.https_port > 0);
    core.stop_core().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn native_restart_returns_final_ports_and_the_public_health_endpoint_is_ready() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let management = core.native_management().unwrap();
    let result = management
        .restart_http_server(Some(0), Some(0))
        .await
        .unwrap();
    assert!(!result.restarting);
    assert!(result.error.is_empty(), "{}", result.error);
    let settings = management.preferences().unwrap();
    assert_eq!(settings["http_port"], result.http_port);
    assert_eq!(settings["https_port"], result.https_port);
    let response = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{}/health", result.http_port))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    assert_eq!(response.text().await.unwrap(), "OK");
    assert_eq!(
        core.http_ports().await,
        Some((result.http_port as u16, result.https_port as u16))
    );
    core.stop_core().await;
}

#[tokio::test]
async fn remote_and_invalid_restart_requests_never_start_a_runtime_operation() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let remote = crate::graphql::executor::execute_graphql(
        &state.public,
        async_graphql::Request::new("mutation{restartHttpServer(httpPort:0){revision}}"),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!remote.errors.is_empty());
    for document in [
        "mutation{restartHttpServer(httpPort:-1){revision}}",
        "mutation{restartHttpServer(httpsPort:65536){revision}}",
    ] {
        assert!(
            !core
                .native_graphql()
                .unwrap()
                .execute(async_graphql::Request::new(document))
                .await
                .errors
                .is_empty()
        );
    }
    assert_eq!(state.http_server.snapshot().revision.0, 0);
    core.stop_core().await;
}

#[tokio::test]
async fn listener_failure_is_reported_to_the_host_with_current_status() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (generation, mut requests) = state.host.connect();
    state.http_server.failed("listener failed".into());
    state.publish_http_status();
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "httpServerStatusChanged");
    assert_eq!(
        request["params"],
        json!({"state":"error","message":"listener failed","httpPort":0,"httpsPort":0})
    );
    state
        .host
        .reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    core.stop_core().await;
}

#[tokio::test]
async fn native_restart_waiter_rejects_a_replaced_operation() {
    let runtime = Runtime::default();
    let first = runtime.begin().unwrap();
    runtime.finish(first.revision, Ok((8080, 8443)));
    let second = runtime.begin().unwrap();
    assert!(
        runtime
            .wait_finished(first.revision)
            .await
            .err()
            .unwrap()
            .contains("superseded")
    );
    runtime.finish(second.revision, Ok((8180, 8543)));
    let finished = runtime.wait_finished(second.revision).await.unwrap();
    assert_eq!((finished.http_port, finished.https_port), (8180, 8543));
}
#[tokio::test]
async fn state_changes_before_host_attachment_are_delivered_as_the_initial_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let before = state
        .host
        .callback_attempts
        .load(std::sync::atomic::Ordering::Relaxed);
    state.http_server.failed("listener startup failed".into());
    state.publish_http_status();
    assert_eq!(
        state
            .host
            .callback_attempts
            .load(std::sync::atomic::Ordering::Relaxed),
        before
    );
    let (generation, mut requests) = state.host.connect();
    state.http_server.notify_snapshot(&state.host).unwrap();
    let request = requests.try_recv().unwrap();
    assert_eq!(request["method"], "httpServerStatusChanged");
    assert_eq!(request["params"]["state"], "error");
    assert_eq!(request["params"]["message"], "listener startup failed");
    state
        .host
        .reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    state.host.disconnect(generation);
    core.stop_core().await;
}
