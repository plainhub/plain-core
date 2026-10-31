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
fn disabled_automatic_refresh_produces_no_tasks_or_system_wake() {
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
async fn revoking_one_automatic_owner_preserves_other_owners_and_manual_requests() {
    for manual in [false, true] {
        let (_dir, service, calls, started, release, server, _events) =
            super::super::tests::fixture().await;
        let feed = service.db.feed_get("feed").unwrap().unwrap();
        let mut waiters = service
            .queue_feeds(vec![feed.clone()], Some("first"), false)
            .unwrap();
        started.acquire().await.unwrap().forget();
        if manual {
            service.queue(Some("feed".into())).unwrap();
        } else {
            service
                .queue_feeds(vec![feed], Some("second"), false)
                .unwrap();
        }
        service.release_lease("first");
        assert!(!*service.state.lock().unwrap().tasks["feed"].cancel.borrow());
        release.add_permits(1);
        assert_eq!(
            super::super::tests::finished(waiters.pop().unwrap())
                .await
                .status,
            "COMPLETED"
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        service.shutdown().await;
        server.abort();
    }
}
#[tokio::test]
async fn revoking_the_last_automatic_owner_cancels_its_owned_fetch() {
    let (_dir, service, _calls, started, _release, server, _events) =
        super::super::tests::fixture().await;
    let feed = service.db.feed_get("feed").unwrap().unwrap();
    let mut waiters = service
        .queue_feeds(vec![feed], Some("only"), false)
        .unwrap();
    started.acquire().await.unwrap().forget();
    service.release_lease("only");
    assert_eq!(
        super::super::tests::finished(waiters.pop().unwrap())
            .await
            .status,
        "CANCELLED"
    );
    assert!(
        service
            .db
            .feed_get("feed")
            .unwrap()
            .unwrap()
            .last_sync_at
            .is_none()
    );
    service.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn disabling_automatic_dispatch_cancels_queued_work_but_manual_refresh_still_runs() {
    let (_dir, service, calls, started, release, server, _events) =
        super::super::tests::fixture().await;
    let permits = service.permits.acquire_many(16).await.unwrap();
    let feed = service.db.feed_get("feed").unwrap().unwrap();
    let mut pending = service
        .queue_feeds(vec![feed], Some("automatic"), false)
        .unwrap();
    service.apply_auto_policy(false, false);
    assert_eq!(
        super::super::tests::finished(pending.pop().unwrap())
            .await
            .status,
        "CANCELLED"
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
        "COMPLETED"
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
        let mut pending = service
            .queue_feeds(vec![feed], Some("automatic"), false)
            .unwrap();
        started.acquire().await.unwrap().forget();
        if manual {
            service.queue(Some("feed".into())).unwrap();
        }
        service.apply_auto_policy(false, true);
        if manual {
            release.add_permits(1);
        }
        assert_eq!(
            super::super::tests::finished(pending.pop().unwrap())
                .await
                .status,
            if manual { "COMPLETED" } else { "CANCELLED" }
        );
        service.shutdown().await;
        server.abort();
    }
}

#[tokio::test(start_paused = true)]
async fn execution_deadline_cancels_and_drains_only_its_orphaned_automatic_tasks() {
    for manual in [false, true] {
        let (_dir, service, calls, _started, _release, server, _events) =
            super::super::tests::fixture().await;
        let permits = service.permits.acquire_many(16).await.unwrap();
        let feed = service.db.feed_get("feed").unwrap().unwrap();
        let mut tasks = service
            .queue_feeds(vec![feed], Some("deadline"), false)
            .unwrap();
        if manual {
            service.queue(Some("feed".into())).unwrap();
        }
        let lease = register(&service, "deadline").unwrap();
        let mut done = lease.done.subscribe();
        let worker = tokio::spawn({
            let service = service.clone();
            async move {
                execute_lease(
                    service,
                    "deadline",
                    tokio::time::Instant::now() + Duration::from_secs(30),
                    lease,
                    std::future::pending::<Result<bool>>(),
                )
                .await
            }
        });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(30)).await;
        while !*done.borrow() {
            done.changed().await.unwrap();
        }
        assert!(!worker.await.unwrap());
        assert!(!service.leases.lock().unwrap().contains_key("deadline"));
        if manual {
            assert_eq!(tasks[0].borrow().status, "QUEUED");
        } else {
            assert_eq!(
                super::super::tests::finished(tasks.pop().unwrap())
                    .await
                    .status,
                "CANCELLED"
            );
        }
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        service.shutdown().await;
        drop(permits);
        server.abort();
    }
}

#[tokio::test]
async fn revocation_before_execution_does_not_poll_work_and_stopped_service_rejects_grants() {
    let (_dir, service, _calls, _started, _release, server, _events) =
        super::super::tests::fixture().await;
    let lease = register(&service, "revoked").unwrap();
    assert!(register(&service, "revoked").is_err());
    lease.revoke.send_replace(true);
    let polled = std::sync::atomic::AtomicUsize::new(0);
    execute_lease(
        service.clone(),
        "revoked",
        tokio::time::Instant::now() + Duration::from_secs(30),
        lease,
        async {
            polled.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(true)
        },
    )
    .await;
    assert_eq!(polled.load(std::sync::atomic::Ordering::SeqCst), 0);
    service.shutdown().await;
    assert!(register(&service, "after-stop").is_err());
    server.abort();
}
