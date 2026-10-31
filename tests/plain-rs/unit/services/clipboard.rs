use super::*;
use crate::test_runtime::Core as ContentServer;
use serde_json::{Value, json};
use std::sync::Arc;

fn start(dir: &tempfile::TempDir) -> (ContentServer, String, Arc<plain_rs::prefs::Prefs>) {
    let token = plain_rs::utils::base64::base64_encode(&[9; 32]);
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", json!(["CLIPBOARD"])).unwrap();
    (
        ContentServer::start(dir.path(), &token, prefs.clone()).unwrap(),
        token,
        prefs,
    )
}

#[tokio::test]
async fn reports_recheck_permission_deduplicate_and_publish_only_nonsensitive_text() {
    let dir = tempfile::tempdir().unwrap();
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
    let body: Value = serde_json::from_str(&event.payload).unwrap();
    let item = state.db.clipboard_latest().unwrap().unwrap();
    assert_eq!(
        body,
        json!({"text":item.text,"sensitive":false,
        "time":chrono::DateTime::parse_from_rfc3339(&item.created_at).unwrap().timestamp_millis()})
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
async fn local_captures_coalesce_duplicates_and_sensitive_or_historical_changes_invalidate_pending_pushes()
 {
    let directory = tempfile::tempdir().unwrap();
    let (server, _, prefs) = start(&directory);
    let state = server.runtime_state();
    let mut changes = state.clipboard_changes.subscribe();
    prefs.set("api_permissions", json!(["CLIPBOARD"])).unwrap();
    report(
        &state,
        &json!({"text":"first","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(changes.has_changed().unwrap());
    let first = changes.borrow_and_update().clone().unwrap();
    assert!(first.eligible);
    report(
        &state,
        &json!({"text":"first","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(!changes.has_changed().unwrap());
    report(
        &state,
        &json!({"text":"first","sensitive":true}).to_string(),
    )
    .unwrap();
    let secret = changes.borrow_and_update().clone().unwrap();
    assert!(!secret.eligible && secret.sensitive);
    report(
        &state,
        &json!({"text":"second","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(changes.borrow_and_update().as_ref().unwrap().eligible);
    report(
        &state,
        &json!({"text":"first","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(!changes.borrow_and_update().as_ref().unwrap().eligible);
    prefs.set("api_permissions", json!([])).unwrap();
    report(
        &state,
        &json!({"text":"disabled","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(!changes.has_changed().unwrap());
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 2);
    prefs.set("api_permissions", json!(["CLIPBOARD"])).unwrap();
    report(
        &state,
        &json!({"text":"third","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(changes.borrow_and_update().as_ref().unwrap().eligible);
    state.clipboard_changes.received("third");
    assert!(changes.has_changed().unwrap());
    assert!(!changes.borrow_and_update().as_ref().unwrap().eligible);
    report(
        &state,
        &json!({"text":"third","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(!changes.has_changed().unwrap());
    state.db.clipboard_delete_query("all:true").unwrap();
    report(
        &state,
        &json!({"text":"third","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(changes.has_changed().unwrap());
    assert!(changes.borrow_and_update().as_ref().unwrap().eligible);
    server.shutdown().await;
}
