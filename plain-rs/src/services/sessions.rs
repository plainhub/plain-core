use plain_rs::db::{Db, SessionRow};
use plain_rs::runtime::Runtime as CoreRuntime;
use serde_json::json;

/// The key a remote client's frames are encrypted with: its session token.
pub fn key(db: &Db, client_id: &str) -> anyhow::Result<Option<Vec<u8>>> {
    Ok(db.session_get(client_id)?.and_then(|row| {
        let key = plain_rs::utils::base64::base64_decode(&row.token);
        (key.len() == 32).then_some(key)
    }))
}

pub fn local_key(state: &CoreRuntime, client_id: &str) -> Option<Vec<u8>> {
    if state.native_client_id().ok().as_deref() != Some(client_id) {
        return None;
    }
    let token = state
        .prefs
        .get::<String>(plain_rs::prefs::pref_keys::URL_TOKEN)
        .ok()??;
    let key = plain_rs::utils::base64::base64_decode(&token);
    (key.len() == 32).then_some(key)
}

pub fn touch(state: &CoreRuntime, client_id: &str) -> anyhow::Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    state.db.session_touch(&[(
        client_id.to_owned(),
        plain_rs::utils::dbtime::iso_from_unix_millis(now),
    )])?;
    if let Err(error) = state.host.notify("systemWebActivity", json!({})) {
        log::warn!("Web activity callback: {error}");
    }
    Ok(())
}

pub fn create(state: &CoreRuntime, name: String) -> anyhow::Result<SessionRow> {
    let now = plain_rs::utils::dbtime::now_iso();
    let row = SessionRow {
        url_token: plain_rs::crypto::gen_token(),
        client_id: uuid::Uuid::new_v4().to_string(),
        name,
        r#type: "CUSTOM".into(),
        client_ip: String::new(),
        os_name: String::new(),
        os_version: String::new(),
        browser_name: String::new(),
        browser_version: String::new(),
        token: plain_rs::utils::base64::base64_encode(&plain_rs::crypto::random_bytes(32)),
        last_seen_at: None,
        created_at: now.clone(),
        updated_at: now,
    };
    state.db.session_save(&row)?;
    Ok(row)
}

pub fn rename(state: &CoreRuntime, client_id: &str, name: &str) -> anyhow::Result<bool> {
    Ok(state
        .db
        .session_rename(client_id, name, &plain_rs::utils::dbtime::now_iso())?
        == 1)
}

pub fn delete(state: &CoreRuntime, client_id: &str) -> anyhow::Result<bool> {
    state
        .clients
        .revoke(client_id)
        .map_err(anyhow::Error::msg)?;
    Ok(state.db.session_delete(client_id)? > 0)
}
