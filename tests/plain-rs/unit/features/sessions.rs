use super::*;

#[test]
fn session_store_and_authentication_rows_share_one_sqlite_authority() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    let store = SessionStore::new(&db);
    let first = store
        .upsert(SessionInfo {
            client_id: "browser".into(),
            client_name: "Browser".into(),
            ..Default::default()
        })
        .unwrap();
    let row = db.session_get("browser").unwrap().unwrap();
    assert_eq!(row.token, first.token);
    assert_eq!(row.r#type, "CUSTOM");
    assert_eq!(token_key(&row.token).unwrap().len(), 32);
    db.session_delete("browser").unwrap();
    assert!(store.get("browser").unwrap().is_none());
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn session_refresh_preserves_credential_metadata_and_never_restores_a_revoked_row() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    let store = SessionStore::new(&db);
    let mut first = store
        .upsert(SessionInfo {
            client_id: "browser".into(),
            ..Default::default()
        })
        .unwrap();
    let mut row = db.session_get("browser").unwrap().unwrap();
    row.client_ip = "192.0.2.1".into();
    db.session_save(&row).unwrap();
    first.client_name = "Renamed".into();
    let updated = store.upsert(first.clone()).unwrap();
    assert_eq!(updated.token, first.token);
    assert_eq!(
        db.session_get("browser").unwrap().unwrap().url_token,
        row.url_token
    );
    assert_eq!(updated.created_at, first.created_at);
    assert_eq!(
        db.session_get("browser").unwrap().unwrap().client_ip,
        "192.0.2.1"
    );
    store.delete("browser").unwrap();
    store.touch_last_active(&first).unwrap();
    assert!(db.session_get("browser").unwrap().is_none());
}

#[test]
fn malformed_tokens_cannot_be_saved() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    assert!(
        SessionStore::new(&db)
            .upsert(SessionInfo {
                client_id: "browser".into(),
                token: "invalid".into(),
                ..Default::default()
            })
            .is_err()
    );
    assert!(db.session_get("browser").unwrap().is_none());
}

use std::path::Path;
