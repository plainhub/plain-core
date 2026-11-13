use crate::infrastructure::db::CHAT_COLS;
use crate::infrastructure::db::DChat;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::row_to_chat;
use plain_libs::anyhow::{Result, ensure};
use plain_libs::rusqlite::{OptionalExtension, params};
use plain_libs::serde_json::Value;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub id: String,
    pub ip: String,
    pub port: u16,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub share_id: String,
    pub url_token: String,
    pub peer_info: Peer,
    pub name: String,
    #[serde(default)]
    pub item_count: i64,
    #[serde(default)]
    pub total_size: i64,
    #[serde(default)]
    pub expires_at: Option<plain_libs::chrono::DateTime<plain_libs::chrono::Utc>>,
}
fn parse(chat: &DChat) -> Result<(Value, Card)> {
    let value: Value = plain_libs::serde_json::from_str(&chat.content)?;
    ensure!(value["type"] == "SHARE", "Message is not a share card");
    let card = plain_libs::serde_json::from_value(value["value"].clone())?;
    Ok((value, card))
}
pub fn find(db: &Db, share_id: &str, token: &str) -> Result<Option<(String, Card)>> {
    db.with_conn(|conn| -> Result<_> {
        let row = conn.query_row("SELECT id,content FROM chats WHERE json_valid(content) AND json_extract(content,'$.type')='SHARE' AND json_extract(content,'$.value.shareId')=?1 AND json_extract(content,'$.value.urlToken')=?2 ORDER BY updated_at DESC,id DESC LIMIT 1",params![share_id,token], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).optional()?;
        row.map(|(id,content)| -> Result<_> { let value: Value = plain_libs::serde_json::from_str(&content)?; Ok((id,plain_libs::serde_json::from_value(value["value"].clone())?)) }).transpose()
    })
}
pub fn current(db: &Db, id: &str, expected: &Card) -> Result<String> {
    let row = crate::infrastructure::db::chat_store::messages::get(db, id)?
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Share message removed"))?;
    let (_, card) = parse(&row)?;
    ensure!(card == *expected, "Share card changed");
    Ok(row.content)
}
pub fn refresh(
    db: &Db,
    id: &str,
    expected: &Card,
    expected_content: &str,
    host: &str,
    port: u16,
    name: &str,
    expires_at: Option<i64>,
) -> Result<(Card, Option<DChat>)> {
    db.with_conn(|conn| -> Result<(Card, Option<DChat>)> {
        let tx = conn.unchecked_transaction()?;
        let Some(mut row) = tx
            .query_row(
                &format!("SELECT {CHAT_COLS} FROM chats WHERE id=?1"),
                [id],
                row_to_chat,
            )
            .optional()?
        else {
            plain_libs::anyhow::bail!("Share message removed");
        };
        ensure!(row.content == expected_content, "Share card changed");
        let (mut content, current) = parse(&row)?;
        ensure!(current == *expected, "Share card changed");
        let mut fresh = current.clone();
        fresh.peer_info.ip = host.into();
        fresh.peer_info.port = port;
        if !name.is_empty() {
            fresh.name = name.into();
        }
        fresh.expires_at = expires_at
            .map(|ms| {
                plain_libs::chrono::DateTime::from_timestamp_millis(ms)
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid share expiry"))
            })
            .transpose()?;
        if fresh == current {
            tx.commit()?;
            return Ok((current, None));
        }
        let value = content["value"]
            .as_object_mut()
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid share card"))?;
        value.insert("name".into(), Value::String(fresh.name.clone()));
        value.insert(
            "expiresAt".into(),
            plain_libs::serde_json::to_value(fresh.expires_at)?,
        );
        value.insert(
            "peerInfo".into(),
            plain_libs::serde_json::to_value(&fresh.peer_info)?,
        );
        let updated = plain_libs::serde_json::to_string(&content)?;
        ensure!(
            tx.execute(
                "UPDATE chats SET content=?2 WHERE id=?1 AND content=?3",
                params![id, updated, row.content]
            )? == 1,
            "Share card changed"
        );
        row.content = updated;
        tx.commit()?;
        Ok((fresh, Some(row)))
    })
}
#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/messages/share_card.rs"]
mod tests;
