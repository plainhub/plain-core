use super::*;
use crate::test_support::schema::SchemaFixture as PublicSchema;
use plain_rs::db::Db;
use serde_json::json;

fn stub(host: Arc<Host>, handler: impl Fn(&str, Value) -> Value + Send + 'static) {
    let (generation, mut requests) = host.connect();
    let host = host.clone();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let Some(id) = request["id"].as_u64() else {
                continue;
            };
            let result = if request["method"] == "systemCapabilities" {
                json!(["NOTIFICATIONS"])
            } else {
                handler(
                    &request["method"].as_str().unwrap_or_default(),
                    request["params"].clone(),
                )
            };
            let _ = host.reply(generation, json!({ "id": id, "result": result }));
        }
    });
}

fn fixture(permissions: Value) -> (tempfile::TempDir, PublicSchema) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", permissions).unwrap();
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let host = Arc::new(Host::default());
    let directory = dir.path().to_path_buf();
    let (events, _) = tokio::sync::broadcast::channel(16);
    (
        dir,
        crate::test_support::schema::build(host, events, prefs, db, directory),
    )
}

fn fact(id: &str, app: &str, time: &str) -> Value {
    json!({
        "id":id,"onlyOnce":true,"isClearable":true,"appId":app,"appName":app,
        "postedAt":time,"silent":false,"title":"title","body":"body",
        "actions":["Reply","Open"],"replyActions":["Reply"],
    })
}

fn schema_host(schema: &PublicSchema) -> Arc<Host> {
    schema.data::<Arc<Host>>().unwrap().clone()
}

