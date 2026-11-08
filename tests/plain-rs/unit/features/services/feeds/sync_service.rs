use super::*;
use axum::{Router, routing::get};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;

pub(super) async fn fixture() -> (
    plain_libs::temporary::TempDir,
    Arc<SyncService>,
    Arc<AtomicUsize>,
    Arc<Semaphore>,
    Arc<Semaphore>,
    tokio::task::JoinHandle<()>,
    broadcast::Receiver<WsEvent>,
) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Arc::new(Db::open(&dir.path().join("feeds.db")).unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/rss", get({
        let calls=calls.clone(); let started=started.clone(); let release=release.clone();
        move || { let calls=calls.clone(); let started=started.clone(); let release=release.clone(); async move {
            calls.fetch_add(1, Ordering::SeqCst);
            started.add_permits(1);
            release.acquire().await.unwrap().forget();
            "<rss><channel><title>News</title><item><guid>one</guid><title>One</title></item></channel></rss>"
        }}
    }));
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    db.feed_save(
        "feed",
        "News",
        &format!("http://{address}/rss"),
        false,
        &super::super::now(),
    )
    .unwrap();
    let (events, receiver) = broadcast::channel(256);
    let host = Arc::new(crate::infrastructure::host::bridge::Host::default());
    let service = SyncService::new(db, events, host, None);
    (dir, service, calls, started, release, server, receiver)
}
pub(super) async fn finished(mut receiver: watch::Receiver<FeedSyncState>) -> FeedSyncState {
    tokio::time::timeout(std::time::Duration::from_secs(10), async move {
        while receiver.borrow().active() {
            receiver.changed().await.unwrap();
        }
        receiver.borrow().clone()
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn all_and_single_requests_share_the_same_in_flight_feed_and_only_emit_feed_events() {
    let (_dir, service, calls, started, release, server, mut events) = fixture().await;
    service.queue(None).unwrap();
    started.acquire().await.unwrap().forget();
    let original = service.state.lock().unwrap().tasks["feed"]
        .state
        .subscribe();
    service.queue(Some("feed".into())).unwrap();
    service.queue(Some("feed".into())).unwrap();
    assert_eq!(
        service.state.lock().unwrap().tasks["feed"]
            .state
            .borrow()
            .task_id,
        original.borrow().task_id
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let batch = service
        .state
        .lock()
        .unwrap()
        .batch
        .as_ref()
        .unwrap()
        .subscribe();
    release.add_permits(1);
    assert_eq!(finished(original).await.status, FeedWorkerStatus::Completed);
    assert_eq!(finished(batch).await.status, FeedWorkerStatus::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let mut types = Vec::new();
    while let Ok(event) = events.try_recv() {
        types.push(event.event_type);
    }
    assert!(!types.contains(&"FEED_SYNC_UPDATED"));
    assert!(types.contains(&"FEEDS_FETCHED"));
    assert!(!types.contains(&"CONTENT_CHANGED"));
    service.shutdown().await;
    server.abort();
}
#[tokio::test]
async fn shutdown_cancels_owned_fetches_without_late_sync_completion_or_detached_tasks() {
    let (_dir, service, calls, started, _release, server, _events) = fixture().await;
    service.queue(Some("feed".into())).unwrap();
    started.acquire().await.unwrap().forget();
    let receiver = service.state.lock().unwrap().tasks["feed"]
        .state
        .subscribe();
    struct SchedulerGuard(Arc<AtomicUsize>);
    impl Drop for SchedulerGuard {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let scheduler_exits = Arc::new(AtomicUsize::new(0));
    let (scheduler_started, scheduler_ready) = tokio::sync::oneshot::channel();
    *service.scheduler.lock().unwrap() = Some(tokio::spawn({
        let scheduler_exits = scheduler_exits.clone();
        async move {
            let _guard = SchedulerGuard(scheduler_exits);
            scheduler_started.send(()).unwrap();
            std::future::pending::<()>().await;
        }
    }));
    scheduler_ready.await.unwrap();
    service.shutdown().await;
    assert_eq!(scheduler_exits.load(Ordering::SeqCst), 1);
    assert!(service.scheduler.lock().unwrap().is_none());
    assert_eq!(finished(receiver).await.status, FeedWorkerStatus::Cancelled);
    assert!(
        service
            .db
            .feed_get("feed")
            .unwrap()
            .unwrap()
            .last_sync_at
            .is_none()
    );
    assert!(service.queue(None).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    server.abort();
}
#[tokio::test]
async fn independent_feeds_share_one_global_concurrency_limit() {
    let (_dir, service, calls, started, _release, server, mut events) = fixture().await;
    let feed = service.db.feed_get("feed").unwrap().unwrap();
    for id in 1..18 {
        service
            .db
            .feed_save(
                &format!("feed-{id}"),
                "News",
                &format!("{}?feed={id}", feed.url),
                false,
                &super::super::now(),
            )
            .unwrap();
    }
    service.queue(None).unwrap();
    started.acquire_many(16).await.unwrap().forget();
    let states = service.states();
    assert_eq!(
        states
            .iter()
            .filter(|state| state.feed_id.as_str() != "all"
                && state.status == FeedWorkerStatus::Running)
            .count(),
        16
    );
    assert_eq!(
        states
            .iter()
            .filter(|state| state.status == FeedWorkerStatus::Queued)
            .count(),
        2
    );
    assert_eq!(calls.load(Ordering::SeqCst), 16);
    service.shutdown().await;
    assert!(
        service
            .states()
            .iter()
            .filter(|state| state.feed_id.as_str() != "all")
            .all(|state| state.status == FeedWorkerStatus::Cancelled)
    );
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, "FEED_SYNC_UPDATED");
    }
    server.abort();
}

#[tokio::test(start_paused = true)]
async fn transient_fetches_retry_three_times_with_owned_cancellable_delays() {
    let calls = Arc::new(AtomicUsize::new(0));
    let worker = {
        let calls = calls.clone();
        tokio::spawn(fetch_with_retry(move || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Err("SERVER:503: unavailable".to_owned()) }
        }))
    };
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tokio::time::advance(std::time::Duration::from_secs(2)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    tokio::time::advance(std::time::Duration::from_secs(4)).await;
    assert_eq!(worker.await.unwrap(), Err("SERVER:503: unavailable".into()));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test(start_paused = true)]
async fn cancelling_backoff_prevents_later_fetch_attempts() {
    let calls = Arc::new(AtomicUsize::new(0));
    let worker = {
        let calls = calls.clone();
        tokio::spawn(fetch_with_retry(move || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Err("TIMEOUT: request".to_owned()) }
        }))
    };
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    tokio::time::advance(std::time::Duration::from_secs(100)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn permanent_fetch_failures_are_not_retried() {
    for error in [
        "SERVER:404: missing",
        "PARSE: invalid feed",
        "sqlite: invalid data",
    ] {
        let calls = AtomicUsize::new(0);
        let result = fetch_with_retry(|| {
            calls.fetch_add(1, Ordering::SeqCst);
            std::future::ready(Err(error.to_owned()))
        })
        .await;
        assert_eq!(result, Err(error.to_owned()));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn deleting_a_feed_cancels_its_fetch_and_stale_schedule_rows_cannot_restart_it() {
    let (_dir, service, calls, started, release, server, _events) = fixture().await;
    let stale = service.db.feed_get("feed").unwrap().unwrap();
    service.queue(Some("feed".into())).unwrap();
    started.acquire().await.unwrap().forget();
    assert!(service.delete("feed").await.unwrap());
    assert!(service.db.feed_get("feed").unwrap().is_none());
    assert!(service.states().is_empty());
    assert!(service.state.lock().unwrap().deleting.is_empty());
    service.queue_feeds(vec![stale], false, false).unwrap();
    assert!(service.states().is_empty());
    release.add_permits(1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(service.db.feed_entry_count("all:true").unwrap(), 0);
    assert!(!service.delete("feed").await.unwrap());
    service.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn deletion_admission_blocks_manual_and_automatic_queueing_until_cleanup() {
    let (_dir, service, calls, _started, _release, server, _events) = fixture().await;
    let row = service.db.feed_get("feed").unwrap().unwrap();
    service.state.lock().unwrap().deleting.insert("feed".into());
    let deletion = Deletion {
        service: &service,
        id: "feed",
    };
    service.queue(Some("feed".into())).unwrap();
    service.queue_feeds(vec![row], false, false).unwrap();
    assert!(service.states().is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(service.delete("feed").await.is_err());
    drop(deletion);
    assert!(service.state.lock().unwrap().deleting.is_empty());
    assert!(service.delete("feed").await.unwrap());
    service.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn feed_changes_reach_the_host_and_reconnect_recovers_a_full_snapshot() {
    let (_dir, service, _calls, started, release, server, mut events) = fixture().await;
    let (generation, mut requests) = service.host.connect();
    service.queue(Some("feed".into())).unwrap();
    let first = requests.recv().await.unwrap();
    assert_eq!(first["method"], "feedSyncChanged");
    assert_eq!(first["params"]["replace"], false);
    assert_eq!(first["params"]["states"][0]["feedId"], "feed");
    service
        .host
        .reply(
            generation,
            serde_json::json!({"id":first["id"],"result":null}),
        )
        .unwrap();
    started.acquire().await.unwrap().forget();
    let running = service.state.lock().unwrap().tasks["feed"]
        .state
        .subscribe();
    release.add_permits(1);
    assert_eq!(finished(running).await.status, FeedWorkerStatus::Completed);
    let (replacement, mut restored) = service.host.connect();
    service.notify_snapshot().unwrap();
    let snapshot = restored.recv().await.unwrap();
    assert_eq!(snapshot["params"]["replace"], true);
    assert_eq!(snapshot["params"]["states"][0]["status"], "COMPLETED");
    service
        .host
        .reply(
            replacement,
            serde_json::json!({"id":snapshot["id"],"result":null}),
        )
        .unwrap();
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, "FEED_SYNC_UPDATED");
    }
    service.shutdown().await;
    server.abort();
}

#[test]
fn feed_states_keep_enum_wire_names_and_active_boundaries() {
    for (status, wire, active) in [
        (FeedWorkerStatus::Queued, "QUEUED", true),
        (FeedWorkerStatus::Running, "RUNNING", true),
        (FeedWorkerStatus::Cancelled, "CANCELLED", false),
        (FeedWorkerStatus::Completed, "COMPLETED", false),
        (FeedWorkerStatus::Error, "ERROR", false),
    ] {
        let state = FeedSyncState {
            feed_id: "feed".into(),
            task_id: "task".into(),
            revision: crate::infrastructure::types::Long(1),
            status,
            error: String::new(),
        };
        assert_eq!(state.active(), active);
        let value = serde_json::to_value(&state).unwrap();
        assert_eq!(value["status"], wire);
        assert_eq!(
            serde_json::from_value::<FeedSyncState>(value)
                .unwrap()
                .status,
            status
        );
    }
    assert!(
        serde_json::from_value::<FeedWorkerStatus>(serde_json::json!("FUTURE_STATUS")).is_err()
    );
}
