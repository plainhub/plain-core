use anyhow::{Result, ensure};
use plain_rs::chat::events;
use plain_rs::chat::messages::share_send;
use plain_rs::chat::messages::target as message_target;
use plain_rs::db::DChat;
use plain_rs::events::WsEvent;
use plain_rs::runtime::Runtime as CoreRuntime;
use serde_json::json;
pub fn committed(state: &CoreRuntime, chat: &DChat, event: &'static str) {
    state.previews.request(&chat.id);
    let _ = state.events.send(WsEvent::broadcast(
        event,
        json!([plain_rs::chat::service::chat_to_json(chat)]).to_string(),
    ));
}
pub async fn deliver(state: &CoreRuntime, row: DChat) -> Result<DChat> {
    if row.channel_id.is_none() && (row.to_id.is_empty() || row.to_id == "local") {
        return Ok(row);
    }
    plain_rs::services::chat::chat_delivery::deliver(state, &row.id, None)
        .await?
        .chat
        .ok_or_else(|| anyhow::anyhow!("Message removed"))
}
pub async fn send(state: &CoreRuntime, target: &str, content: &str) -> Result<DChat> {
    let (to, channel) = message_target::parse(target)?;
    let (content, _imports) = prepare_content(state, content)?;
    let row =
        plain_rs::chat::messages::message_lifecycle::create(&state.db, &to, &channel, &content)?;
    committed(state, &row, events::WS_MESSAGE_CREATED);
    deliver(state, row).await
}
pub async fn send_many(
    state: &CoreRuntime,
    targets: Vec<String>,
    content: String,
) -> Result<Vec<DChat>> {
    let rows = prepare_send_many(state, &targets, &content)?;
    deliver_many(state, rows).await
}

pub(super) fn prepare_send_many(
    state: &CoreRuntime,
    targets: &[String],
    content: &str,
) -> Result<Vec<DChat>> {
    let targets = validate_targets(targets)?;
    if targets.is_empty() {
        return Ok(vec![]);
    }
    let (content, _imports) = prepare_content(state, content)?;
    plain_rs::chat::messages::message_lifecycle::create_many(&state.db, &targets, &content)
}

pub async fn forward_many(
    state: &CoreRuntime,
    id: &str,
    targets: Vec<String>,
) -> Result<Vec<DChat>> {
    let targets = validate_targets(&targets)?;
    let rows = plain_rs::chat::messages::message_lifecycle::forward_many(&state.db, id, &targets)?;
    deliver_many(state, rows).await
}

pub(super) async fn deliver_many(state: &CoreRuntime, rows: Vec<DChat>) -> Result<Vec<DChat>> {
    for row in &rows {
        committed(state, row, events::WS_MESSAGE_CREATED);
    }
    futures_util::future::join_all(rows.into_iter().map(|row| deliver(state, row)))
        .await
        .into_iter()
        .collect()
}

pub fn validate_targets(targets: &[String]) -> Result<Vec<(String, String)>> {
    ensure!(targets.len() <= 128, "Too many share targets");
    targets
        .iter()
        .map(|target| message_target::parse(target))
        .collect()
}

fn prepare_content(state: &CoreRuntime, content: &str) -> Result<(String, Option<Imported>)> {
    let value: serde_json::Value = serde_json::from_str(content)?;
    if value["type"] != "TEXT" {
        return Ok((content.to_owned(), None));
    }
    let text = value["value"]["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Text required"))?;
    if text.encode_utf16().count() <= 2048 {
        return Ok((content.to_owned(), None));
    }
    let content = share_send::text(&state.db, &state.directory, text)?;
    let imports = Imported {
        db: state.db.clone(),
        directory: state.directory.clone(),
        ids: plain_rs::chat::attachments::app_file_store::content_reference_ids(&content),
    };
    Ok((content.to_string(), Some(imports)))
}
pub fn delete_query(state: &CoreRuntime, query: &str) -> Result<usize> {
    let ids = if query.trim().is_empty() {
        vec![]
    } else {
        plain_rs::db::chat_store::messages::ids(&state.db, query)?
    };
    delete(state, ids)
}

pub fn delete(state: &CoreRuntime, ids: Vec<String>) -> Result<usize> {
    let count = plain_rs::chat::attachments::app_file_store::chat_deletion::delete(
        &state.db,
        &state.directory,
        plain_rs::chat::attachments::app_file_store::chat_deletion::Selection::Ids(&ids),
    )?;
    let _ = state.events.send(WsEvent::broadcast(
        events::WS_MESSAGE_DELETED,
        json!(format!("ids={}", ids.join(","))).to_string(),
    ));
    Ok(count)
}

struct Imported {
    db: std::sync::Arc<plain_rs::db::Db>,
    directory: std::path::PathBuf,
    ids: std::collections::BTreeMap<String, i64>,
}
impl Drop for Imported {
    fn drop(&mut self) {
        if let Err(error) = plain_rs::chat::attachments::app_file_store::release_unbound(
            &self.db,
            &self.directory,
            std::mem::take(&mut self.ids),
        ) {
            log::warn!("Unable to release unused chat imports: {error}");
        }
    }
}
pub async fn retry(state: &CoreRuntime, id: String) -> Result<DChat> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = std::sync::Arc::new(std::sync::Mutex::new(Some(sender)));
    let state = state.clone();
    tokio::spawn(async move {
        let mut stop = state.stop.clone();
        let result = tokio::select! {
            _=stop.changed()=>Err(anyhow::anyhow!("Chat service stopped")),
            result=plain_rs::services::chat::chat_delivery::deliver_observed(&state,&id,None,plain_rs::transport::router::Attempt::Retry,|chat| {
                if let Some(sender)=sender.lock().unwrap().take() {let _=sender.send(Ok(chat.clone()));}
            })=>result.and_then(|receipt|receipt.chat.ok_or_else(||anyhow::anyhow!("Message unavailable"))),
        };
        if let Some(sender) = sender.lock().unwrap().take() {
            let _ = sender.send(result);
        }
    });
    receiver.await?
}
