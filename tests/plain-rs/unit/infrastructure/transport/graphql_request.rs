use super::*;

fn id(time: i64, random: u8) -> RequestId {
    let mut bytes = [random; 16];
    bytes[..6].copy_from_slice(&time.to_be_bytes()[2..]);
    bytes[6] = (bytes[6] & 15) | 0x70;
    bytes[8] = (bytes[8] & 63) | 0x80;
    RequestId::parse(&Uuid::from_bytes(bytes).to_string()).unwrap()
}

#[test]
fn common_envelope_preserves_graphql_fields_and_replaces_caller_id() {
    let input = r#"{"requestId":"caller","query":"query Q { x }","variables":{"text":"a|b"},"operationName":"Q","extensions":{"test":true}}"#;
    let first = Envelope::new(input).unwrap();
    let second = Envelope::new(input).unwrap();
    assert_ne!(first.id(), second.id());
    assert!(first.id().is_some());
    let decoded = Envelope::parse(&first.json().unwrap()).unwrap();
    assert_eq!(decoded.request_id, first.request_id);
    assert_eq!(decoded.content, first.content);
    assert_eq!(decoded.content["variables"]["text"], "a|b");
    assert_eq!(decoded.content["operationName"], "Q");
    assert_eq!(decoded.content["extensions"]["test"], true);
    for bad in [
        "null",
        "[]",
        "invalid",
        r#"{"query":"x"}"#,
        r#"{"requestId":null}"#,
        r#"{"requestId":"a","requestId":"b"}"#,
    ] {
        assert!(Envelope::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn uuid_validation_and_freshness_are_shared_and_canonical() {
    let request = id(10_000_000, 0xab);
    assert_eq!(request.timestamp(), 10_000_000);
    assert_eq!(
        RequestId::parse(&request.to_string().to_uppercase()),
        Some(request)
    );
    for bad in [
        "",
        "not-a-uuid",
        "00000000-0000-4000-8000-000000000000",
        "00000000-0000-7000-c000-000000000000",
        "00000000000070008000000000000000",
    ] {
        assert!(RequestId::parse(bad).is_none());
    }
    for window in [60_000, 1_800_000] {
        assert!(request.is_fresh(10_000_000 - window, window as u64));
        assert!(request.is_fresh(10_000_000 + window, window as u64));
        assert!(!request.is_fresh(10_000_000 - window - 1, window as u64));
        assert!(!request.is_fresh(10_000_000 + window + 1, window as u64));
    }
    assert!(!request.is_fresh(-1, u64::MAX));
}

#[test]
fn persistent_ids_survive_reopen_and_keep_client_and_scope_isolation() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("replay.db");
    let now = 10_000_000;
    let request = id(now, 0xab);
    let db = Db::open(&path).unwrap();
    assert!(
        request
            .admit_persistent(&db, "guest", "share-a", now, 60_000)
            .unwrap()
    );
    let upper = RequestId::parse(&request.to_string().to_uppercase()).unwrap();
    assert!(
        !upper
            .admit_persistent(&db, "guest", "share-a", now, 60_000)
            .unwrap()
    );
    assert!(
        request
            .admit_persistent(&db, "guest", "share-b", now, 60_000)
            .unwrap()
    );
    assert!(
        request
            .admit_persistent(&db, "peer_graphql:c", "share-a", now, 60_000)
            .unwrap()
    );
    assert!(
        id(now, 0xac)
            .admit_persistent(&db, "guest", "share-a", now, 60_000)
            .unwrap()
    );
    drop(db);
    let db = Db::open(&path).unwrap();
    assert!(
        !request
            .admit_persistent(&db, "guest", "share-a", now, 60_000)
            .unwrap()
    );
    assert!(
        !id(now - 60_001, 1)
            .admit_persistent(&db, "guest", "share-a", now, 60_000)
            .unwrap()
    );
    assert!(
        !id(now + 60_001, 1)
            .admit_persistent(&db, "guest", "share-a", now, 60_000)
            .unwrap()
    );
}

#[test]
fn peer_outer_is_strict_and_preserves_exact_signed_request_bytes() {
    let inner =
        "{ \"requestId\": \"00000000-0000-7000-8000-000000000000\", \"variables\": {\"n\":1e+30} }";
    let outer = format!("{{\"signature\":\"s\",\"request\":{inner}}}");
    let parsed: SignedEnvelope = plain_libs::serde_json::from_str(&outer).unwrap();
    assert_eq!(parsed.request.get(), inner);
    for bad in [
        format!("{{\"signature\":\"s\",\"request\":{inner},\"query\":\"x\"}}"),
        format!("{{\"request\":{inner}}}"),
        inner.to_string(),
    ] {
        assert!(plain_libs::serde_json::from_str::<SignedEnvelope>(&bad).is_err());
    }
    assert!(Envelope::parse(&outer).is_err());
}
