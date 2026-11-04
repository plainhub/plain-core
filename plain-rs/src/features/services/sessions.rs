use crate::infrastructure::db::Db;
use crate::infrastructure::db::SessionRow;
use crate::runtime::Runtime as CoreRuntime;

/// The key a remote client's frames are encrypted with: its session token.
pub fn key(db: &Db, client_id: &str) -> anyhow::Result<Option<Vec<u8>>> {
    Ok(db.session_get(client_id)?.and_then(|row| {
        let key = plain_libs::utils::base64::base64_decode(&row.token);
        (key.len() == 32).then_some(key)
    }))
}

pub fn local_key(state: &CoreRuntime, client_id: &str) -> Option<Vec<u8>> {
    if state.native_client_id().ok().as_deref() != Some(client_id) {
        return None;
    }
    let token = state
        .prefs
        .get::<String>(crate::features::prefs::pref_keys::URL_TOKEN)
        .ok()??;
    let key = plain_libs::utils::base64::base64_decode(&token);
    (key.len() == 32).then_some(key)
}

pub fn touch(state: &CoreRuntime, client_id: &str) -> anyhow::Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    state.db.session_touch(&[(
        client_id.to_owned(),
        plain_libs::utils::dbtime::iso_from_unix_millis(now),
    )])?;
    Ok(())
}

pub fn create(state: &CoreRuntime, name: String) -> anyhow::Result<SessionRow> {
    let now = plain_libs::utils::dbtime::now_iso();
    let row = SessionRow {
        url_token: plain_libs::crypto::gen_token(),
        client_id: uuid::Uuid::new_v4().to_string(),
        name,
        r#type: "CUSTOM".into(),
        client_ip: String::new(),
        os_name: String::new(),
        os_version: String::new(),
        browser_name: String::new(),
        browser_version: String::new(),
        token: plain_libs::utils::base64::base64_encode(&plain_libs::crypto::random_bytes(32)),
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
        .session_rename(client_id, name, &plain_libs::utils::dbtime::now_iso())?
        == 1)
}

pub fn delete(state: &CoreRuntime, client_id: &str) -> anyhow::Result<bool> {
    state
        .clients
        .revoke(client_id)
        .map_err(anyhow::Error::msg)?;
    Ok(state.db.session_delete(client_id)? > 0)
}
