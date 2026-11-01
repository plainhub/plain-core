use super::*;

#[test]
fn named_frames_round_trip_without_numeric_type_headers() {
    let key = [7u8; 32];
    let frame = encode("PAIRING_REQUEST_RECEIVED", b"{}", &key).unwrap();
    assert!(frame.starts_with(b"PAIRING_REQUEST_RECEIVED\0"));
    assert_eq!(
        decode(&frame, &key).unwrap(),
        ("PAIRING_REQUEST_RECEIVED".into(), b"{}".to_vec())
    );
    let token = plain_libs::utils::base64::base64_encode(&key);
    let frame = encode_with_token("MESSAGE_CREATED", b"[]", &token).unwrap();
    assert_eq!(
        decode_with_token(&frame, &token).unwrap(),
        ("MESSAGE_CREATED".into(), b"[]".to_vec())
    );
}

#[test]
fn raw_media_frames_keep_all_payload_bytes() {
    for kind in [
        "SCREEN_MIRROR_VIDEO",
        "SCREEN_MIRROR_AUDIO",
        "IMAGE_EDITOR_UPDATE",
    ] {
        let payload = [0, 1, 255, 0, 4];
        let frame = encode_raw(kind, &payload).unwrap();
        let (name, bytes) = decode_raw(&frame).unwrap();
        assert_eq!(name, kind);
        assert_eq!(bytes, payload);
    }
}

#[test]
fn malformed_names_numeric_frames_and_wrong_keys_are_rejected() {
    for name in [
        "",
        "pairing_started",
        "PAIRING-STARTED",
        "22",
        "MESSAGE\0CREATED",
    ] {
        assert!(encode_raw(name, b"x").is_none());
    }
    for frame in [
        b"".as_slice(),
        b"MESSAGE_CREATED",
        b"\0payload",
        b"22\0payload",
        b"message_created\0payload",
        &[0, 0, 0, 22, 1, 2],
        &[255, 0, 1],
    ] {
        assert!(decode_raw(frame).is_none());
    }
    let frame = encode("MESSAGE_CREATED", b"x", &[1; 32]).unwrap();
    assert!(decode(&frame, &[2; 32]).is_none());
    assert!(decode(b"MESSAGE_CREATED\0", &[1; 32]).is_none());
}

#[test]
fn an_authenticated_payload_cannot_be_renamed_or_use_the_old_unbound_cipher() {
    let key = [7; 32];
    let frame = encode("CLIPBOARD_CHANGED", br#"{"text":"fixture"}"#, &key).unwrap();
    let (_, body) = decode_raw(&frame).unwrap();
    let renamed = encode_raw("DEVICE_NAME_UPDATED", body).unwrap();
    assert!(decode(&renamed, &key).is_none());
    let legacy = plain_libs::crypto::xchacha_encrypt_raw(&key, b"{}").unwrap();
    assert!(decode(&encode_raw("CLIPBOARD_CHANGED", &legacy).unwrap(), &key).is_none());
}

#[test]
fn network_events_authenticate_registration_order_name_and_binary_payload_before_advancing() {
    let key = [7; 32];
    let mut sender = Stream::new(b"first registration");
    let mut receiver = Stream::new(b"first registration");
    let first = sender
        .encode("CLIPBOARD_CHANGED", br#"{"text":"fixture"}"#, &key)
        .unwrap();
    let second = sender
        .encode("SCREEN_MIRROR_VIDEO", &[0, 1, 255, 0, 4], &key)
        .unwrap();
    assert!(receiver.decode(&second, &key).is_none());
    assert!(
        Stream::new(b"other registration")
            .decode(&first, &key)
            .is_none()
    );
    assert!(receiver.decode(&first, &[8; 32]).is_none());
    let (_, body) = decode_raw(&first).unwrap();
    let renamed = encode_raw("DEVICE_NAME_UPDATED", body).unwrap();
    assert!(receiver.decode(&renamed, &key).is_none());
    let mut tampered = first.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert!(receiver.decode(&tampered, &key).is_none());
    for length in 0..first.len() {
        assert!(receiver.decode(&first[..length], &key).is_none());
    }
    assert_eq!(
        receiver.decode(&first, &key).unwrap(),
        (
            "CLIPBOARD_CHANGED".into(),
            br#"{"text":"fixture"}"#.to_vec()
        )
    );
    assert!(receiver.decode(&first, &key).is_none());
    assert_eq!(
        receiver.decode(&second, &key).unwrap(),
        ("SCREEN_MIRROR_VIDEO".into(), vec![0, 1, 255, 0, 4])
    );
    assert!(decode(&first, &key).is_none());
    let old = encode("CLIPBOARD_CHANGED", b"{}", &key).unwrap();
    assert!(
        Stream::new(b"first registration")
            .decode(&old, &key)
            .is_none()
    );
    assert!(sender.encode("invalid", b"x", &key).is_none());
    assert!(sender.encode("CONTENT_CHANGED", b"x", &[7; 31]).is_none());
    assert_eq!(sender.next, 2);
    sender.next = u64::MAX;
    assert!(sender.encode("CONTENT_CHANGED", b"x", &key).is_none());
}

#[test]
fn noble_event_vectors_match_rust_authentication_and_exact_cipher_bytes() {
    use chacha20poly1305::{
        XChaCha20Poly1305, XNonce,
        aead::{Aead, KeyInit, Payload},
    };
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../plain-rs/testdata/crypto/websocket-event-vectors.json"
    ))
    .unwrap();
    let vectors = vectors.as_array().unwrap();
    let registration =
        plain_libs::utils::base64::base64_decode(vectors[0]["registration"].as_str().unwrap());
    let mut stream = Stream::new(&registration);
    for (sequence, vector) in vectors.iter().enumerate() {
        let bytes =
            |field: &str| plain_libs::utils::base64::base64_decode(vector[field].as_str().unwrap());
        let key = bytes("key");
        let frame = bytes("frame");
        let payload = bytes("payload");
        let name = vector["name"].as_str().unwrap();
        let aad = stream.aad(name, sequence as u64);
        assert_eq!(
            plain_libs::utils::hex::bytes_to_hex(&aad),
            vector["aadHex"].as_str().unwrap()
        );
        assert_eq!(
            stream.decode(&frame, &key).unwrap(),
            (name.to_owned(), payload.clone())
        );
        assert_eq!(
            decode(&bytes("nativeFrame"), &key).unwrap(),
            (name.to_owned(), payload.clone())
        );
        let (_, body) = decode_raw(&frame).unwrap();
        assert_eq!(
            XChaCha20Poly1305::new_from_slice(&key)
                .unwrap()
                .encrypt(
                    XNonce::from_slice(&body[8..32]),
                    Payload {
                        msg: &payload,
                        aad: &aad
                    }
                )
                .unwrap(),
            body[32..]
        );
    }
}
