use crate::infrastructure::db::Db;
use crate::infrastructure::db::SessionRow;
use crate::runtime::Runtime as CoreRuntime;

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
    crate::features::sessions::token_key(&state.token)
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
    let name = name.trim().to_owned();
    plain_libs::anyhow::ensure!(!name.is_empty(), "API token name is required");
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

pub fn rename(
    state: &CoreRuntime,
    client_id: &str,
    name: &str,
) -> plain_libs::anyhow::Result<bool> {
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

pub fn revoke(
    state: &CoreRuntime,
    actor_id: &str,
    target_id: &str,
) -> plain_libs::anyhow::Result<bool> {
    remove(
        state,
        actor_id,
        target_id,
        crate::features::audit_events::AuditEvent::RevokeSession,
    )
}

pub fn logout(state: &CoreRuntime, actor_id: &str) -> plain_libs::anyhow::Result<bool> {
    remove(
        state,
        actor_id,
        actor_id,
        crate::features::audit_events::AuditEvent::Logout,
    )
}

fn remove(
    state: &CoreRuntime,
    actor_id: &str,
    target_id: &str,
    event: fn(
        crate::features::audit_events::SessionEventData,
    ) -> crate::features::audit_events::AuditEvent,
) -> plain_libs::anyhow::Result<bool> {
    use crate::features::audit_events::{EventSession, SessionEventData};
    let actor = super::audit_events::client(state, actor_id)?;
    let Some(target) = state.db.session_get(target_id)? else {
        return Ok(false);
    };
    let data = SessionEventData {
        session: EventSession {
            client_id: target.client_id,
            client_name: target.name,
            r#type: target.r#type,
        },
    };
    if !delete(state, target_id)? {
        return Ok(false);
    }
    state
        .audit_events
        .record(event(data), Some(actor))
        .map_err(plain_libs::anyhow::Error::msg)?;
    Ok(true)
}
