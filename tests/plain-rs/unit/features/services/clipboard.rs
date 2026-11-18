use super::*;
use crate::test_runtime::Core as ContentServer;
use plain_libs::serde_json::{Value, json};
use std::sync::Arc;

fn start(dir: &plain_libs::temporary::TempDir) -> (ContentServer, String, Arc<crate::features::prefs::Prefs>) {
    let token = plain_libs::utils::base64::base64_encode(&[9; 32]);
    let prefs =
        Arc::new(crate::features::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", json!(["CLIPBOARD"])).unwrap();
    (
        ContentServer::start(dir.path(), &token, prefs.clone()).unwrap(),
        token,
        prefs,
    )
}

#[tokio::test]
async fn reports_recheck_permission_deduplicate_and_publish_only_nonsensitive_text() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let (server, _, prefs) = start(&dir);
    let state = server.runtime_state();
    let mut events = state.events.subscribe();
    let (generation, mut requests) = state.host.connect();
    prefs.set("api_permissions", json!([])).unwrap();
    let payload = json!({"text":"visible 🌍", "sensitive":false}).to_string();
    report(&state, &payload).unwrap();
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 0);
    assert!(events.try_recv().is_err());
    prefs.set("api_permissions", json!(["CLIPBOARD"])).unwrap();
    report(&state, &payload).unwrap();
    let callback = requests.try_recv().unwrap();
    assert_eq!(callback["method"], "clipboardHistoryRefresh");
    state
        .host
        .reply(generation, json!({"id":callback["id"],"result":null}))
        .unwrap();
    let event = events.try_recv().unwrap();
    assert_eq!(event.event_type, "CLIPBOARD_CHANGED");
    let body: Value = plain_libs::serde_json::from_str(&event.payload).unwrap();
    let item = state.db.clipboard_latest().unwrap().unwrap();
    assert_eq!(
        body,
        json!({"text":item.text,"sensitive":false,
        "time":plain_libs::chrono::DateTime::parse_from_rfc3339(&item.created_at).unwrap().timestamp_millis()})
    );
    report(&state, &payload).unwrap();
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 1);
    assert!(events.try_recv().is_err());
    for invalid in [
        json!({"text":"", "sensitive":false}),
        json!({"text":"🌍".repeat(128 * 1024 + 1), "sensitive":false}),
    ] {
        assert!(report(&state, &invalid.to_string()).is_err());
    }
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 1);
    assert!(events.try_recv().is_err());
    server.shutdown().await;
}

#[tokio::test]
async fn repeated_local_captures_are_not_treated_as_received_echoes_after_history_is_cleared() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let (server, _, _) = start(&dir);
    let state = server.runtime_state();
    let payload = json!({"text":"local fixture", "sensitive":false}).to_string();
    report(&state, &payload).unwrap();
    report(&state, &payload).unwrap();
    assert_eq!(state.db.clipboard_clear().unwrap(), 1);
    let mut events = state.events.subscribe();
    report(&state, &payload).unwrap();
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 1);
    assert_eq!(events.try_recv().unwrap().event_type, "CLIPBOARD_CHANGED");
    server.shutdown().await;
}

#[tokio::test]
async fn sensitive_reports_are_stored_but_scoped_platform_echoes_are_not() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let (server, _, _) = start(&directory);
    let state = server.runtime_state();
    let mut events = state.events.subscribe();
    let text = "sensitive fixture";
    let payload = json!({"text":text,"sensitive":true}).to_string();
    let first = state.clipboard_writes.begin(text);
    let second = state.clipboard_writes.begin(text);
    drop(first);
    report(&state, &payload).unwrap();
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 0);
    assert!(events.try_recv().is_err());
    drop(second);
    report(&state, &payload).unwrap();
    let item = state.db.clipboard_latest().unwrap().unwrap();
    assert_eq!(item.text, text);
    assert!(item.sensitive);
    assert!(events.try_recv().is_err());
    server.shutdown().await;
}
