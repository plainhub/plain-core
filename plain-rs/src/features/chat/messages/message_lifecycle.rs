use crate::features::chat::channels::chat_helper::ChannelDeliveryResult;
use crate::features::chat::channels::chat_helper::build_status_data_json;
use crate::features::chat::channels::chat_helper::compute_status;
use crate::features::chat::enums::ChannelStatus;
use crate::features::chat::enums::ChatStatus;
use crate::infrastructure::db::CHANNEL_COLS;
use crate::infrastructure::db::CHAT_COLS;
use crate::infrastructure::db::DChannel;
use crate::infrastructure::db::DChat;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::PEER_COLS;
use crate::infrastructure::db::chat_store::messages;
use crate::infrastructure::db::now_iso;
use crate::infrastructure::db::row_to_channel;
use crate::infrastructure::db::row_to_chat;
use crate::infrastructure::db::row_to_peer;
use plain_libs::anyhow::{Result, bail};
use plain_libs::rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
#[derive(serde::Serialize)]
pub struct Received {
    pub chat: DChat,
    pub peer: DPeer,
    pub channel: Option<DChannel>,
}
pub fn create(db: &Db, to_id: &str, channel_id: &str, content: &str) -> Result<DChat> {
    Ok(create_many(db, &[(to_id.to_owned(), channel_id.to_owned())], content)?.remove(0))
}
pub fn create_many(db: &Db, targets: &[(String, String)], content: &str) -> Result<Vec<DChat>> {
    let _files = db.app_files_lock()?;
    db.with_conn(|connection| {
        let tx = connection.unchecked_transaction()?;
        let rows = create_in_transaction(&tx, targets, content)?;
        tx.commit()?;
        Ok(rows)
    })
}
pub fn forward_many(db: &Db, id: &str, targets: &[(String, String)]) -> Result<Vec<DChat>> {
    if targets.is_empty() {
        return Ok(vec![]);
    }
    let _files = db.app_files_lock()?;
    db.with_conn(|connection| {
        let tx = connection.unchecked_transaction()?;
        let content: String = tx
            .query_row("SELECT content FROM chats WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .optional()?
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Message unavailable"))?;
        let rows = create_in_transaction(&tx, targets, &content)?;
        tx.commit()?;
        Ok(rows)
    })
}
fn create_in_transaction(
    tx: &plain_libs::rusqlite::Transaction<'_>,
    targets: &[(String, String)],
    content: &str,
) -> Result<Vec<DChat>> {
    messages::validate_content(content)?;
    let mut chats = Vec::with_capacity(targets.len());
    for (to, channel) in targets {
        if !to.is_empty() && !channel.is_empty() {
            bail!("Ambiguous chat target");
        }
        let mut chat = DChat::new("me", to, channel, content);
        if !channel.is_empty() || (!to.is_empty() && to != "local") {
            chat.status = ChatStatus::Pending;
        }
        chats.push(chat);
    }
    let refs = crate::features::chat::attachments::app_file_store::content_refs::collect(
        &plain_libs::serde_json::from_str(content)?,
    );
    for chat in &chats {
        crate::features::chat::attachments::app_file_store::content_refs::claim(tx, refs.clone())?;
        tx.execute("INSERT INTO chats(id,from_id,to_id,channel_id,content,status,status_data,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![chat.id,chat.from_id,chat.to_id,chat.channel_id.as_deref().filter(|value|!value.is_empty()),chat.content,chat.status,chat.status_data.as_deref().filter(|value|!value.is_empty()),chat.created_at,chat.updated_at])?;
    }
    Ok(chats)
}
pub fn receive(
    db: &Db,
    from_id: &str,
    channel_id: &str,
    content: &str,
    signature: &str,
    timestamp: i64,
) -> Result<Option<Received>> {
    let raw = plain_libs::utils::base64::base64_decode(signature);
    let now = crate::features::peers::pairing::now_ms();
    if raw.len() != 64
        || timestamp <= 0
        || now.abs_diff(timestamp) > crate::infrastructure::transport::auth::TIMESTAMP_WINDOW_MS
    {
        bail!("Invalid message receipt");
    }
    let receipt =
        plain_libs::utils::base64::base64_encode(&Sha256::digest(plain_libs::serde_json::to_vec(&(
            from_id,
            channel_id,
            plain_libs::utils::base64::base64_encode(&raw),
            timestamp,
        ))?));
    db.with_conn(|c|->Result<Option<Received>>{
        let tx = c.unchecked_transaction()?;

        let peer=tx.query_row(&format!("SELECT {PEER_COLS} FROM peers WHERE id=?1"),[from_id],row_to_peer).optional()?.ok_or_else(||plain_libs::anyhow::anyhow!("invalid peer"))?;
        let channel=if channel_id.is_empty() {None}else {
            let channel=tx.query_row(&format!("SELECT {CHANNEL_COLS} FROM chat_channels WHERE id=?1"),[channel_id],row_to_channel).optional()?.ok_or_else(||plain_libs::anyhow::anyhow!("Unknown channel"))?;
            if channel.status!=ChannelStatus::Joined {bail!("Channel not joined");}
            if !channel.joined_member_ids().iter().any(|member| member == from_id) {
                bail!("Sender is not a joined channel member");
            }
            Some(channel)
        };
        messages::validate_content(content)?;
        tx.execute("DELETE FROM chat_receipts WHERE sent_at<?1",[crate::infrastructure::db::iso_from_unix_millis(now.saturating_sub(crate::infrastructure::transport::auth::TIMESTAMP_WINDOW_MS as i64))])?;
        if tx.execute("INSERT INTO chat_receipts(receipt_id,peer_id,sent_at) VALUES(?1,?2,?3) ON CONFLICT(receipt_id) DO NOTHING",params![receipt,from_id,crate::infrastructure::db::iso_from_unix_millis(timestamp)])?==0 {tx.commit()?;
            return Ok(None);}
        let chat=DChat::new(from_id,if channel_id.is_empty(){"me"}else{""},channel_id,content);
        tx.execute("INSERT INTO chats(id,from_id,to_id,channel_id,content,status,status_data,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![chat.id,chat.from_id,chat.to_id,chat.channel_id.as_deref().filter(|value|!value.is_empty()),chat.content,chat.status,chat.status_data.as_deref().filter(|value|!value.is_empty()),chat.created_at,chat.updated_at])?;
        tx.commit()?;
        Ok(Some(Received{chat,peer,channel}))
    })
}
#[derive(serde::Deserialize)]
struct StatusData {
    results: Option<Vec<ChannelDeliveryResult>>,
}
pub fn delivery(
    db: &Db,
    id: &str,
    results: Option<Vec<ChannelDeliveryResult>>,
    retry: bool,
) -> Result<Option<DChat>> {
    delivery_impl(db, id, results, retry, None)
}
pub(crate) fn delivery_for_content(
    db: &Db,
    id: &str,
    results: Option<Vec<ChannelDeliveryResult>>,
    retry: bool,
    content: &str,
) -> Result<Option<DChat>> {
    delivery_impl(db, id, results, retry, Some(content))
}
fn delivery_impl(
    db: &Db,
    id: &str,
    results: Option<Vec<ChannelDeliveryResult>>,
    retry: bool,
    expected_content: Option<&str>,
) -> Result<Option<DChat>> {
    db.with_conn(|c| -> Result<Option<DChat>> {
        let tx = c.unchecked_transaction()?;
        let Some(mut chat) = tx
            .query_row(
                &format!("SELECT {CHAT_COLS} FROM chats WHERE id=?1"),
                [id],
                row_to_chat,
            )
            .optional()?
        else {
            return Ok(None);
        };
        if expected_content.is_some_and(|content| content != chat.content) {
            bail!("Message content changed during delivery");
        }
        let (status, data) = if let Some(mut results) = results {
            let mut ids = std::collections::HashSet::new();
            if results
                .iter()
                .any(|r| r.peer_id.is_empty() || !ids.insert(r.peer_id.clone()))
            {
                bail!("Invalid delivery recipients");
            }
            if let Some(data) = chat.status_data.as_deref().filter(|_| retry) {
                let old: StatusData = plain_libs::serde_json::from_str(data)?;
                let mut merged: Vec<_> = old
                    .results
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|r| !ids.contains(&r.peer_id))
                    .collect();
                merged.append(&mut results);
                results = merged;
            }
            (compute_status(&results), build_status_data_json(&results))
        } else {
            (ChatStatus::Failed, String::new())
        };
        chat.status = status;
        chat.status_data = (!data.is_empty()).then_some(data);
        chat.updated_at = now_iso();
        tx.execute(
            "UPDATE chats SET status=?2,status_data=?3,updated_at=?4 WHERE id=?1",
            params![
                chat.id,
                chat.status,
                chat.status_data
                    .as_deref()
                    .filter(|value| !value.is_empty()),
                chat.updated_at
            ],
        )?;
        tx.commit()?;
        Ok(Some(chat))
    })
}
#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/messages/message_lifecycle.rs"]
mod tests;
