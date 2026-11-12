use super::*;

fn pair(path: &str) -> (Stream, Stream) {
    (
        Stream::new(path, "client", b"init", b"ack", Role::Client).unwrap(),
        Stream::new(path, "client", b"init", b"ack", Role::Server).unwrap(),
    )
}

#[test]
fn raw_media_frames_keep_names_and_every_binary_byte() {
    for name in [
        "SCREEN_MIRROR_VIDEO",
        "SCREEN_MIRROR_AUDIO",
        "IMAGE_EDITOR_UPDATE",
    ] {
        let payload = [0, 1, 255, 0, 4];
        let frame = encode_raw(name, &payload).unwrap();
        assert_eq!(decode_raw(&frame).unwrap(), (name, payload.as_slice()));
    }
    for name in ["", "bad_name", "BAD-NAME", "22", "NAME\0OTHER"] {
        assert!(encode_raw(name, b"data").is_none());
    }
    for frame in [
        b"".as_slice(),
        b"NAME",
        b"\0payload",
        b"22\0payload",
        &[255, 0, 1],
    ] {
        assert!(decode_raw(frame).is_none());
    }
}

#[test]
fn counters_are_independent_strict_and_authenticated() {
    let key = [7; 32];
    let (mut client, mut server) = pair("/screen");
    let first = client.encode(b"first", &key).unwrap();
    let second = client.encode(b"second", &key).unwrap();
    assert_eq!(first.len(), 8 + 24 + b"first".len() + 16);
    assert_eq!(&first[..8], &1u64.to_be_bytes());
    assert_eq!(&second[..8], &2u64.to_be_bytes());
    assert!(server.decode(&second, &key).is_none());
    let mut changed = first.clone();
    changed[7] = 2;
    assert!(server.decode(&changed, &key).is_none());
    changed[7] = 1;
    *changed.last_mut().unwrap() ^= 1;
    assert!(server.decode(&changed, &key).is_none());
    assert!(server.decode(&first, &[8; 32]).is_none());
    for length in 0..first.len() {
        assert!(server.decode(&first[..length], &key).is_none());
    }
    assert_eq!(server.decode(&first, &key).unwrap(), b"first");
    assert!(server.decode(&first, &key).is_none());
    assert_eq!(server.decode(&second, &key).unwrap(), b"second");
    let response = server.encode(b"response", &key).unwrap();
    assert_eq!(&response[..8], &1u64.to_be_bytes());
    assert_eq!(client.decode(&response, &key).unwrap(), b"response");
}

#[test]
fn frames_cannot_cross_connections_routes_clients_or_directions() {
    let key = [7; 32];
    let (mut client, _) = pair("/screen");
    let frame = client.encode(b"control", &key).unwrap();
    for (path, owner, init, ack, role) in [
        (
            "/screen",
            "client",
            b"init".as_slice(),
            b"new ack".as_slice(),
            Role::Server,
        ),
        ("/screen", "client", b"new init", b"ack", Role::Server),
        ("/image_editor", "client", b"init", b"ack", Role::Server),
        ("/screen", "other", b"init", b"ack", Role::Server),
        ("/screen", "client", b"init", b"ack", Role::Client),
    ] {
        let mut receiver = Stream::new(path, owner, init, ack, role).unwrap();
        assert!(receiver.decode(&frame, &key).is_none());
        assert_eq!(receiver.received, 0);
    }
}

#[test]
fn sequence_overflow_fails_without_wrapping_or_consuming_state() {
    let key = [7; 32];
    let (mut client, mut server) = pair("/graphql");
    client.sent = u64::MAX - 1;
    server.received = u64::MAX - 1;
    let final_frame = client.encode(b"last", &key).unwrap();
    assert_eq!(&final_frame[..8], &u64::MAX.to_be_bytes());
    assert_eq!(server.decode(&final_frame, &key).unwrap(), b"last");
    assert!(client.encode(b"overflow", &key).is_none());
    assert!(server.decode(&final_frame, &key).is_none());
    assert_eq!(client.sent, u64::MAX);
    assert_eq!(server.received, u64::MAX);
    let (mut client, _) = pair("/graphql");
    assert!(client.encode(b"bad key", &[0; 2]).is_none());
    assert_eq!(client.sent, 0);
}

