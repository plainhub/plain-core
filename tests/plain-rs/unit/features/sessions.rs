use super::*;
use crate::infrastructure::db::{Db, SessionRow};
use std::path::Path;

fn fixture() -> SessionRow {
    SessionRow {
        client_id: "fixture-client".into(), name: "Fixture app".into(),
        r#type: SessionType::Login, token: plain_libs::crypto::gen_token(),
        url_token: plain_libs::crypto::gen_token(), client_ip: String::new(),
        app_name: "Fixture app".into(), app_version: "1".into(),
        os_name: "Fixture OS".into(), os_version: "1".into(),
        last_seen_at: Some("2026-01-02T00:00:00Z".into()),
        created_at: "2026-01-01T00:00:00Z".into(), updated_at: "2026-01-02T00:00:00Z".into(),
    }
}

#[test]
fn token_keys_require_exactly_32_bytes_of_base64() {
    assert_eq!(token_key(&plain_libs::crypto::gen_token()).unwrap().len(), 32);
    for invalid in ["invalid".into(), String::new(), plain_libs::utils::base64::base64_encode(&[1; 31]), plain_libs::utils::base64::base64_encode(&[1; 33])] {
        assert!(token_key(&invalid).is_err());
    }
}

#[test]
fn session_types_preserve_storage_and_wire_values() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    for (kind, wire) in [(SessionType::Login, "LOGIN"), (SessionType::ApiToken, "API_TOKEN")] {
        let mut row = fixture();
        row.r#type = kind;
        db.session_save(&row).unwrap();
        assert_eq!(db.session_get("fixture-client").unwrap().unwrap().r#type, kind);
        assert_eq!(plain_libs::serde_json::to_value(kind).unwrap(), wire);
        assert_eq!(plain_libs::serde_json::from_value::<SessionType>(plain_libs::serde_json::json!(wire)).unwrap(), kind);
    }
    assert!(plain_libs::serde_json::from_value::<SessionType>(plain_libs::serde_json::json!("FUTURE_SESSION")).is_err());
}
