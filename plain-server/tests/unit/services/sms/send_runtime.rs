use super::*;

#[tokio::test]
async fn native_part_reports_publish_one_result_and_replay_the_durable_outbox() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(plain_rs::kv::LazyDb::new(directory.path().join("plain.db")));
    let (events, mut received) = broadcast::channel(16);
    let runtime = Runtime::new(store.clone(), events.clone());
    runtime
        .register("send".into(), Some("request".into()), 2)
        .unwrap();
    let report = |index| Report {
        request_id: "send".into(),
        part_index: index,
        part_count: 2,
        success: true,
        result_code: -1,
    };
    runtime.report(report(0)).unwrap();
    runtime.report(report(0)).unwrap();
    assert!(received.try_recv().is_err());
    runtime.report(report(1)).unwrap();
    let event = received.recv().await.unwrap();
    assert_eq!(event.event_type, "SMS_SEND_RESULT");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&event.payload).unwrap(),
        serde_json::json!({"requestId":"request","success":true,"resultCode":-1})
    );
    runtime.report(report(1)).unwrap();
    assert!(received.try_recv().is_err());
    let restored = Runtime::new(store, events);
    restored.replay().unwrap();
    assert_eq!(received.recv().await.unwrap().payload, event.payload);
    restored.cancel("send").unwrap();
    restored.report(report(1)).unwrap();
    assert!(received.try_recv().is_err());
}
