use crate::features::chat::events;
use crate::features::chat::messages::target as message_target;
use crate::infrastructure::db::DChat;
use crate::infrastructure::events::WsEvent;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use plain_libs::serde_json::json;
pub fn committed(state: &CoreRuntime, chat: &DChat, event: &'static str) {
    state.previews.request(&chat.id);
    let _ = state.events.send(WsEvent::broadcast(
        event,
        json!([crate::features::chat::service::chat_to_json(chat)]).to_string(),
    ));
}
pub async fn deliver(state: &CoreRuntime, row: DChat) -> Result<DChat> {
    if row.channel_id.is_none() && (row.to_id.is_empty() || row.to_id == "local") {
        return Ok(row);
    }
    crate::features::services::chat::chat_delivery::deliver(state, &row.id)
        .await?
        .chat
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Message removed"))
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
    crate::features::chat::messages::message_lifecycle::create_many(&state.db, &targets, content)
}

pub async fn forward_many(
    state: &CoreRuntime,
    id: &str,
    targets: Vec<String>,
    note: Option<&str>,
) -> Result<Vec<DChat>> {
    let targets = validate_targets(&targets)?;
    let rows = crate::features::chat::messages::message_lifecycle::forward_many(
        &state.db, id, &targets, note,
    )?;
    for (original, note) in &rows {
        committed(state, original, events::WS_MESSAGE_CREATED);
        if let Some(note) = note {
            committed(state, note, events::WS_MESSAGE_CREATED);
        }
    }

    Ok(
        futures_util::future::join_all(rows.into_iter().map(|(original, note)| async move {
            let original = deliver_committed_forward(state, original).await;
            let note = match note {
                Some(note) => Some(deliver_committed_forward(state, note).await),
                None => None,
            };
            [Some(original), note]
        }))
        .await
        .into_iter()
        .flatten()
        .flatten()
        .collect(),
    )
}

async fn deliver_committed_forward(state: &CoreRuntime, row: DChat) -> DChat {
    match deliver(state, row.clone()).await {
        Ok(delivered) => delivered,
        Err(error) => {
            // This request already created the outgoing rows. Reporting a
            // mutation-wide error would invite the client to forward twice.
            log::warn!("Committed forward did not reach a final delivery state: {error}");
            crate::infrastructure::db::chat_store::messages::get(&state.db, &row.id)
                .ok()
                .flatten()
                .unwrap_or(row)
        }
    }
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

pub fn delete_query(state: &CoreRuntime, query: &str) -> Result<usize> {
    let ids = if query.trim().is_empty() {
        vec![]
    } else {
        crate::infrastructure::db::chat_store::messages::ids(&state.db, query)?
    };
    delete(state, ids)
}

pub fn delete(state: &CoreRuntime, ids: Vec<String>) -> Result<usize> {
    let count = crate::features::chat::attachments::app_file_store::chat_deletion::delete(
        &state.db,
        &state.directory,
        crate::features::chat::attachments::app_file_store::chat_deletion::Selection::Ids(&ids),
    )?;
    let _ = state.events.send(WsEvent::broadcast(
        events::WS_MESSAGE_DELETED,
        json!(format!("ids={}", ids.join(","))).to_string(),
    ));
    Ok(count)
}

pub async fn retry(
    state: &CoreRuntime,
    id: String,
    recipient_ids: Option<Vec<String>>,
) -> Result<DChat> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = std::sync::Arc::new(std::sync::Mutex::new(Some(sender)));
    let state = state.clone();
    tokio::spawn(async move {
        let mut stop = state.stop.clone();
        let result = tokio::select! {
            _=stop.changed()=>Err(plain_libs::anyhow::anyhow!("Chat service stopped")),
            result=crate::features::services::chat::chat_delivery::deliver_observed(&state,&id,recipient_ids,crate::infrastructure::transport::router::Attempt::Retry,|chat| {
                if let Some(sender)=sender.lock().unwrap().take() {let _=sender.send(Ok(chat.clone()));}
            })=>result.and_then(|receipt|receipt.chat.ok_or_else(||plain_libs::anyhow::anyhow!("Message unavailable"))),
        };
        if let Some(sender) = sender.lock().unwrap().take() {
            let _ = sender.send(result);
        }
    });
    receiver.await?
}