#[tokio::test]
async fn notifications_map_host_facts_into_the_contract_type() {
    let (_dir, schema) = fixture(json!(["NOTIFICATION_LISTENER"]));
    stub(schema_host(&schema), |method, _| match method {
        "systemNotificationFacts" => json!([
            fact("0|1|com.a", "com.a", "2026-10-05T01:00:00.000Z"),
            fact("0|2|com.b", "com.b", "2026-10-05T02:00:00.000Z"),
        ]),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(
            r#"query { notifications(offset:0, limit:10, query:"") {
                   id onlyOnce isClearable appId appName postedAt silent title body
                   actions replyActions } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let items = response.data.into_json().unwrap()["notifications"].clone();
    assert_eq!(items[0]["id"], "0|2|com.b", "newest first");
    assert_eq!(items[0]["appId"], "com.b");
    assert_eq!(items[0]["postedAt"], "2026-10-05T02:00:00.000Z");
    assert_eq!(items[0]["actions"], json!(["Reply", "Open"]));
    assert_eq!(items[0]["replyActions"], json!(["Reply"]));
}

#[tokio::test]
async fn notification_roots_error_out_without_the_api_permission() {
    let (_dir, schema) = fixture(json!([]));
    let response = schema
        .execute(r#"query { notifications(offset:0, limit:1, query:"") { id } }"#)
        .await;
    assert_eq!(response.errors[0].message, "no_permission");
}

#[tokio::test]
async fn delete_notifications_counts_only_what_the_platform_confirmed() {
    let (_dir, schema) = fixture(json!(["NOTIFICATION_LISTENER"]));
    stub(schema_host(&schema), |method, params| match method {
        "systemCancelNotifications" => {
            assert_eq!(params["ids"], json!(["a", "b"]));
            json!(["a"])
        }
        other => panic!("unexpected host call {other}"),
    });
    let mut events = schema
        .data::<tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>>()
        .unwrap()
        .subscribe();
    let response = schema
        .execute(r#"mutation { deleteNotifications(ids:["a","b"]) { affectedCount } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["deleteNotifications"]["affectedCount"],
        1
    );
    assert_eq!(
        events.try_recv().unwrap().event_type,
        "NOTIFICATION_REFRESHED"
    );
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn a_receipt_naming_an_unrequested_notification_is_rejected() {
    let (_dir, schema) = fixture(json!(["NOTIFICATION_LISTENER"]));
    stub(schema_host(&schema), |method, _| match method {
        "systemCancelNotifications" => json!(["never-requested"]),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(r#"mutation { deleteNotifications(ids:["a"]) { affectedCount } }"#)
        .await;
    assert_eq!(
        response.errors[0].message,
        "invalid notification cancellation receipt"
    );
}

#[tokio::test]
async fn reply_notification_reports_a_vanished_action_as_action_not_found() {
    let (_dir, schema) = fixture(json!(["NOTIFICATION_LISTENER"]));
    stub(schema_host(&schema), |method, params| match method {
        "systemReplyNotification" => {
            assert_eq!(params["id"], "a");
            assert_eq!(params["actionIndex"], 0);
            assert_eq!(params["text"], "hi");
            json!(false)
        }
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(r#"mutation { replyNotification(id:"a", actionIndex:0, text:"hi") }"#)
        .await;
    assert_eq!(response.errors[0].message, "action_not_found");
}

/// A negative index never reaches the platform: plain-app's `getOrNull`
/// would return null there too, but it must not reach the host either.
#[tokio::test]
async fn reply_notification_rejects_a_negative_index_without_calling_the_host() {
    let (_dir, schema) = fixture(json!(["NOTIFICATION_LISTENER"]));
    stub(schema_host(&schema), |method, _| {
        panic!("unexpected host call {method}")
    });
    let response = schema
        .execute(r#"mutation { replyNotification(id:"a", actionIndex:-1, text:"hi") }"#)
        .await;
    assert_eq!(response.errors[0].message, "action_not_found");
}

#[test]
fn reported_notifications_keep_permission_filtering_and_project_public_events() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    let (events, mut receiver) = tokio::sync::broadcast::channel(8);
    let facts = |kind: &str| {
        serde_json::from_value(json!({
        "type":kind,
        "notification":{"id":"notice", "appId":"app", "title":"hello", "postedAt":"2026-10-01T00:00:00.000Z"}
    })).unwrap()
    };
    publish(&prefs, &events, facts("NOTIFICATION_CREATED")).unwrap();
    assert!(receiver.try_recv().is_err());
    prefs
        .set("api_permissions", json!(["NOTIFICATION_LISTENER"]))
        .unwrap();
    publish(&prefs, &events, facts("NOTIFICATION_UPDATED")).unwrap();
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.event_type, "NOTIFICATION_UPDATED");
    let payload: Value = serde_json::from_str(&event.payload).unwrap();
    assert_eq!(payload["id"], "notice");
    assert_eq!(payload["postedAt"], "2026-10-01T00:00:00.000Z");
    prefs.set("api_permissions", json!([])).unwrap();
    publish(&prefs, &events, facts("NOTIFICATION_DELETED")).unwrap();
    assert_eq!(
        receiver.try_recv().unwrap().event_type,
        "NOTIFICATION_DELETED"
    );
    assert!(publish(&prefs, &events, facts("MESSAGE_CREATED")).is_err());
    assert!(receiver.try_recv().is_err());
}

#[test]
fn sms_refresh_hints_follow_notification_permissions_filters_and_event_kinds() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    let (events, mut receiver) = tokio::sync::broadcast::channel(8);
    let facts = |kind: &str, app: &str| {
        serde_json::from_value(
            json!({"type":kind,"notification":fact("notice",app,"2026-10-01T00:00:00.000Z")}),
        )
        .unwrap()
    };
    publish(
        &prefs,
        &events,
        facts("NOTIFICATION_CREATED", "com.google.android.apps.messaging"),
    )
    .unwrap();
    assert!(receiver.try_recv().is_err());
    prefs
        .set("api_permissions", json!(["NOTIFICATION_LISTENER"]))
        .unwrap();
    for kind in ["NOTIFICATION_CREATED", "NOTIFICATION_UPDATED"] {
        publish(
            &prefs,
            &events,
            facts(kind, "com.google.android.apps.messaging"),
        )
        .unwrap();
        assert_eq!(receiver.try_recv().unwrap().event_type, kind);
        let hint = receiver.try_recv().unwrap();
        assert_eq!(hint.event_type, "SMS_PROVIDER_CHANGED");
        assert_eq!(
            serde_json::from_str::<Value>(&hint.payload).unwrap(),
            json!({"uris":[]})
        );
        assert!(receiver.try_recv().is_err());
    }
    prefs
        .set("notification_filter", json!({"mode":"allowlist","apps":[]}))
        .unwrap();
    publish(
        &prefs,
        &events,
        facts("NOTIFICATION_CREATED", "com.google.android.apps.messaging"),
    )
    .unwrap();
    assert!(receiver.try_recv().is_err());
    publish(
        &prefs,
        &events,
        facts("NOTIFICATION_DELETED", "com.google.android.apps.messaging"),
    )
    .unwrap();
    assert_eq!(
        receiver.try_recv().unwrap().event_type,
        "NOTIFICATION_DELETED"
    );
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn rejected_and_empty_cancellation_receipts_do_not_publish_refresh_hints() {
    for receipt in [json!([]), json!(["not-requested"])] {
        let empty = receipt.as_array().unwrap().is_empty();
        let (_dir, schema) = fixture(json!(["NOTIFICATION_LISTENER"]));
        stub(schema_host(&schema), move |method, _| match method {
            "systemCancelNotifications" => receipt.clone(),
            method => panic!("unexpected host call {method}"),
        });
        let mut events = schema
            .data::<tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>>()
            .unwrap()
            .subscribe();
        let response = schema
            .execute(r#"mutation { deleteNotifications(ids:["a"]) { affectedCount } }"#)
            .await;
        if empty {
            assert!(response.errors.is_empty());
            assert_eq!(
                response.data.into_json().unwrap()["deleteNotifications"]["affectedCount"],
                0
            );
        } else {
            assert_eq!(
                response.errors[0].message,
                "invalid notification cancellation receipt"
            );
        }
        assert!(events.try_recv().is_err());
    }
}
