use super::*;

fn streams(key: &[u8], client: &str) -> (websocket::Stream, websocket::Stream) {
    let init = registration(key, client).unwrap();
    let ack = websocket::seal_ack(
        key,
        client,
        "/graphql",
        &init,
        br#"{"serverNonce":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}"#,
    )
    .unwrap();
    (
        websocket::Stream::new("/graphql", client, &init, &ack, Role::Server).unwrap(),
        acknowledge(key, client, &init, &ack).unwrap(),
    )
}

#[test]
fn registration_binds_identity_and_path_without_timestamp() {
    let key = [7; 32];
    let init = registration(&key, "client").unwrap();
    let plain = websocket::open_init(&key, "client", "/graphql", &init).unwrap();
    let value: plain_libs::serde_json::Value = plain_libs::serde_json::from_slice(&plain).unwrap();
    assert!(value.get("timestampMillis").is_none());
    assert_eq!(value["clientNonce"].as_str().unwrap().len(), 64);
    assert!(websocket::open_init(&key, "client", "/screen", &init).is_none());
    assert!(websocket::open_init(&key, "other", "/graphql", &init).is_none());
    assert!(websocket::open_init(&[8; 32], "client", "/graphql", &init).is_none());
    assert!(registration(&[7; 31], "client").is_err());
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
