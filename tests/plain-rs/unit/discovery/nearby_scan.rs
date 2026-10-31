use super::*;
use serde_json::json;
fn begin(scans: &Scans) -> String {
    scans.set_owner(1);
    let id = uuid::Uuid::new_v4();
    scans.report_state(1, id, true).unwrap();
    id.to_string()
}
fn payload(id: &str) -> String {
    json!({"id":id,"name":"fixture","deviceType":"PHONE","port":2443,"version":"1","platform":"android"}).to_string()
}

#[test]
fn only_active_unanswered_reads_have_a_host_owner() {
    let scans = Scans::default();
    let session = begin(&scans);
    let short = crate::discovery::nearby_wire::short_id("peer");
    let Step::Read { generation } = scans.seen(&session, &short).unwrap() else {
        panic!();
    };
    assert_eq!(scans.pending_owner(&session, &short, &generation), Some(1));
    assert_eq!(scans.pending_owner(&session, &short, "stale"), None);
    scans
        .reply(&session, &short, &generation, Some(&payload("peer")))
        .unwrap();
    assert_eq!(scans.pending_owner(&session, &short, &generation), None);
    let second = uuid::Uuid::new_v4();
    scans.report_state(1, second, true).unwrap();
    let Step::Read { generation } = scans.seen(&second.to_string(), &short).unwrap() else {
        panic!();
    };
    scans.report_state(1, second, false).unwrap();
    assert_eq!(
        scans.pending_owner(&second.to_string(), &short, &generation),
        None
    );
    scans.report_state(1, second, true).unwrap();
    let Step::Read { generation } = scans.seen(&second.to_string(), &short).unwrap() else {
        panic!();
    };
    scans.set_owner(2);
    assert_eq!(
        scans.pending_owner(&second.to_string(), &short, &generation),
        None
    );
}
#[test]
fn scan_identity_generation_throttle_and_session_cleanup_are_rust_owned() {
    let scans = Scans::default();
    let session = begin(&scans);
    let short = crate::discovery::nearby_wire::short_id("peer");
    let now = Instant::now();
    let Step::Read { generation } = scans.seen_at(&session, &short, now).unwrap() else {
        panic!()
    };
    assert!(matches!(
        scans.seen_at(&session, &short, now).unwrap(),
        Step::Wait
    ));
    assert!(
        scans
            .reply(&session, &short, "old", Some(&payload("peer")))
            .unwrap()
            .is_none()
    );
    assert!(
        scans
            .reply(&session, &short, &generation, Some(&payload("other")))
            .is_err()
    );
    let Step::Read { generation } = scans.seen_at(&session, &short, now).unwrap() else {
        panic!()
    };
    assert_eq!(
        scans
            .reply(&session, &short, &generation, Some(&payload("peer")))
            .unwrap()
            .unwrap()
            .id,
        "peer"
    );
    let emit = scans.0.lock().unwrap().scans[&session][&short]
        .last_emit
        .unwrap();
    assert!(matches!(
        scans
            .seen_at(&session, &short, emit + Duration::from_secs(5))
            .unwrap(),
        Step::Wait
    ));
    assert!(matches!(
        scans
            .seen_at(&session, &short, emit + Duration::from_millis(5001))
            .unwrap(),
        Step::Emit { .. }
    ));
    scans
        .report_state(1, session.parse().unwrap(), false)
        .unwrap();
    assert!(scans.seen(&session, &short).is_err());
    assert!(
        scans
            .reply(&session, &short, &generation, Some(&payload("peer")))
            .unwrap()
            .is_none()
    );
}
#[test]
fn abandoned_gatt_reads_can_be_reclaimed_without_accepting_old_completion() {
    let scans = Scans::default();
    let id = begin(&scans);
    let short = crate::discovery::nearby_wire::short_id("peer");
    let now = Instant::now();
    let Step::Read { generation: old } = scans.seen_at(&id, &short, now).unwrap() else {
        panic!()
    };
    let Step::Read { generation } = scans
        .seen_at(&id, &short, now + Duration::from_secs(30))
        .unwrap()
    else {
        panic!()
    };
    assert_ne!(old, generation);
    assert!(
        scans
            .reply(&id, &short, &old, Some(&payload("peer")))
            .unwrap()
            .is_none()
    );
    assert!(
        scans
            .reply(&id, &short, &generation, None)
            .unwrap()
            .is_none()
    );
}

#[test]
fn duplicate_scan_facts_preserve_pending_reads_and_capacity() {
    let scans = Scans::default();
    scans.set_owner(1);
    for index in 1..=16 {
        scans
            .report_state(1, uuid::Uuid::from_u128(index), true)
            .unwrap();
    }
    let id = uuid::Uuid::from_u128(1);
    let short = "0001020304050607";
    assert!(matches!(
        scans.seen(&id.to_string(), short).unwrap(),
        Step::Read { .. }
    ));
    scans.report_state(1, id, true).unwrap();
    assert!(matches!(
        scans.seen(&id.to_string(), short).unwrap(),
        Step::Wait
    ));
    assert!(
        scans
            .report_state(1, uuid::Uuid::from_u128(17), true)
            .is_err()
    );
    scans.report_state(1, id, false).unwrap();
    scans.report_state(1, id, false).unwrap();
    scans
        .report_state(1, uuid::Uuid::from_u128(17), true)
        .unwrap();
}

#[test]
fn host_replacement_and_close_reject_stale_lifecycle_and_read_results() {
    let scans = Scans::default();
    let id = begin(&scans);
    let short = "0001020304050607";
    let Step::Read { generation } = scans.seen(&id, short).unwrap() else {
        panic!()
    };
    scans.set_owner(2);
    assert!(scans.report_state(1, id.parse().unwrap(), true).is_err());
    assert!(
        scans
            .reply(&id, short, &generation, Some(&payload("peer")))
            .unwrap()
            .is_none()
    );
    scans.report_state(2, id.parse().unwrap(), true).unwrap();
    scans.close_owner(1);
    scans.set_owner(1);
    scans.set_owner(2);
    assert!(matches!(scans.seen(&id, short).unwrap(), Step::Read { .. }));
    scans.close_owner(2);
    scans.set_owner(2);
    assert!(scans.seen(&id, short).is_err());
    assert!(scans.report_state(2, id.parse().unwrap(), true).is_err());
}
