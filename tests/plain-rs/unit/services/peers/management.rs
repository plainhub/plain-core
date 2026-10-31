use super::*;
use plain_rs::{chat::enums::DeviceType, db::chat_store::SaveMode};
use serde_json::json;

#[test]
fn reconnect_backoff_has_the_existing_one_to_thirty_second_bounds() {
    let mut delay = Duration::from_secs(1);
    for seconds in [2, 4, 8, 16, 30, 30] {
        delay = next_delay(delay);
        assert_eq!(delay, Duration::from_secs(seconds));
    }
}

fn peer() -> DPeer {
    let mut peer = DPeer::new("peer", "Fixture", "::1", Some(18443), DeviceType::Computer);
    peer.token = plain_rs::utils::base64::base64_encode(&[7; 32]);
    peer
}

async fn fixture() -> (tempfile::TempDir, crate::test_runtime::Core, CoreRuntime) {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open_with_cache(
        &directory.path().join("data"),
        &directory.path().join("cache"),
    )
    .unwrap();
    let state = core.runtime_state();
    state.management.shutdown().await;
    (directory, core, state)
}

#[test]
fn connection_identity_uses_the_canonical_tls_address_and_encodes_the_local_client_id() {
    let peer = peer();
    let actor = "actor&auth=1 /?";
    let result = identity(&peer, actor, "https://[::1]:18443").unwrap();
    let url = reqwest::Url::parse(&result.url).unwrap();
    assert_eq!(url.scheme(), "wss");
    assert_eq!(url.host_str(), Some("[::1]"));
    assert_eq!(url.port(), Some(18443));
    assert_eq!(url.path(), "/");
    assert_eq!(
        url.query_pairs().collect::<Vec<_>>(),
        vec![("cid".into(), actor.into())]
    );
    assert!(!result.url.contains(&peer.token));
    let mut renamed = peer.clone();
    renamed.name = "Renamed".into();
    assert!(identity(&renamed, actor, "https://[::1]:18443").unwrap() == result);
}

#[test]
fn chat_only_self_invalid_token_and_missing_address_records_do_not_open_management_sessions() {
    let mut peer = peer();
    assert!(identity(&peer, "peer", "https://[::1]:18443").is_none());
    assert!(identity(&peer, "", "https://[::1]:18443").is_none());
    assert!(identity(&peer, "actor", "").is_none());
    assert!(identity(&peer, "actor", "http://[::1]:8080").is_none());
    peer.port = None;
    assert!(identity(&peer, "actor", "https://[::1]:18443").is_none());
    peer.port = Some(18443);
    for token in [
        String::new(),
        "invalid".into(),
        plain_rs::utils::base64::base64_encode(&[7; 31]),
    ] {
        peer.token = token;
        assert!(identity(&peer, "actor", "https://[::1]:18443").is_none());
    }
}

#[tokio::test]
async fn reconciliation_reuses_unchanged_workers_and_retires_replaced_and_removed_workers() {
    let (_directory, core, state) = fixture().await;
    let identity = identity(&peer(), "actor", "https://[::1]:18443").unwrap();
    let (cancel, cancelled) = watch::channel(false);
    let job = tokio::spawn(std::future::pending::<()>());
    let id = job.id();
    let mut workers = HashMap::from([(
        "peer".into(),
        Worker {
            identity: identity.clone(),
            cancel,
            job,
        },
    )]);
    reconcile(
        &state,
        &mut workers,
        HashMap::from([("peer".into(), identity.clone())]),
    )
    .await;
    assert_eq!(workers["peer"].job.id(), id);
    assert!(!*cancelled.borrow());
    core.stop_core().await;
    let mut replacement = identity;
    replacement.token = plain_rs::utils::base64::base64_encode(&[8; 32]);
    reconcile(
        &state,
        &mut workers,
        HashMap::from([("peer".into(), replacement)]),
    )
    .await;
    assert_ne!(workers["peer"].job.id(), id);
    assert!(*cancelled.borrow());
    let removed = workers["peer"].cancel.subscribe();
    reconcile(&state, &mut workers, HashMap::new()).await;
    assert!(workers.is_empty());
    assert!(*removed.borrow());
}

#[tokio::test]
async fn relay_rechecks_credentials_and_addresses_under_the_database_lock() {
    let (_directory, core, state) = fixture().await;
    let mut peer = peer();
    peer.port = None;
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let expected = Identity {
        url: "wss://[::1]/?cid=actor".into(),
        token: peer.token.clone(),
        ip: peer.ip.clone(),
        port: None,
    };
    let (generation, mut requests) = state.host.connect();
    assert!(
        publish_clipboard_change(&state, "peer", &expected, &watch::channel(false).1)
            .await
            .unwrap()
    );
    let relayed = requests.try_recv().unwrap();
    assert_eq!(relayed["method"], "peerClipboardChanged");
    assert_eq!(
        relayed["params"],
        json!({"peerIds":["peer"],"refreshAll":false})
    );
    state
        .host
        .reply(generation, json!({"id":relayed["id"],"result":null}))
        .unwrap();
    state.db.with_conn(|connection| {
        connection
            .execute("UPDATE peers SET token='' WHERE id='peer'", [])
            .unwrap()
    });
    assert!(
        !publish_clipboard_change(&state, "peer", &expected, &watch::channel(false).1)
            .await
            .unwrap()
    );
    state.db.with_conn(|connection| {
        connection
            .execute(
                "UPDATE peers SET token=?1,ip='127.0.0.1' WHERE id='peer'",
                [&peer.token],
            )
            .unwrap()
    });
    assert!(
        !publish_clipboard_change(&state, "peer", &expected, &watch::channel(false).1)
            .await
            .unwrap()
    );
    assert!(requests.try_recv().is_err());
    state.host.disconnect(generation);
    core.stop_core().await;
}

#[tokio::test]
async fn a_stopped_core_cannot_relay_an_event_even_when_credentials_still_match() {
    let (_directory, core, state) = fixture().await;
    let mut peer = peer();
    peer.port = None;
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let expected = Identity {
        url: "wss://[::1]/".into(),
        token: peer.token,
        ip: peer.ip,
        port: None,
    };
    core.stop_core().await;
    assert!(
        !publish_clipboard_change(&state, "peer", &expected, &watch::channel(false).1)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn peer_snapshot_waits_for_the_database_without_blocking_the_async_thread() {
    let (_directory, core, state) = fixture().await;
    let (locked, ready) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let db = state.db.clone();
    let holder = std::thread::spawn(move || {
        db.with_conn(|_| {
            locked.send(()).unwrap();
            let _ = released.recv_timeout(Duration::from_secs(3));
        });
    });
    ready.await.unwrap();
    let mut pending = Box::pin(desired(&state, "actor"));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(pending.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    release.send(()).unwrap();
    assert!(pending.await.unwrap().1.is_empty());
    holder.join().unwrap();
    core.stop_core().await;
}

#[tokio::test]
async fn a_cancelled_worker_cannot_publish_even_while_credentials_still_match() {
    let (_directory, core, state) = fixture().await;
    let peer = peer();
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let expected = identity(&peer, "actor", "https://[::1]:18443").unwrap();
    let (generation, mut requests) = state.host.connect();
    let (_cancel, cancelled) = watch::channel(true);
    assert!(
        !publish_clipboard_change(&state, "peer", &expected, &cancelled)
            .await
            .unwrap()
    );
    assert!(requests.try_recv().is_err());
    state.host.disconnect(generation);
    core.stop_core().await;
}
