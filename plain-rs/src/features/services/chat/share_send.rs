use crate::features::files::shares::Service;
use crate::infrastructure::db::DChat;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use std::sync::Arc;

struct PendingShare {
    db: Arc<crate::infrastructure::db::Db>,
    id: Option<String>,
}
impl Drop for PendingShare {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            if let Err(error) = self.db.share_delete(&id) {
                log::error!("Unable to roll back unbound folder share: {error}");
            }
        }
    }
}

pub async fn existing(
    state: &CoreRuntime,
    service: Arc<Service>,
    targets: Vec<String>,
    id: String,
) -> Result<Vec<DChat>> {
    crate::features::services::chat::chat_actions::validate_targets(&targets)?;
    if targets.is_empty() {
        return Ok(vec![]);
    }
    let content = crate::features::services::chat::share_card::content(state, service, id).await?;
    crate::features::services::chat::chat_actions::send_many(state, targets, content).await
}

pub async fn folder(
    state: &CoreRuntime,
    service: Arc<Service>,
    targets: Vec<String>,
    path: String,
    name: String,
    expires_at: Option<String>,
) -> Result<Vec<DChat>> {
    crate::features::services::chat::chat_actions::validate_targets(&targets)?;
    if targets.is_empty() {
        return Ok(vec![]);
    }
    if let Some(expires) = expires_at.as_deref() {
        ensure!(
            plain_libs::chrono::DateTime::parse_from_rfc3339(expires)? > plain_libs::chrono::Utc::now(),
            "Share has expired"
        );
    }
    ensure!(!path.trim().is_empty(), "Share folder path required");
    let path = state.file_platform.resolve_reference(&path)?;
    state
        .file_platform
        .authorize_metadata_paths(std::slice::from_ref(&path))
        .await?;
    let create = service.clone();
    let db = state.db.clone();
    let (share, mut pending) = tokio::task::spawn_blocking(move || -> Result<_> {
        ensure!(std::fs::metadata(&path)?.is_dir(), "Share folder required");
        let share = create.create(name, vec![path], true, expires_at)?;
        let pending = PendingShare {
            db,
            id: Some(share.id.clone()),
        };
        Ok((share, pending))
    })
    .await??;
    let content =
        crate::features::services::chat::share_card::content(state, service, share.id).await?;
    let rows = crate::features::services::chat::chat_actions::prepare_send_many(
        state, &targets, &content,
    )?;
    pending.id = None;
    let _ = state
        .events
        .send(crate::infrastructure::events::WsEvent::broadcast(
            "CONTENT_CHANGED",
            "{}".into(),
        ));
    crate::features::services::chat::chat_actions::deliver_many(state, rows).await
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/chat/share_send.rs"]
mod tests;
