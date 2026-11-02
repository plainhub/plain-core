use super::*;

#[test]
fn count_threshold_commits_once_after_fifty_records() {
    let now = Instant::now();
    let mut batch = WriteBatch::default();
    for value in 0..50 {
        batch.push(value, now);
    }
    assert!(!batch.is_due(now));
    assert!(
        !batch
            .flush_due(now, |_| -> Result<(), ()> { panic!("premature write") })
            .unwrap()
    );
    batch.push(50, now);
    let mut writes = 0;
    assert!(
        batch
            .flush_due(now, |values| {
                writes += 1;
                assert_eq!(values, (0..51).collect::<Vec<_>>());
                Ok::<_, ()>(())
            })
            .unwrap()
    );
    assert_eq!(writes, 1);
    assert!(batch.pending().is_empty());
    assert!(
        !batch
            .flush(|_| -> Result<(), ()> { panic!("empty write") })
            .unwrap()
    );
}

#[test]
fn age_threshold_uses_the_first_pending_record_and_resets_after_commit() {
    let now = Instant::now();
    let mut batch = WriteBatch::default();
    batch.push(1, now);
    batch.push(2, now + Duration::from_secs(30));
    assert!(!batch.is_due(now + Duration::from_secs(59)));
    assert!(
        batch
            .flush_due(now + Duration::from_secs(60), |values| {
                assert_eq!(values, [1, 2]);
                Ok::<_, ()>(())
            })
            .unwrap()
    );
    batch.push(3, now + Duration::from_secs(70));
    assert!(!batch.is_due(now + Duration::from_secs(129)));
    assert!(batch.is_due(now + Duration::from_secs(130)));
}

#[test]
fn failed_writes_preserve_order_and_pending_records_for_a_later_commit() {
    let now = Instant::now();
    let mut batch = WriteBatch::new(1, Duration::from_secs(60));
    batch.push("old", now);
    batch.push("new", now);
    assert_eq!(
        batch.flush_due(now, |_| Err("storage failure")),
        Err("storage failure")
    );
    assert_eq!(batch.pending(), ["old", "new"]);
    batch.push("latest", now);
    assert!(
        batch
            .flush(|values| {
                assert_eq!(values, ["old", "new", "latest"]);
                Ok::<_, ()>(())
            })
            .unwrap()
    );
    assert!(batch.pending().is_empty());
}

#[test]
fn removing_the_last_pending_record_resets_the_deadline() {
    let now = Instant::now();
    let mut batch = WriteBatch::default();
    batch.push(1, now);
    batch.retain(|_| false);
    assert!(!batch.is_due(now + Duration::from_secs(90)));
    batch.push(2, now + Duration::from_secs(90));
    assert!(!batch.is_due(now + Duration::from_secs(149)));
    assert!(batch.is_due(now + Duration::from_secs(150)));
}
