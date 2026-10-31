use crate::server::ContentServer;
use plain_rs::runtime::Runtime as ServerState;
use plain_rs::services::peers::clipboard::*;
use plain_rs::services::peers::management::Identity;
use plain_rs::{
    chat::enums::DeviceType,
    db::{
        DPeer,
        chat_store::{SaveMode, peers},
    },
};
use serde_json::Value;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio::sync::watch;

async fn fixture() -> (
    tempfile::TempDir,
    ContentServer,
    ContentServer,
    ServerState,
    ServerState,
    DPeer,
    Identity,
    watch::Receiver<Vec<String>>,
    Arc<AtomicBool>,
    tokio::task::JoinHandle<()>,
) {
    let directory = tempfile::tempdir().unwrap();
    let remote = ContentServer::open_with_cache(
        &directory.path().join("remote/data"),
        &directory.path().join("remote/cache"),
    )
    .unwrap();
    let remote_state = remote.runtime_state().core;
    remote_state.management.shutdown().await;
    remote_state
        .prefs
        .set("api_permissions", json!(["CLIPBOARD"]))
        .unwrap();
    let session = remote
        .native_management()
        .unwrap()
        .create_session("Clipboard send fixture".into())
        .unwrap();
    remote_state.prefs.set("http_port", 0).unwrap();
    remote_state.prefs.set("https_port", 0).unwrap();
    let (_, port) = remote.start_configured_public().await.unwrap();
    let local = ContentServer::open_with_cache(
        &directory.path().join("local/data"),
        &directory.path().join("local/cache"),
    )
    .unwrap();
    let state = local.runtime_state().core;
    state.management.shutdown().await;
    state.prefs.set("client_id", &session.client_id).unwrap();
    state
        .prefs
        .set("api_permissions", json!(["CLIPBOARD"]))
        .unwrap();
    let mut peer = DPeer::new(
        "remote",
        "Remote",
        "127.0.0.1",
        Some(port),
        DeviceType::Computer,
    );
    peer.token = session.token;
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    state
        .prefs
        .set("clipboard_directions", json!({"remote":"push"}))
        .unwrap();
    let identity = Identity {
        url: format!("wss://127.0.0.1:{port}/?cid={}", session.client_id),
        token: peer.token.clone(),
        ip: peer.ip.clone(),
        port: peer.port,
    };
    let (observed, writes) = watch::channel(Vec::<String>::new());
    let succeeds = Arc::new(AtomicBool::new(false));
    let (generation, mut requests) = remote_state.host.connect();
    let (host, success) = (remote_state.host.clone(), succeeds.clone());
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["CLIPBOARD"]),
                "systemSetClipboard" => {
                    observed.send_modify(|writes| {
                        writes.push(request["params"]["text"].as_str().unwrap().to_owned())
                    });
                    json!(success.load(Ordering::SeqCst))
                }
                _ => Value::Null,
            };
            let _ = host.reply(generation, json!({"id":request["id"],"result":result}));
        }
    });
    plain_rs::services::clipboard::report(
        &state,
        &json!({"text":"Fixture 🌍","sensitive":false}).to_string(),
    )
    .unwrap();
    (
        directory,
        local,
        remote,
        state,
        remote_state,
        peer,
        identity,
        writes,
        succeeds,
        responder,
    )
}

