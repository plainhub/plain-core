use super::*;

#[test]
fn attachments_use_the_supplied_runtime_token_without_a_session_record() {
    let token = plain_libs::crypto::gen_token();
    let content = r#"{"type":"FILES","value":{"items":[{"uri":"fid:fixture.txt"}]}}"#;
    let wire: Value = plain_libs::serde_json::from_str(&peer_content(content, "client", &token).unwrap()).unwrap();
    let id = wire["value"]["items"][0]["uri"].as_str().unwrap().strip_prefix("fsid:").unwrap();
    let (scope, payload) = resource_link::parse(id).unwrap();
    assert_eq!(scope, Scope::Session("client".into()));
    let key = crate::features::sessions::token_key(&token).unwrap();
    assert_eq!(resource_link::open(&scope, &key, payload).unwrap(), Resource::reference("fid:fixture.txt"));
    let rotated = crate::features::sessions::token_key(&plain_libs::crypto::gen_token()).unwrap();
    assert!(resource_link::open(&scope, &rotated, payload).is_err());
    assert!(peer_content(content, "client", "invalid-token").is_err());
}

#[test]
fn attachment_duration_preserves_units_precedence_and_rejects_invalid_values() {
    for (value, expected) in [
        (plain_libs::serde_json::json!({"durationMs":7,"duration":8}),7),
        (plain_libs::serde_json::json!({"legacyDurationSec":2}),2000),
        (plain_libs::serde_json::json!({"duration":3}),3000),
        (plain_libs::serde_json::json!({"durationMs":null,"duration":4}),4000),
        (plain_libs::serde_json::json!({}),0),
    ] { assert_eq!(attachment_duration_ms(&value).unwrap(), expected); }
    for value in [plain_libs::serde_json::json!({"durationMs":"bad"}),plain_libs::serde_json::json!({"duration":-1}),plain_libs::serde_json::json!({"legacyDurationSec":i64::MAX}),plain_libs::serde_json::json!({"legacyDurationSec":2,"duration":i64::MAX})] {
        assert!(attachment_duration_ms(&value).is_err());
    }
}
