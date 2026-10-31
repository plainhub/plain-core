use super::*;
use serde_json::json;

fn item(id: &str) -> Item {
    Item {
        id: id.into(),
        text: "Fixture 🌍".into(),
        sensitive: false,
    }
}

#[test]
fn first_observation_seeds_history_without_writing_and_reload_preserves_deduplication() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("prefs.json");
    let prefs = Prefs::load(&path).unwrap();
    let store_path = directory.path().join("plain.db");
    let store = crate::kv::Db::open(&store_path).unwrap();
    assert!(!prepare(&prefs, &store, "peer", &item("old")).unwrap());
    assert!(!prepare(&prefs, &store, "peer", &item("old")).unwrap());
    assert!(prepare(&prefs, &store, "peer", &item("new")).unwrap());
    assert!(prepare(&prefs, &store, "peer", &item("new")).unwrap());
    applied(&store, "peer", &item("new")).unwrap();
    assert!(!prefs.entries().iter().any(|(key, _)| key == APPLIED));
    drop(store);
    let store = crate::kv::Db::open(&store_path).unwrap();
    let reloaded = Prefs::load(&path).unwrap();
    assert!(!prepare(&reloaded, &store, "peer", &item("new")).unwrap());
    assert!(prepare(&reloaded, &store, "peer", &item("next")).unwrap());
    assert!(!prepare(&reloaded, &store, "another-peer", &item("next")).unwrap());
}

#[test]
fn directions_and_sensitive_items_do_not_advance_the_applied_marker() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    let store = crate::kv::Db::open(&directory.path().join("plain.db")).unwrap();
    assert!(!prepare(&prefs, &store, "peer", &item("baseline")).unwrap());
    for (direction, receives) in [
        ("off", false),
        ("push", false),
        ("pull", true),
        ("both", true),
    ] {
        prefs
            .set(CLIPBOARD_DIRECTIONS, json!({"peer":direction}))
            .unwrap();
        assert_eq!(super::receives(&prefs, "peer").unwrap(), receives);
        assert_eq!(
            prepare(&prefs, &store, "peer", &item("next")).unwrap(),
            receives
        );
    }
    let sensitive = Item {
        sensitive: true,
        ..item("secret")
    };
    assert!(!prepare(&prefs, &store, "peer", &sensitive).unwrap());
    assert_eq!(
        store.get_json::<Value>(APPLIED).unwrap().unwrap(),
        json!({"peer":"baseline"})
    );
    for direction in [json!("invalid"), json!(1), Value::Null] {
        prefs
            .set(CLIPBOARD_DIRECTIONS, json!({"peer":direction}))
            .unwrap();
        assert!(super::receives(&prefs, "peer").is_err());
    }
}

#[test]
fn invalid_text_and_identity_never_change_persisted_state() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    let store = crate::kv::Db::open(&directory.path().join("plain.db")).unwrap();
    for invalid in [
        item(""),
        Item {
            text: " ".into(),
            ..item("id")
        },
        Item {
            text: "🌍".repeat(128 * 1024 + 1),
            ..item("id")
        },
    ] {
        assert!(prepare(&prefs, &store, "peer", &invalid).is_err());
        assert!(store.get_json::<Value>(APPLIED).unwrap().is_none());
    }
}

#[test]
fn failed_marker_persistence_does_not_claim_success_or_lose_other_peers() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    let store = crate::kv::Db::open(&directory.path().join("plain.db")).unwrap();
    assert!(!prepare(&prefs, &store, "peer", &item("old")).unwrap());
    assert!(!prepare(&prefs, &store, "other", &item("other-old")).unwrap());
    let before = store.get_json::<Value>(APPLIED).unwrap();
    store.set_write_failure_for_test(true);
    assert!(applied(&store, "peer", &item("new")).is_err());
    store.set_write_failure_for_test(false);
    assert_eq!(store.get_json::<Value>(APPLIED).unwrap(), before);
    assert!(prepare(&prefs, &store, "peer", &item("new")).unwrap());
}

#[test]
fn invalid_persisted_markers_do_not_claim_success_or_lose_other_peers() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    let store = crate::kv::Db::open(&directory.path().join("plain.db")).unwrap();
    assert!(!prepare(&prefs, &store, "peer", &item("old")).unwrap());
    assert!(!prepare(&prefs, &store, "other", &item("other-old")).unwrap());
    store
        .set_json(APPLIED, json!({"peer":1,"other":"other-old"}))
        .unwrap();
    let before = store.get_json::<Value>(APPLIED).unwrap();
    assert!(applied(&store, "peer", &item("new")).is_err());
    assert!(prepare(&prefs, &store, "peer", &item("new")).is_err());
    assert_eq!(store.get_json::<Value>(APPLIED).unwrap(), before);
}

#[test]
fn sending_is_explicit_and_does_not_inherit_the_default_receive_direction() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    assert!(!sends(&prefs, "peer").unwrap());
    assert!(receives(&prefs, "peer").unwrap());
    for (direction, send, receive) in [
        ("off", false, false),
        ("pull", false, true),
        ("push", true, false),
        ("both", true, true),
    ] {
        prefs
            .set(CLIPBOARD_DIRECTIONS, json!({"peer":direction}))
            .unwrap();
        assert_eq!(sends(&prefs, "peer").unwrap(), send);
        assert_eq!(receives(&prefs, "peer").unwrap(), receive);
        assert!(!sends(&prefs, "another").unwrap());
    }
    for value in [json!("invalid"), json!(1), Value::Null] {
        prefs
            .set(CLIPBOARD_DIRECTIONS, json!({"peer":value}))
            .unwrap();
        assert!(sends(&prefs, "peer").is_err());
        assert!(receives(&prefs, "peer").is_err());
    }
}
