use super::*;

#[test]
fn attachments_resolve_the_current_session_url_token_without_retaining_a_service_key() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    crate::features::sessions::SessionStore::new(&db)
        .upsert(crate::features::sessions::SessionInfo {
            client_id: "client".into(),
            ..Default::default()
        })
        .unwrap();
    let mut session = db.session_get("client").unwrap().unwrap();
    let content = r#"{"type":"FILES","value":{"items":[{"uri":"fid:fixture.txt"}]}}"#;
    for _ in 0..2 {
        let wire: Value =
            plain_libs::serde_json::from_str(&peer_content(content, "client", &db).unwrap()).unwrap();
        let id = wire["value"]["items"][0]["uri"]
            .as_str()
            .unwrap()
            .strip_prefix("fsid:")
            .unwrap();
        let (scope, payload) = resource_link::parse(id).unwrap();
        assert_eq!(scope, Scope::Session("client".into()));
        let key = crate::features::sessions::token_key(&session.url_token).unwrap();
        assert_eq!(
            resource_link::open(&scope, &key, payload).unwrap(),
            Resource::reference("fid:fixture.txt")
        );
        session.url_token = plain_libs::crypto::gen_token();
        db.session_save(&session).unwrap();
        let rotated = crate::features::sessions::token_key(&session.url_token).unwrap();
        assert!(resource_link::open(&scope, &rotated, payload).is_err());
    }
    db.session_delete("client").unwrap();
    assert!(peer_content(content, "client", &db).is_err());
    assert!(db.session_get("client").unwrap().is_none());
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
