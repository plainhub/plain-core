use plain_libs::protocols::http_client;
use super::*;
use crate::server::ContentServer;
use std::sync::Arc;
use std::time::Duration;

fn server(directory: &std::path::Path, token: &str) -> ContentServer {
    ContentServer::start(
        &directory,
        token,
        Arc::new(plain_rs::features::prefs::Prefs::load(&directory.join("prefs.json")).unwrap()),
    )
    .unwrap()
}

async fn listener_tasks(server: &ContentServer) -> Vec<tokio::task::AbortHandle> {
    server
        .runtime_state()
        .public_server
        .lock()
        .await
        .as_ref()
        .unwrap()
        .listeners
        .tasks
        .iter()
        .map(|task| task.abort_handle())
        .collect()
}

async fn start(server: &ContentServer) -> (u16, u16) {
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    server
        .start_public(
            0,
            0,
            cert_pem,
            cert_key,
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn loopback_and_public_listeners_serve_the_same_health_endpoint() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[9; 32]);
    let server = server(directory.path(), &token);
    let client = http_client::builder().build().unwrap();
    let url = format!("http://127.0.0.1:{}/health", server.port);
    assert_eq!(client.get(&url).send().await.unwrap().status(), 200);
    assert_eq!(
        client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert!(server.http_ports().await.is_none());
    start(&server).await;
    let state = server.runtime_state();
    assert!(
        state
            .public_server
            .lock()
            .await
            .as_ref()
            .unwrap()
            .listeners
            .check_health()
            .await
            .is_ok()
    );
    server.stop_public().await;
    assert!(server.http_ports().await.is_none());
    server.shutdown().await;
}

#[tokio::test]
async fn listener_failure_cleans_up_and_notifies_host() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[10; 32]);
    let server = server(directory.path(), &token);
    start(&server).await;
    let state = server.runtime_state();
    let tasks = listener_tasks(&server).await;
    let (host_generation, mut calls) = state.host.connect();
    let host = state.host.clone();
    let (reported, mut reports) = tokio::sync::mpsc::unbounded_channel();
    let responder = tokio::spawn(async move {
        while let Some(request) = calls.recv().await {
            if request["method"] == "httpServerStatusChanged" {
                reported.send(request["params"].clone()).unwrap();
            }
            let _ = host.reply(
                host_generation,
                plain_libs::serde_json::json!({"id":request["id"],"result":[]}),
            );
        }
    });
    let (failure, receiver) = watch::channel(None);
    let stop = state
        .public_server
        .lock()
        .await
        .as_ref()
        .unwrap()
        .stop
        .subscribe();
    monitor(state.clone(), receiver, stop);
    failure
        .send(Some("injected listener failure".into()))
        .unwrap();
    let failure = tokio::time::timeout(Duration::from_secs(5), reports.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        failure,
        plain_libs::serde_json::json!({"state":"error","message":"injected listener failure","httpPort":0,"httpsPort":0})
    );
    assert!(server.http_ports().await.is_none());
    assert!(tasks.iter().all(|task| task.is_finished()));
    start(&server).await;
    assert_eq!(server.http_status().state, "started");
    assert!(server.http_status().message.is_empty());
    state.host.disconnect(host_generation);
    server.shutdown().await;
    responder.await.unwrap();
}

#[tokio::test]
async fn stale_failure_cannot_stop_a_new_listener() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[11; 32]);
    let server = server(directory.path(), &token);
    start(&server).await;
    let state = server.runtime_state();
    let stop = state
        .public_server
        .lock()
        .await
        .as_ref()
        .unwrap()
        .stop
        .subscribe();
    server.stop_public().await;
    let ports = start(&server).await;
    let (failure, receiver) = watch::channel(Some("late failure".into()));
    monitor(state.clone(), receiver, stop);
    tokio::task::yield_now().await;
    assert_eq!(server.http_ports().await, Some(ports));
    assert!(
        state
            .public_server
            .lock()
            .await
            .as_ref()
            .unwrap()
            .listeners
            .check_health()
            .await
            .is_ok()
    );
    drop(failure);
    let tasks = listener_tasks(&server).await;
    server.shutdown().await;
    assert!(tasks.iter().all(|task| task.is_finished()));
}

#[tokio::test]
async fn dropping_core_releases_public_listeners() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[12; 32]);
    let server = server(directory.path(), &token);
    start(&server).await;
    let tasks = listener_tasks(&server).await;
    drop(server);
    tokio::time::timeout(Duration::from_secs(5), async {
        while tasks.iter().any(|task| !task.is_finished()) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("Dropping the core must release all public listeners");
}

#[tokio::test]
async fn health_works_without_a_host() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[14; 32]);
    let server = server(directory.path(), &token);
    let (port, _) = start(&server).await;
    let client = http_client::builder().build().unwrap();
    let base = format!("http://127.0.0.1:{port}");
    let response = client.get(format!("{base}/health")).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "OK");
    server.shutdown().await;
}

#[tokio::test]
async fn public_discovery_withdrawal_finishes_before_lifetime_returns() {
    let (_core, core_stop) = watch::channel(false);
    let (public, public_stop) = watch::channel(false);
    let (started, ready) = tokio::sync::oneshot::channel();
    let (withdrawn, withdraw_started) = tokio::sync::oneshot::channel();
    let (release, wait_release) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(public_discovery_lifetime(
        core_stop,
        public_stop,
        async move {
            started.send(()).unwrap();
            Ok(())
        },
        async move {
            withdrawn.send(()).unwrap();
            wait_release.await.unwrap();
            Ok(())
        },
    ));
    ready.await.unwrap();
    public.send_replace(true);
    withdraw_started.await.unwrap();
    assert!(!task.is_finished());
    release.send(()).unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn stopping_public_discovery_cancels_pending_start_and_runs_withdrawal_once() {
    let (_core, core_stop) = watch::channel(false);
    let (public, public_stop) = watch::channel(false);
    let (entered, started) = tokio::sync::oneshot::channel();
    let (cancelled, cancellation) = tokio::sync::oneshot::channel();
    struct CancelSignal(Option<tokio::sync::oneshot::Sender<()>>);
    impl Drop for CancelSignal {
        fn drop(&mut self) {
            self.0.take().unwrap().send(()).unwrap();
        }
    }
    let withdrawals = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = withdrawals.clone();
    let task = tokio::spawn(public_discovery_lifetime(
        core_stop,
        public_stop,
        async move {
            let _guard = CancelSignal(Some(cancelled));
            entered.send(()).unwrap();
            std::future::pending::<plain_libs::anyhow::Result<()>>().await
        },
        async move {
            cancellation.await.unwrap();
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        },
    ));
    started.await.unwrap();
    public.send_replace(true);
    task.await.unwrap();
    assert_eq!(withdrawals.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn already_stopped_discovery_never_publishes_or_withdraws() {
    let (_core, core_stop) = watch::channel(false);
    let (_public, public_stop) = watch::channel(true);
    public_discovery_lifetime(
        core_stop,
        public_stop,
        async { panic!("Publication must not start") },
        async { panic!("Withdrawal must not start") },
    )
    .await;
}
