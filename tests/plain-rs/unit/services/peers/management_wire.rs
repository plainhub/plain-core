use super::*;

fn streams(key: &[u8], client: &str) -> (graphql::Stream, graphql::Stream) {
    let init = registration(key, client, 1_700_000_000_123).unwrap();
    let ack = graphql::seal_ack(
        key,
        client,
        &init,
        br#"{"serverNonce":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}"#,
    )
    .unwrap();
    (
        graphql::Stream::new(client, &init, &ack, Role::Server).unwrap(),
        acknowledge(key, client, &init, &ack).unwrap(),
    )
}

#[test]
fn registration_binds_identity_and_keeps_timestamp() {
    let key = [7; 32];
    let init = registration(&key, "client", 1_700_000_000_123).unwrap();
    let plain = graphql::open_init(&key, "client", &init).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&plain).unwrap();
    assert_eq!(value["timestampMillis"], 1_700_000_000_123i64);
    assert!(graphql::open_init(&key, "other", &init).is_none());
    assert!(graphql::open_init(&[8; 32], "client", &init).is_none());
    assert!(registration(&[7; 31], "client", 0).is_err());
}

#[test]
fn subscription_messages_authenticate_and_reject_replays() {
    let key = [7; 32];
    let (mut sender, mut receiver) = streams(&key, "client");
    let ready = br#"{"type":"next","id":"clipboardChanged","payload":{"data":null,"extensions":{"subscriptionReady":true}}}"#;
    let frame = sender.encode(ready, &key).unwrap();
    assert!(matches!(
        event(&frame, &key, &mut receiver).unwrap(),
        Incoming::Ready
    ));
    assert!(event(&frame, &key, &mut receiver).is_err());
    let next = br#"{"type":"next","id":"clipboardChanged","payload":{"data":{"clipboardChanged":{"text":"text","sensitive":false,"time":1}}}}"#;
    let frame = sender.encode(next, &key).unwrap();
    assert!(matches!(
        event(&frame, &key, &mut receiver).unwrap(),
        Incoming::Event("CLIPBOARD_CHANGED")
    ));
}

#[test]
fn corrupt_frames_and_unknown_operations_are_rejected() {
    let key = [7; 32];
    let (mut sender, mut receiver) = streams(&key, "client");
    let frame = sender
        .encode(
            br#"{"type":"next","id":"unknown","payload":{"data":{"unknown":true}}}"#,
            &key,
        )
        .unwrap();
    assert!(event(&frame, &[8; 32], &mut receiver).is_err());
    let mut corrupt = frame.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(event(&corrupt, &key, &mut receiver).is_err());
    assert!(event(&frame, &key, &mut receiver).is_err());
}
