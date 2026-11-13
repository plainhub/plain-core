use super::*;

#[test]
fn timestamp_ok_within_5min() {
    let now = now_ms();
    assert!(timestamp_ok(now));
    assert!(timestamp_ok(now - 4 * 60 * 1000));
    assert!(timestamp_ok(now + 4 * 60 * 1000));
    assert!(!timestamp_ok(now - 6 * 60 * 1000));
    assert!(!timestamp_ok(now + 6 * 60 * 1000));
}

#[test]
fn prefer_sender_ip_returns_one_address() {
    let ips = vec![
        "192.0.2.2".to_string(),
        "192.0.2.2".to_string(),
        "".to_string(),
    ];
    assert_eq!(prefer_sender_ip(&ips, "192.0.2.3"), "192.0.2.3");
    // Sender already in the list → listed once, first.
    assert_eq!(prefer_sender_ip(&ips, "192.0.2.2"), "192.0.2.2");
    assert_eq!(prefer_sender_ip(&ips, ""), "192.0.2.2");
    assert_eq!(prefer_sender_ip(&[], ""), "");
}
