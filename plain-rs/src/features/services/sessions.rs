use crate::infrastructure::db::Db;
use crate::infrastructure::db::SessionRow;
use crate::runtime::Runtime as CoreRuntime;

pub fn native(state: &CoreRuntime) -> plain_libs::anyhow::Result<SessionRow> {
    let id = state
        .native_client_id()
        .map_err(plain_libs::anyhow::Error::msg)?;
    let session = state
        .db
        .session_get(&id)?
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Native session unavailable"))?;
    plain_libs::anyhow::ensure!(
        session.token == state.token.as_ref(),
        "Native session changed"
    );
    Ok(session)
}

/// The key a remote client's frames are encrypted with: its session token.
pub fn key(db: &Db, client_id: &str) -> plain_libs::anyhow::Result<Option<Vec<u8>>> {
    Ok(db.session_get(client_id)?.and_then(|row| {
        let key = plain_libs::utils::base64::base64_decode(&row.token);
        (key.len() == 32).then_some(key)
    }))
}

pub fn local_key(state: &CoreRuntime, client_id: &str) -> Option<Vec<u8>> {
    if state.native_client_id().ok().as_deref() != Some(client_id) {
        return None;
    }
    let session = native(state).ok()?;
    crate::features::sessions::token_key(&session.token)
        .ok()
        .map(Vec::from)
}

pub fn touch(state: &CoreRuntime, client_id: &str) -> plain_libs::anyhow::Result<()> {
    let now = plain_libs::chrono::Utc::now().timestamp_millis();
    state.db.session_touch(&[(
        client_id.to_owned(),
        plain_libs::utils::dbtime::iso_from_unix_millis(now),
    )])?;
    Ok(())
}

pub fn create(state: &CoreRuntime, name: String) -> plain_libs::anyhow::Result<SessionRow> {
    let now = plain_libs::utils::dbtime::now_iso();
    let row = SessionRow {
        url_token: plain_libs::crypto::gen_token(),
        client_id: crate::features::client_identity::generate(),
        name,
        r#type: crate::features::sessions::SessionType::ApiToken,
        client_ip: String::new(),
        os_name: String::new(),
        os_version: String::new(),
        app_name: String::new(),
        app_version: String::new(),
        token: plain_libs::utils::base64::base64_encode(&plain_libs::crypto::random_bytes(32)),
        last_seen_at: None,
        created_at: now.clone(),
        updated_at: now,
    };
    state.db.session_save(&row)?;
    Ok(row)
}

pub fn rename(state: &CoreRuntime, client_id: &str, name: &str) -> plain_libs::anyhow::Result<bool> {
    Ok(state
        .db
        .session_rename(client_id, name, &plain_libs::utils::dbtime::now_iso())?
        == 1)
}

pub fn delete(state: &CoreRuntime, client_id: &str) -> plain_libs::anyhow::Result<bool> {
    state
        .clients
        .revoke(client_id)
        .map_err(plain_libs::anyhow::Error::msg)?;
    Ok(state.db.session_delete(client_id)? > 0)
}
