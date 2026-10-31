use super::*;
use serde_json::json;

#[test]
fn native_batches_emit_one_deduplicated_public_invalidation_and_empty_batches_emit_none() {
    let (events, mut receiver) = broadcast::channel(8);
    publish(&events, []);
    assert!(receiver.try_recv().is_err());
    publish(&events, [3, 1, 3, 24]);
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.event_type, MEDIA_LIBRARY_UPDATED);
    assert!(!event.is_host_event());
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&event.payload).unwrap(),
        json!({"types":[1,3,24]})
    );
    assert!(receiver.try_recv().is_err());
}

#[test]
fn catalog_routing_selects_only_the_reported_media_kind() {
    use plain_rs::enums::DataType;
    assert!(includes(r#"{"types":[3,1]}"#, DataType::Image));
    assert!(!includes(r#"{"types":[1]}"#, DataType::Image));
    assert!(!includes(r#"{"types":[]}"#, DataType::Image));
    assert!(!includes(r#"{"types":[3],"extra":true}"#, DataType::Image));
    assert!(!includes("invalid", DataType::Image));
}