#[tokio::test]
async fn sending_uses_the_real_encrypted_api_preserves_source_and_retries_platform_failure_without_duplicate_history()
 {
    let (
        _directory,
        local,
        remote,
        state,
        remote_state,
        peer,
        identity,
        writes,
        succeeds,
        responder,
    ) = fixture().await;
    let mut remote_events = remote_state.events.subscribe();
    let (_cancel, cancelled) = watch::channel(false);
    assert!(
        send(
            &state,
            &peer.id,
            &identity,
            &cancelled,
            "Fixture 🌍",
            &watch::channel(Some(true)).1
        )
        .await
        .is_err()
    );
    succeeds.store(true, Ordering::SeqCst);
    assert!(
        send(
            &state,
            &peer.id,
            &identity,
            &cancelled,
            "Fixture 🌍",
            &watch::channel(Some(true)).1
        )
        .await
        .unwrap()
    );
    assert_eq!(*writes.borrow(), vec!["Fixture 🌍", "Fixture 🌍"]);
    assert_eq!(remote_state.db.clipboard_count_rows().unwrap(), 1);
    assert!(
        remote_events.try_recv().is_err(),
        "HTTP writes must not broadcast clipboard text or generic refreshes"
    );
    let row = remote_state.db.clipboard_latest().unwrap().unwrap();
    assert_eq!(row.source, state.native_client_id().unwrap());
    assert_eq!(row.text, "Fixture 🌍");
    let outgoing = remote_state.clipboard_changes.subscribe();
    plain_rs::services::clipboard::report(
        &remote_state,
        &json!({"text":"Fixture 🌍","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(!outgoing.has_changed().unwrap());
    assert!(remote_events.try_recv().is_err());
    let observation = remote_state.clipboard_changes.subscribe();
    assert!(!observation.borrow().as_ref().unwrap().eligible);
    local.stop_core().await;
    remote.stop_core().await;
    responder.abort();
    let _ = responder.await;
}

#[tokio::test]
async fn invalid_directions_disabled_local_sharing_revoked_identity_and_cancelled_workers_cannot_write_a_remote_clipboard()
 {
    let (
        _directory,
        local,
        remote,
        state,
        remote_state,
        peer,
        identity,
        writes,
        _succeeds,
        responder,
    ) = fixture().await;
    let (cancel, cancelled) = watch::channel(false);
    for direction in ["off", "pull"] {
        state
            .prefs
            .set("clipboard_directions", json!({"remote":direction}))
            .unwrap();
        assert!(
            !send(
                &state,
                &peer.id,
                &identity,
                &cancelled,
                "Fixture",
                &watch::channel(Some(true)).1
            )
            .await
            .unwrap()
        );
    }
    state
        .prefs
        .set("clipboard_directions", json!({"remote":"invalid"}))
        .unwrap();
    assert!(
        send(
            &state,
            &peer.id,
            &identity,
            &cancelled,
            "Fixture",
            &watch::channel(Some(true)).1
        )
        .await
        .is_err()
    );
    state
        .prefs
        .set("clipboard_directions", json!({"remote":"push"}))
        .unwrap();
    state.prefs.set("api_permissions", json!([])).unwrap();
    assert!(
        !send(
            &state,
            &peer.id,
            &identity,
            &cancelled,
            "Fixture",
            &watch::channel(Some(true)).1
        )
        .await
        .unwrap()
    );
    state
        .prefs
        .set("api_permissions", json!(["CLIPBOARD"]))
        .unwrap();
    cancel.send_replace(true);
    assert!(
        !send(
            &state,
            &peer.id,
            &identity,
            &cancelled,
            "Fixture",
            &watch::channel(Some(true)).1
        )
        .await
        .unwrap()
    );
    cancel.send_replace(false);
    state.db.with_conn(|db| {
        db.execute("UPDATE peers SET token='changed' WHERE id=?1", [&peer.id])
            .unwrap()
    });
    assert!(
        !send(
            &state,
            &peer.id,
            &identity,
            &cancelled,
            "Fixture",
            &watch::channel(Some(true)).1
        )
        .await
        .unwrap()
    );
    for text in [" ".to_owned(), "🌍".repeat(128 * 1024 + 1)] {
        assert!(
            send(
                &state,
                &peer.id,
                &identity,
                &cancelled,
                &text,
                &watch::channel(Some(true)).1
            )
            .await
            .is_err()
        );
    }
    assert!(writes.borrow().is_empty());
    assert_eq!(remote_state.db.clipboard_count_rows().unwrap(), 0);
    local.stop_core().await;
    remote.stop_core().await;
    responder.abort();
    let _ = responder.await;
}

#[tokio::test]
async fn unconfirmed_or_unsupported_remote_capabilities_keep_automatic_pushes_out_of_the_network() {
    use std::{future::Future, task::Poll};
    let (
        _directory,
        local,
        remote,
        state,
        _remote_state,
        peer,
        identity,
        writes,
        _succeeds,
        responder,
    ) = fixture().await;
    let (cancel, cancelled) = watch::channel(false);
    let (access, accessed) = watch::channel(None);
    let changed = tokio::sync::Notify::new();
    let mut service = Box::pin(follow(
        &state, &peer.id, &identity, &cancelled, &changed, accessed,
    ));
    assert!(std::future::poll_fn(|cx| Poll::Ready(service.as_mut().poll(cx).is_pending())).await);
    plain_rs::services::clipboard::report(
        &state,
        &json!({"text":"pending","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(std::future::poll_fn(|cx| Poll::Ready(service.as_mut().poll(cx).is_pending())).await);
    access.send_replace(Some(false));
    assert!(std::future::poll_fn(|cx| Poll::Ready(service.as_mut().poll(cx).is_pending())).await);
    assert!(writes.borrow().is_empty());
    cancel.send_replace(true);
    service.await;
    local.stop_core().await;
    remote.stop_core().await;
    responder.abort();
    let _ = responder.await;
}

#[tokio::test]
async fn automatic_push_coalesces_to_the_latest_local_fact_and_replaces_a_failed_candidate() {
    use std::{future::Future, task::Poll};
    let (
        _directory,
        local,
        remote,
        state,
        remote_state,
        peer,
        identity,
        mut writes,
        succeeds,
        responder,
    ) = fixture().await;
    let (cancel, cancelled) = watch::channel(false);
    let (_access, accessed) = watch::channel(Some(true));
    let changed = tokio::sync::Notify::new();
    let mut service = Box::pin(follow(
        &state, &peer.id, &identity, &cancelled, &changed, accessed,
    ));
    assert!(std::future::poll_fn(|cx| Poll::Ready(service.as_mut().poll(cx).is_pending())).await);
    for text in ["superseded", "first attempt"] {
        plain_rs::services::clipboard::report(
            &state,
            &json!({"text":text,"sensitive":false}).to_string(),
        )
        .unwrap();
    }
    tokio::select! {
        _ = &mut service => panic!("Clipboard worker stopped before sending"),
        result = tokio::time::timeout(Duration::from_secs(3),writes.changed()) => {result.unwrap().unwrap();},
    }
    assert_eq!(*writes.borrow(), vec!["first attempt"]);
    succeeds.store(true, Ordering::SeqCst);
    plain_rs::services::clipboard::report(
        &state,
        &json!({"text":"latest attempt","sensitive":false}).to_string(),
    )
    .unwrap();
    tokio::select! {
        _ = &mut service => panic!("Clipboard worker stopped before replacing a failed candidate"),
        result = tokio::time::timeout(Duration::from_secs(3),writes.changed()) => {result.unwrap().unwrap();},
    }
    assert_eq!(*writes.borrow(), vec!["first attempt", "latest attempt"]);
    cancel.send_replace(true);
    tokio::time::timeout(Duration::from_secs(3), service)
        .await
        .unwrap();
    assert_eq!(
        remote_state.db.clipboard_latest().unwrap().unwrap().text,
        "latest attempt"
    );
    local.stop_core().await;
    remote.stop_core().await;
    responder.abort();
    let _ = responder.await;
}

#[tokio::test]
async fn an_incoming_write_invalidates_a_local_send_while_identity_lookup_is_waiting() {
    use std::{future::Future, task::Poll};
    let (
        _directory,
        local,
        remote,
        state,
        _remote_state,
        peer,
        identity,
        writes,
        _succeeds,
        responder,
    ) = fixture().await;
    let db = state.db.clone();
    let (locked, acquired) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let holder = tokio::task::spawn_blocking(move || {
        db.with_conn(|_| {
            locked.send(()).unwrap();
            released.recv().unwrap();
        })
    });
    acquired.await.unwrap();
    let (_cancel, cancelled) = watch::channel(false);
    let (_access, accessed) = watch::channel(Some(true));
    let mut sending = Box::pin(send(
        &state,
        &peer.id,
        &identity,
        &cancelled,
        "Fixture 🌍",
        &accessed,
    ));
    assert!(std::future::poll_fn(|cx| Poll::Ready(sending.as_mut().poll(cx).is_pending())).await);
    state.clipboard_changes.received("Fixture 🌍");
    release.send(()).unwrap();
    assert!(!sending.await.unwrap());
    holder.await.unwrap();
    assert!(writes.borrow().is_empty());
    local.stop_core().await;
    remote.stop_core().await;
    responder.abort();
    let _ = responder.await;
}
