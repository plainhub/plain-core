use super::*;

#[tokio::test]
async fn database_contention_yields_before_waiting_for_feed_schedule_rows() {
    use std::{future::Future, task::Poll};
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let held = db.clone();
    let (locked, ready) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        held.with_conn(|_| {
            locked.send(()).unwrap();
            let _ = released.recv_timeout(Duration::from_secs(3));
        });
    });
    ready.recv().unwrap();
    let settings = Settings::default();
    let mut read = Box::pin(scheduled_feeds(db, &settings, 0));
    let yielded =
        std::future::poll_fn(|context| Poll::Ready(read.as_mut().poll(context).is_pending())).await;
    let _ = release.send(());
    holder.join().unwrap();
    assert!(
        yielded,
        "Database contention must not block the async runtime"
    );
    assert!(read.await.unwrap().0.is_empty());
}

fn feed(last: Option<&str>) -> FeedRow {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("feeds.db")).unwrap();
    let mut feed = db
        .feed_save(
            "feed",
            "Feed",
            "https://example.com/feed",
            false,
            &super::super::super::now(),
        )
        .unwrap();
    feed.last_sync_at = last.map(str::to_owned);
    feed
}
#[test]
fn restart_enqueues_one_current_refresh_instead_of_replaying_missed_periods() {
    let settings = Settings {
        feed_auto_refresh_interval: 3600,
        ..Default::default()
    };
    let last = "2026-01-01T00:00:00Z";
    let now = chrono::DateTime::parse_from_rfc3339("2026-02-01T00:00:00Z")
        .unwrap()
        .timestamp_millis();
    let (tasks, _) = due(&settings, vec![feed(Some(last))], now).unwrap();
    assert_eq!(tasks.len(), 1);
    let mut refreshed = tasks[0].clone();
    refreshed.last_sync_at = Some(
        chrono::DateTime::from_timestamp_millis(now)
            .unwrap()
            .to_rfc3339(),
    );
    let (tasks, next) = due(&settings, vec![refreshed], now).unwrap();
    assert!(tasks.is_empty());
    assert_eq!(next, Some(now + 3_600_000));
}
#[test]
fn disabled_automatic_refresh_produces_no_tasks() {
    let settings = Settings {
        feed_auto_refresh: false,
        ..Default::default()
    };
    let (tasks, next) = due(&settings, vec![feed(None)], 0).unwrap();
    assert!(tasks.is_empty());
    assert_eq!(next, None);
}
#[test]
fn network_policy_restricts_only_automatic_execution_and_never_treats_cellular_as_wifi() {
    for network in [
        NetworkType::Wifi,
        NetworkType::Ethernet,
        NetworkType::Cellular,
        NetworkType::Other,
        NetworkType::None,
    ] {
        assert_eq!(allowed(true, network), network == NetworkType::Wifi);
        assert_eq!(allowed(false, network), network != NetworkType::None);
    }
}
#[test]
fn due_boundaries_and_clock_rollback_are_defined_without_clock_timing_assertions() {
    assert_eq!(next_at(None, 60, 1000).unwrap(), 1000);
    assert_eq!(
        next_at(Some("1970-01-01T00:00:01Z"), 60, 1000).unwrap(),
        61_000
    );
    assert_eq!(
        next_at(Some("2026-01-01T00:00:00Z"), 60, 1000).unwrap(),
        1000
    );
    assert!(next_at(None, 0, 1000).is_err());
    assert!(next_at(Some("invalid"), 60, 1000).is_err());
}
#[test]
fn retry_policy_is_limited_to_transient_failures() {
    for error in [
        "TIMEOUT: request",
        "DNS: connection",
        "SERVER:429: limited",
        "SERVER:503: unavailable",
    ] {
        assert!(retryable(error));
    }
    for error in [
        "SERVER:403: denied",
        "SERVER:404: missing",
        "PARSE: invalid",
        "UNKNOWN: malformed",
        "sqlite: busy",
    ] {
        assert!(!retryable(error));
    }
}
#[tokio::test]
async fn automatic_refreshes_deduplicate_and_manual_requests_survive_automatic_cancellation() {
    for manual in [false, true] {
        let (_dir, service, calls, started, release, server, _events) =
            super::super::tests::fixture().await;
        let feed = service.db.feed_get("feed").unwrap().unwrap();
        service
            .queue_feeds(vec![feed.clone()], false, false)
            .unwrap();
        let pending = service.state.lock().unwrap().tasks["feed"]
            .state
            .subscribe();
        let task_id = pending.borrow().task_id.clone();
        started.acquire().await.unwrap().forget();
        service.queue_feeds(vec![feed], false, false).unwrap();
        assert_eq!(
            service.state.lock().unwrap().tasks["feed"]
                .state
                .borrow()
                .task_id,
            task_id
        );
        if manual {
            service.queue(Some("feed".into())).unwrap();
        }
        service.cancel_automatic();
        if manual {
            release.add_permits(1);
        }
        assert_eq!(
            super::super::tests::finished(pending).await.status,
            if manual {
                FeedWorkerStatus::Completed
            } else {
                FeedWorkerStatus::Cancelled
            }
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        service.shutdown().await;
        server.abort();
    }
}

#[tokio::test]
async fn disabling_automatic_dispatch_cancels_queued_work_but_manual_refresh_still_runs() {
    let (_dir, service, calls, started, release, server, _events) =
        super::super::tests::fixture().await;
    let permits = service.permits.acquire_many(16).await.unwrap();
    let feed = service.db.feed_get("feed").unwrap().unwrap();
    service.queue_feeds(vec![feed], false, false).unwrap();
    let pending = service.state.lock().unwrap().tasks["feed"]
        .state
        .subscribe();
    service.cancel_automatic();
    assert!(service.queue(Some("feed".into())).is_err());
    assert_eq!(
        super::super::tests::finished(pending).await.status,
        FeedWorkerStatus::Cancelled
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    drop(permits);
    service.queue(Some("feed".into())).unwrap();
    started.acquire().await.unwrap().forget();
    let state = service.state.lock().unwrap().tasks["feed"]
        .state
        .subscribe();
    release.add_permits(1);
    assert_eq!(
        super::super::tests::finished(state).await.status,
        FeedWorkerStatus::Completed
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    service.shutdown().await;
    server.abort();
}
#[tokio::test]
async fn a_network_policy_change_cancels_only_automatic_fetches() {
    for manual in [false, true] {
        let (_dir, service, _calls, started, release, server, _events) =
            super::super::tests::fixture().await;
        let feed = service.db.feed_get("feed").unwrap().unwrap();
        service.queue_feeds(vec![feed], false, false).unwrap();
        let pending = service.state.lock().unwrap().tasks["feed"]
            .state
            .subscribe();
        started.acquire().await.unwrap().forget();
        if manual {
            service.queue(Some("feed".into())).unwrap();
        }
        service.cancel_automatic();
        if manual {
            release.add_permits(1);
        }
        assert_eq!(
            super::super::tests::finished(pending).await.status,
            if manual {
                FeedWorkerStatus::Completed
            } else {
                FeedWorkerStatus::Cancelled
            }
        );
        service.shutdown().await;
        server.abort();
    }
}
