use super::*;
#[test]
fn rejects_duplicate_stale_overflow_and_invalid_nonce_formats() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("replay.sqlite");
    let db = Db::open(&path).unwrap();
    let guard = Replay::new("main");
    let wrap = |timestamp: i64, nonce: &str| format!("{timestamp}|{nonce}|{{\"query\":\"a|b\"}}");
    let valid = "00112233445566778899aabbccddeeff";
    assert_eq!(
        guard.admit(&db, "a", &wrap(100000, valid), 100000).unwrap(),
        "{\"query\":\"a|b\"}"
    );
    assert!(
        guard
            .admit(&db, "a", &wrap(100000, &valid.to_uppercase()), 100000)
            .is_err()
    );
    assert!(guard.admit(&db, "b", &wrap(100000, valid), 100000).is_ok());
    for nonce in [
        "",
        "nonce",
        "g0112233445566778899aabbccddeeff",
        "00112233-4455-6677-8899-aabbccddeefff",
    ] {
        assert!(guard.admit(&db, "a", &wrap(100000, nonce), 100000).is_err());
    }
    assert!(
        guard
            .admit(
                &db,
                "a",
                &wrap(100000, "00112233-4455-4677-8899-aabbccddeeff"),
                100000
            )
            .is_ok()
    );
    assert!(
        guard
            .admit(&db, "a", &wrap(i64::MIN, valid), 100000)
            .is_err()
    );
    assert!(
        guard
            .admit(
                &db,
                "a",
                &format!("99999999999999999999|{valid}|{{}}"),
                100000
            )
            .is_err()
    );
    assert!(guard.admit(&db, "a", &wrap(39999, valid), 100000).is_err());
    assert!(guard.admit(&db, "a", &wrap(40000, valid), 100000).is_ok());
    assert!(guard.admit(&db, "a", "{}", 100000).is_err());
    drop(db);
    let db = Db::open(&path).unwrap();
    assert!(guard.admit(&db, "a", &wrap(100000, valid), 100000).is_err());
}

#[test]
fn subscription_accepts_only_full_nonce_and_rejects_replay() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("subscription.sqlite")).unwrap();
    let guard = Replay::new("subscription");
    let nonce = "ab".repeat(32);
    let plain = format!("100000|{nonce}|");
    assert!(Replay::new("main").admit(&db, "a", &plain, 100000).is_err());
    assert!(
        Replay::new("guest")
            .admit(&db, "a", &plain, 100000)
            .is_err()
    );
    assert!(
        Replay::new("websocket")
            .admit(&db, "a", &plain, 100000)
            .is_err()
    );
    assert_eq!(guard.admit(&db, "a", &plain, 100000), Ok(""));
    assert!(guard.admit(&db, "a", &plain, 100000).is_err());
    assert!(
        guard
            .admit(&db, "a", &plain.to_uppercase(), 100000)
            .is_err()
    );
    for invalid in ["ab".repeat(16), "ab".repeat(24), "gg".repeat(32)] {
        assert!(
            guard
                .admit(&db, "a", &format!("100000|{invalid}|"), 100000)
                .is_err()
        );
    }
}
