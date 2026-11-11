use super::*;

fn id(millis: u64, random: u8) -> Uuid {
    let mut bytes = [random; 16];
    bytes[..6].copy_from_slice(&millis.to_be_bytes()[2..]);
    bytes[6] = (bytes[6] & 15) | 0x70;
    bytes[8] = (bytes[8] & 63) | 0x80;
    Uuid::from_bytes(bytes)
}

fn cache(wall: i64, now: Instant) -> Cache {
    Cache {
        sessions: HashMap::new(),
        total: 0,
        last_cleanup: now,
        clock_wall: wall,
        clock_instant: now,
    }
}

#[test]
fn freshness_accepts_both_boundaries_and_rejects_the_next_millisecond() {
    let now = Instant::now();
    let wall = 10_000_000;
    for offset in [-(WINDOW_MS as i64), 0, WINDOW_MS as i64] {
        assert!(
            cache(wall, now)
                .admit([1; 32], id((wall + offset) as u64, 0), wall, now, 10, 10)
                .is_ok()
        );
    }
    for offset in [-(WINDOW_MS as i64) - 1, WINDOW_MS as i64 + 1] {
        assert_eq!(
            cache(wall, now).admit([1; 32], id((wall + offset) as u64, 0), wall, now, 10, 10),
            Err(StatusCode::BAD_REQUEST)
        );
    }
}

#[test]
fn duplicates_do_not_refresh_retention_and_sessions_are_isolated() {
    let now = Instant::now();
    let wall = 10_000_000;
    let mut cache = cache(wall, now);
    let request = id(wall as u64, 0);
    assert!(cache.admit([1; 32], request, wall, now, 10, 10).is_ok());
    assert_eq!(
        cache.admit(
            [1; 32],
            request,
            wall + 1000,
            now + Duration::from_secs(1),
            10,
            10
        ),
        Err(StatusCode::BAD_REQUEST)
    );
    assert_eq!(cache.sessions[&[1; 32]][&request], now);
    assert!(cache.admit([2; 32], request, wall, now, 10, 10).is_ok());
}

#[test]
fn expiry_is_lazy_and_keeps_the_inclusive_full_hour_boundary() {
    let now = Instant::now();
    let wall = 10_000_000;
    let mut cache = cache(wall, now);
    let future = id(wall as u64 + WINDOW_MS, 0);
    cache.admit([1; 32], future, wall, now, 10, 10).unwrap();
    assert_eq!(
        cache.admit([1; 32], future, wall + 3_600_000, now + RETENTION, 10, 10),
        Err(StatusCode::BAD_REQUEST)
    );
    assert_eq!(cache.total, 1);
    cache
        .admit(
            [1; 32],
            id((wall + 3_600_001) as u64, 1),
            wall + 3_600_001,
            now + RETENTION + Duration::from_millis(1),
            10,
            10,
        )
        .unwrap();
    assert!(cache.sessions[&[1; 32]].contains_key(&future));
    cache
        .admit(
            [1; 32],
            id((wall + 3_660_000) as u64, 2),
            wall + 3_660_000,
            now + RETENTION + CLEANUP_INTERVAL,
            10,
            10,
        )
        .unwrap();
    assert!(!cache.sessions[&[1; 32]].contains_key(&future));
}

#[test]
fn capacity_fails_closed_without_evicting_live_ids() {
    let now = Instant::now();
    let wall = 10_000_000;
    let mut cache = cache(wall, now);
    let first = id(wall as u64, 0);
    cache.admit([1; 32], first, wall, now, 1, 2).unwrap();
    assert_eq!(
        cache.admit([1; 32], id(wall as u64, 1), wall, now, 1, 2),
        Err(StatusCode::TOO_MANY_REQUESTS)
    );
    cache.admit([2; 32], first, wall, now, 1, 2).unwrap();
    assert_eq!(
        cache.admit([3; 32], first, wall, now, 1, 2),
        Err(StatusCode::TOO_MANY_REQUESTS)
    );
    assert_eq!(
        cache.admit([1; 32], first, wall, now, 1, 2),
        Err(StatusCode::BAD_REQUEST)
    );
    assert_eq!(cache.total, 2);
}

#[test]
fn a_backward_clock_cannot_revive_an_expired_request() {
    let now = Instant::now();
    let wall = 10_000_000;
    let mut cache = cache(wall, now);
    let first = id(wall as u64 + WINDOW_MS, 0);
    cache.admit([1; 32], first, wall, now, 10, 10).unwrap();
    let later = now + RETENTION + CLEANUP_INTERVAL;
    cache
        .admit(
            [1; 32],
            id((wall + 3_660_000) as u64, 1),
            wall + 3_660_000,
            later,
            10,
            10,
        )
        .unwrap();
    assert!(!cache.sessions[&[1; 32]].contains_key(&first));
    assert_eq!(
        cache.admit([1; 32], first, wall, later, 10, 10),
        Err(StatusCode::BAD_REQUEST)
    );
}

#[test]
fn forward_adjustments_keep_the_new_time_floor() {
    let now = Instant::now();
    let mut cache = cache(10_000_000, now);
    cache
        .admit([1; 32], id(20_000_000, 0), 20_000_000, now, 10, 10)
        .unwrap();
    assert_eq!(
        cache.admit(
            [1; 32],
            id(10_000_000, 1),
            10_000_000,
            now + Duration::from_secs(1),
            10,
            10
        ),
        Err(StatusCode::BAD_REQUEST)
    );
}

#[test]
fn sub_millisecond_expiry_does_not_reopen_the_last_valid_millisecond() {
    let now = Instant::now();
    let wall = 10_000_000;
    let mut cache = cache(wall, now);
    let request = id(wall as u64 + WINDOW_MS, 0);
    cache.admit([1; 32], request, wall, now, 10, 10).unwrap();
    assert_eq!(
        cache.admit(
            [1; 32],
            request,
            wall + 3_600_000,
            now + RETENTION + Duration::from_nanos(1),
            10,
            10
        ),
        Err(StatusCode::BAD_REQUEST)
    );
    assert_eq!(cache.total, 1);
}

#[test]
fn lazy_cleanup_reclaims_sparse_session_allocations() {
    let now = Instant::now();
    let wall = 10_000_000;
    let mut cache = cache(wall, now);
    for random in 0..200 {
        cache
            .admit([1; 32], id(wall as u64, random), wall, now, 1000, 1000)
            .unwrap();
    }
    let capacity = cache.sessions[&[1; 32]].capacity();
    let later = now + RETENTION;
    cache
        .admit(
            [1; 32],
            id((wall + 3_600_000) as u64, 0),
            wall + 3_600_000,
            later,
            1000,
            1000,
        )
        .unwrap();
    cache
        .admit(
            [1; 32],
            id((wall + 3_660_000) as u64, 0),
            wall + 3_660_000,
            later + CLEANUP_INTERVAL,
            1000,
            1000,
        )
        .unwrap();
    assert_eq!(cache.total, 2);
    assert!(cache.sessions[&[1; 32]].capacity() < capacity / 4);
}
