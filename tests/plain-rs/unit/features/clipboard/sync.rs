use super::*;
use plain_libs::serde_json::{Value, json};

#[test]
fn direction_is_canonical_per_peer_and_defaults_to_receive_only() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    assert_eq!(direction(&prefs, "peer").unwrap(), Direction::Receive);
    for (wire, expected, send, receive) in [
        ("OFF", Direction::Off, false, false),
        ("SEND", Direction::Send, true, false),
        ("RECEIVE", Direction::Receive, false, true),
        ("BOTH", Direction::Both, true, true),
    ] {
        prefs
            .set(CLIPBOARD_DIRECTIONS, json!({"peer":wire,"unrelated":"bad"}))
            .unwrap();
        let actual = direction(&prefs, "peer").unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.sends(), send);
        assert_eq!(actual.receives(), receive);
        assert_eq!(
            plain_libs::serde_json::to_value(actual).unwrap(),
            json!(wire)
        );
        assert_eq!(direction(&prefs, "absent").unwrap(), Direction::Receive);
    }
    for value in [json!("invalid"), json!("pull"), json!(1), Value::Null] {
        prefs
            .set(CLIPBOARD_DIRECTIONS, json!({"peer":value}))
            .unwrap();
        assert!(direction(&prefs, "peer").is_err());
    }
}
