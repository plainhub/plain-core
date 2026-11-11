use super::*;
use plain_libs::serde_json::json;

#[test]
fn canonical_messages_round_trip_and_reject_unknown_or_malformed_payloads() {
    for value in [
        json!({"kind":"DISCOVER"}),
        json!({"kind":"PAIR_CANCEL","payload":{"fromId":"a","toId":"b","requestSignature":"request","signaturePublicKey":"public","timestamp":1,"signature":"proof"}}),
        json!({"kind":"DISCOVER_REPLY","payload":{"id":"peer","name":"Fixture","port":1234,"deviceType":"PHONE","version":"1","signaturePublicKey":"test-public-key","platform":"test"}}),
    ] {
        let message: Message = plain_libs::serde_json::from_value(value).unwrap();
        let expected = plain_libs::serde_json::to_value(&message).unwrap();
        assert_eq!(
            plain_libs::serde_json::to_value(Message::parse(&message.wire().unwrap()).unwrap()).unwrap(),
            expected
        );
    }
    assert_eq!(Message::Discover.wire().unwrap(), "DISCOVER:");
    for bad in [
        "UNKNOWN:{}",
        "PAIR_CANCEL:{}",
        "PAIR_CANCEL:bad",
        "no prefix",
        "PAIR_REQUEST:{}",
        "PAIR_RESPONSE:{}",
    ] {
        assert!(Message::parse(bad).is_err(), "{bad}");
    }
}
#[test]
fn discover_identity_matches_the_full_advertised_uuid_before_becoming_a_peer() {
    let id = "706c6169-2d74-8e53-9a16-f08b62c591d3";
    let body = json!({"id":id,"name":"Fixture","port":1234,"deviceType":"PHONE","version":"1","signaturePublicKey":"test-public-key","platform":"test","awareSupported":true,"awareRunning":false}).to_string();
    let reply = discover_reply(&body, id).unwrap();
    assert_eq!(reply.id, id);
    assert!(reply.aware_supported);
    assert!(!reply.aware_running);
    assert!(discover_reply(&body, "706c6169-b903-841f-a6ce-32d75908f4ab").is_err());
    assert!(discover_reply("{}", id).is_err());
    let invalid = body.replace(id, "legacy");
    assert!(discover_reply(&invalid, "legacy").is_err());
}
