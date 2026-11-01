use super::*;
#[cfg(feature = "chat")]
use serde_json::json;

#[test]
#[cfg(feature = "chat")]
fn public_pairing_success_excludes_credentials_without_changing_native_payload() {
    let payload = json!({
        "deviceId": "peer",
        "deviceName": "Device",
        "ip": "192.0.2.1",
        "key": "private-chat-key",
        "token": "private-session-token",
        "password": "private-password",
        "credentials": {"key": "nested-key"}
    });
    let event = WsEvent::targeted(WS_PAIRING_SUCCESS, payload.to_string(), "client");
    let public = event.clone().into_public().unwrap();
    assert_eq!(public.event_type, WS_PAIRING_SUCCESS);
    assert_eq!(public.target_cid.as_deref(), Some("client"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&public.payload).unwrap(),
        json!({"deviceId":"peer","deviceName":"Device","ip":"192.0.2.1"})
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&event.payload).unwrap(),
        payload
    );
}

#[test]
fn public_pairing_success_drops_malformed_and_binary_payloads() {
    for payload in [
        "not json",
        "null",
        "[]",
        r#"{"key":"secret"}"#,
        r#"{"deviceId":"peer","deviceName":"Device","ip":false,"key":"secret"}"#,
    ] {
        assert!(
            WsEvent::broadcast(WS_PAIRING_SUCCESS, payload.into())
                .into_public()
                .is_none()
        );
    }
    assert!(
        WsEvent::broadcast_binary(WS_PAIRING_SUCCESS, b"secret".to_vec())
            .into_public()
            .is_none()
    );
}

#[test]
fn public_projection_keeps_regular_text_and_binary_buffers_and_excludes_internal_events() {
    let payload = "not necessarily json".to_string();
    let pointer = payload.as_ptr();
    let event = WsEvent::targeted("CONTENT_CHANGED", payload, "client")
        .into_public()
        .unwrap();
    assert_eq!(event.payload, "not necessarily json");
    assert_eq!(event.payload.as_ptr(), pointer);
    assert_eq!(event.target_cid.as_deref(), Some("client"));
    let bytes = vec![0, 1, 255];
    let pointer = bytes.as_ptr();
    let event = WsEvent::broadcast_binary("SCREEN_MIRROR_VIDEO", bytes)
        .into_public()
        .unwrap();
    let bytes = event.binary_payload.unwrap();
    assert_eq!(bytes, [0, 1, 255]);
    assert_eq!(bytes.as_ptr(), pointer);
}