#[test]
fn handshake_binds_both_authenticated_fresh_frames_without_time() {
    let key = [7; 32];
    let body = plain_libs::serde_json::to_vec(&Init {
        client_nonce: "ab".repeat(32),
    })
    .unwrap();
    let init = seal_init(&key, "client", "/graphql", &body).unwrap();
    assert_eq!(init.len(), 24 + body.len() + 16);
    assert_eq!(open_init(&key, "client", "/graphql", &init).unwrap(), body);
    let mut changed_init = init.clone();
    changed_init[0] ^= 1;
    assert!(open_init(&key, "client", "/graphql", &changed_init).is_none());
    assert!(open_init(&key, "other", "/graphql", &init).is_none());
    assert!(open_init(&key, "client", "/screen", &init).is_none());
    assert!(plain_libs::serde_json::from_str::<Init>(
        r#"{"clientNonce":"ab","timestampMillis":0}"#
    )
    .is_err());
    let response = plain_libs::serde_json::to_vec(&Ack {
        server_nonce: "cd".repeat(32),
    })
    .unwrap();
    let ack = seal_ack(&key, "client", "/graphql", &init, &response).unwrap();
    assert_eq!(ack.len(), 24 + response.len() + 16);
    let mut changed_ack = ack.clone();
    changed_ack[0] ^= 1;
    assert!(open_ack(&key, "client", "/graphql", &init, &changed_ack).is_none());
    let new_ack = seal_ack(&key, "client", "/graphql", &init, &response).unwrap();
    assert_ne!(ack, new_ack);
    assert_eq!(
        open_ack(&key, "client", "/graphql", &init, &ack).unwrap(),
        response
    );
    let other_init = seal_init(&key, "client", "/graphql", &body).unwrap();
    assert!(open_ack(&key, "client", "/graphql", &other_init, &ack).is_none());
    let mut old = Stream::new("/graphql", "client", &init, &ack, Role::Client).unwrap();
    let frame = old.encode(b"old request", &key).unwrap();
    let mut reconnected = Stream::new("/graphql", "client", &init, &new_ack, Role::Server).unwrap();
    assert!(reconnected.decode(&frame, &key).is_none());
}

#[test]
fn independent_cross_language_vectors_match_the_complete_wire_contract() {
    let vectors: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(include_str!(
        "../../../../../plain-rs/testdata/crypto/websocket-event-vectors.json"
    ))
    .unwrap();
    for vector in vectors.as_array().unwrap() {
        let bytes = |value: &plain_libs::serde_json::Value| {
            plain_libs::utils::base64::base64_decode(value.as_str().unwrap())
        };
        let key = bytes(&vector["key"]);
        let path = vector["path"].as_str().unwrap();
        let client = vector["client"].as_str().unwrap();
        let init = bytes(&vector["init"]);
        let ack = bytes(&vector["ack"]);
        let _: Init =
            plain_libs::serde_json::from_slice(&open_init(&key, client, path, &init).unwrap())
                .unwrap();
        let _: Ack =
            plain_libs::serde_json::from_slice(&open_ack(&key, client, path, &init, &ack).unwrap())
                .unwrap();
        for (name, role) in [
            ("clientFrames", Role::Server),
            ("serverFrames", Role::Client),
        ] {
            let mut stream = Stream::new(path, client, &init, &ack, role).unwrap();
            for entry in vector[name].as_array().unwrap() {
                let frame = bytes(&entry["frame"]);
                assert_eq!(stream.decode(&frame, &key).unwrap(), bytes(&entry["plain"]));
                assert!(stream.decode(&frame, &key).is_none());
            }
        }
    }
}
