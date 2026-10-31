use super::*;

#[test]
fn registration_rejects_replay_stale_future_and_non_timestamp_plaintext() {
    let replay = Replay::default();
    let frame = [7; 24];
    assert!(admit(&replay, "client", &frame, b"100000", false, 100000).is_ok());
    assert!(admit(&replay, "client", &frame, b"100000", false, 100000).is_err());
    assert!(admit(&replay, "other", &frame, b"100000", false, 100000).is_ok());
    for plain in [
        b"39999".as_slice(),
        b"160001",
        b"-9223372036854775808",
        b"{}",
        b"99999999999999999999",
    ] {
        assert!(admit(&replay, "client", &[8; 24], plain, false, 100000).is_err());
    }
}

#[test]
fn control_frames_keep_exact_binary_and_json_bytes_and_reject_replay_and_missing_time() {
    let replay = Replay::default();
    for (nonce, payload) in [
        (1, b"\x54\x01\x00\xff".as_slice()),
        (
            2,
            br#"{"type":"screenMirrorControl","input":{"action":"BACK"}}"#,
        ),
    ] {
        let mut plain = 100000i64.to_be_bytes().to_vec();
        plain.extend_from_slice(payload);
        assert_eq!(
            admit(&replay, "client", &[nonce; 24], &plain, true, 100000).unwrap(),
            payload
        );
        assert!(admit(&replay, "client", &[nonce; 24], &plain, true, 100000).is_err());
        assert!(admit(&replay, "client", &[nonce + 10; 24], &plain, true, 160001).is_err());
        assert!(admit(&replay, "client", &[nonce + 20; 24], payload, true, 100000).is_err());
    }
}
