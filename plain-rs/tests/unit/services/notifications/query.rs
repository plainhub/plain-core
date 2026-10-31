use super::*;
use serde_json::json;

#[test]
fn notifications_apply_preferences_literal_text_and_instant_order() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    prefs
        .set(
            "notification_filter",
            json!({"mode":"allowlist","apps":["allowed"]}),
        )
        .unwrap();
    let facts = vec![
        json!({"id":"a","appId":"allowed","appName":"A","title":"50% sale","body":"","postedAt":"2026-10-05T01:00:00.000Z"}),
        json!({"id":"b","appId":"blocked","appName":"B","title":"50% sale","body":"","postedAt":"2026-10-05T02:00:00.000Z"}),
        json!({"id":"c","appId":"allowed","appName":"C","title":"50% sale","body":"","postedAt":"2026-10-05T04:00:00+02:00"}),
    ];
    let result = notifications(&prefs, facts.clone(), "text:'50%'").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0]["id"], "c");
    assert!(
        notifications(&prefs, facts, "text:'50_'")
            .unwrap()
            .is_empty()
    );
}
