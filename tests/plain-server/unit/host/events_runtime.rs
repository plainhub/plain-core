use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct Waiting(Arc<AtomicBool>);

impl Drop for Waiting {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn serving_handles_callbacks_while_request_waits_and_cancels_on_stop() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let session = Arc::new(core.host_events().unwrap());
    let dropped = Arc::new(AtomicBool::new(false));
    let started = Arc::new(tokio::sync::Notify::new());
    let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let runner = tokio::spawn(session.serve({
        let dropped = dropped.clone();
        let started = started.clone();
        move |method, _params| {
            let dropped = dropped.clone();
            let started = started.clone();
            let events = events.clone();
            async move {
                if method == "systemWebLoginRequested" {
                    events.send(method.clone()).unwrap();
                }
                if method == "waitingRequest" {
                    let _waiting = Waiting(dropped);
                    started.notify_one();
                    std::future::pending::<()>().await;
                }
                Ok(json!(true))
            }
        }
    }));
    let host = core.runtime_state().host.clone();
    let call = tokio::spawn(async move { host.call("waitingRequest", json!({})).await });
    started.notified().await;
    core.runtime_state()
        .host
        .notify("systemWebLoginRequested", json!({}))
        .unwrap();
    assert_eq!(received.recv().await.unwrap(), "systemWebLoginRequested");
    assert!(!dropped.load(Ordering::SeqCst));
    core.stop_core().await;
    runner.await.unwrap().unwrap();
    assert!(dropped.load(Ordering::SeqCst));
    assert!(call.await.unwrap().is_err());
}

#[tokio::test]
async fn serving_preserves_request_errors_and_old_session_cannot_disconnect_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let session = Arc::new(core.host_events().unwrap());
    let runner = tokio::spawn(session.serve(|method, _params| async move {
        if method == "failingRequest" {
            Err("platform failure".to_string())
        } else {
            Ok(json!(true))
        }
    }));
    assert_eq!(
        core.runtime_state()
            .host
            .call("failingRequest", json!({}))
            .await
            .unwrap_err(),
        "platform failure"
    );
    let replacement = core.host_events().unwrap();
    runner.await.unwrap().unwrap();
    let host = core.runtime_state().host.clone();
    let request = tokio::spawn(async move { host.call("replacementRequest", json!({})).await });
    loop {
        let next = replacement.next_request().await.unwrap();
        replacement
            .reply(json!({"id": next["id"], "result": true}))
            .unwrap();
        if next["method"] == "replacementRequest" {
            break;
        }
    }
    assert_eq!(request.await.unwrap().unwrap(), json!(true));
    core.stop_core().await;
}

#[tokio::test]
async fn aborting_runner_disconnects_session_even_with_retained_handle() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let session = Arc::new(core.host_events().unwrap());
    let started = Arc::new(tokio::sync::Notify::new());
    let runner = tokio::spawn(session.clone().serve({
        let started = started.clone();
        move |method, _params| {
            let started = started.clone();
            async move {
                if method == "waitingRequest" {
                    started.notify_one();
                    std::future::pending::<()>().await;
                }
                Ok(json!(true))
            }
        }
    }));
    let host = core.runtime_state().host.clone();
    let call = tokio::spawn(async move { host.call("waitingRequest", json!({})).await });
    started.notified().await;
    runner.abort();
    assert!(runner.await.unwrap_err().is_cancelled());
    assert!(core.runtime_state().host.generation().is_none());
    assert!(call.await.unwrap().is_err());
    drop(session);
    core.stop_core().await;
}
