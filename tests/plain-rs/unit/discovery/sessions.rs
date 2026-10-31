use super::*;

#[test]
fn repeated_acquisition_is_idempotent_and_other_views_remain_active_after_release() {
    let mut sessions = DiscoverySessions::default();
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    sessions.acquire("first", first).unwrap();
    sessions.acquire("first", first).unwrap();
    sessions.acquire("second", second).unwrap();
    sessions.release("first", first).unwrap();
    sessions.release("first", first).unwrap();
    assert!(sessions.active());
    sessions.release("second", second).unwrap();
    assert!(!sessions.active());
}

#[test]
fn clients_cannot_acquire_or_release_another_clients_token() {
    let mut sessions = DiscoverySessions::default();
    let token = Uuid::from_u128(1);
    sessions.acquire("first", token).unwrap();
    assert!(sessions.acquire("second", token).is_err());
    assert!(sessions.release("second", token).is_err());
    assert!(sessions.active());
    assert!(!sessions.release_owner("second"));
    assert!(sessions.release_owner("first"));
    assert!(!sessions.active());
}

#[test]
fn disconnect_removes_only_the_disconnected_clients_sessions_and_allows_reconnect() {
    let mut sessions = DiscoverySessions::default();
    let first = Uuid::from_u128(1);
    sessions.acquire("first", first).unwrap();
    sessions.acquire("first", Uuid::from_u128(2)).unwrap();
    sessions.acquire("second", Uuid::from_u128(3)).unwrap();
    assert!(sessions.release_owner("first"));
    assert!(sessions.active());
    sessions.acquire("first", first).unwrap();
    sessions.release_owner("second");
    assert!(sessions.active());
    sessions.release_owner("first");
    assert!(!sessions.active());
}

#[test]
fn session_limits_reject_new_owners_without_losing_existing_ownership() {
    let mut sessions = DiscoverySessions::default();
    assert!(sessions.acquire("", Uuid::from_u128(1)).is_err());
    for index in 0..OWNER_CAPACITY {
        sessions
            .acquire("first", Uuid::from_u128(index as u128 + 1))
            .unwrap();
    }
    assert!(sessions.acquire("first", Uuid::from_u128(1000)).is_err());
    sessions.acquire("first", Uuid::from_u128(1)).unwrap();
    for index in OWNER_CAPACITY..SESSION_CAPACITY {
        sessions
            .acquire(
                &format!("owner-{index}"),
                Uuid::from_u128(index as u128 + 1),
            )
            .unwrap();
    }
    assert!(sessions.acquire("overflow", Uuid::from_u128(2000)).is_err());
    assert!(sessions.active());
}
