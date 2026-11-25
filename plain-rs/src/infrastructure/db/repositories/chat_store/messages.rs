use crate::infrastructure::db::DChat;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::repositories::chat::CHAT_COLS;
use crate::infrastructure::db::repositories::chat::row_to_chat;
use crate::infrastructure::db::repositories::chat_store::SaveMode;
use crate::infrastructure::db::repositories::chat_store::validate;
use plain_libs::anyhow::{Result, bail};
use plain_libs::rusqlite::{OptionalExtension, params};
pub fn get(db: &Db, id: &str) -> Result<Option<DChat>> {
    Ok(db.with_conn(|c| {
        c.query_row(
            &format!("SELECT {CHAT_COLS} FROM chats WHERE id=?1"),
            [id],
            row_to_chat,
        )
        .optional()
    })?)
}
pub fn all(db: &Db) -> Result<Vec<DChat>> {
    Ok(db.with_conn(|c| {
        let mut s = c.prepare(&format!(
            "SELECT {CHAT_COLS} FROM chats ORDER BY julianday(created_at),id"
        ))?;
        s.query_map([], row_to_chat)?
            .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
    })?)
}
pub fn save(db: &Db, rows: &[DChat], mode: SaveMode) -> Result<()> {
    for row in rows {
        validate(&row.id, &row.created_at, &row.updated_at)?;
        validate_content(&row.content)?;
    }
    db.with_conn(|c|->Result<()>{ let tx=c.unchecked_transaction()?;
 for row in rows {
  let sql=match mode { SaveMode::Insert=>"INSERT INTO chats(id,from_id,to_id,channel_id,content,status,status_data,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)", SaveMode::Update=>"UPDATE chats SET from_id=?2,to_id=?3,channel_id=?4,content=?5,status=?6,status_data=?7,created_at=?8,updated_at=?9 WHERE id=?1",SaveMode::Upsert=>"INSERT INTO chats(id,from_id,to_id,channel_id,content,status,status_data,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO UPDATE SET from_id=excluded.from_id,to_id=excluded.to_id,channel_id=excluded.channel_id,content=excluded.content,status=excluded.status,status_data=excluded.status_data,updated_at=excluded.updated_at" };
  let changed=tx.execute(sql,params![row.id,row.from_id,row.to_id,row.channel_id.as_deref().filter(|value|!value.is_empty()),row.content,row.status,row.status_data.as_deref().filter(|value|!value.is_empty()),plain_libs::utils::dbtime::normalize(&row.created_at)?,plain_libs::utils::dbtime::normalize(&row.updated_at)?])?;
  if changed!=1 { bail!("messages record missing"); }
 } tx.commit()?; Ok(()) })
}
pub fn delete(db: &Db, ids: &[String]) -> Result<usize> {
    db.with_conn(|c| -> Result<usize> {
        let tx = c.unchecked_transaction()?;
        let mut n = 0;
        for id in ids {
            n += tx.execute("DELETE FROM chats WHERE id=?1", [id])?;
        }
        tx.commit()?;
        Ok(n)
    })
}
pub(crate) fn validate_content(content: &str) -> Result<plain_libs::serde_json::Value> {
    let value: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(content)?;
    let message = &value["value"];
    match value["type"].as_str() {
        Some("SHARE") => {
            plain_libs::serde_json::from_value::<crate::features::chat::messages::share_card::Card>(message.clone())?;
            return Ok(value);
        }
        Some("MESSAGE") => {}
        _ => bail!("invalid chat content envelope"),
    }
    if !matches!(message["format"].as_str(), Some("PLAIN" | "MARKDOWN")) {
        bail!("invalid chat text format");
    }
    let items = message["items"].as_array()
        .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid attachment items"))?;
    let has_text = match &message["text"] {
        plain_libs::serde_json::Value::String(text) => !text.trim().is_empty(),
        plain_libs::serde_json::Value::Null => false,
        _ => bail!("invalid chat text"),
    };
    if !has_text && items.is_empty() {
        bail!("Chat message requires text or an attachment");
    }
    let mut ids = std::collections::HashSet::new();
    for item in items {
        if !matches!(item["type"].as_str(), Some("IMAGE" | "FILE")) {
            bail!("invalid attachment type");
        }
        let attachment = &item["attachment"];
        validate_attachment(attachment)?;
        let id = attachment["id"].as_str().filter(|id| !id.is_empty())
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Chat attachment ID must not be empty"))?;
        if !ids.insert(id) {
            bail!("Chat attachment IDs must be unique");
        }
    }
    Ok(value)
}
fn validate_attachment(item: &plain_libs::serde_json::Value) -> Result<()> {
    if !item.is_object()
        || !item
            .get("uri")
            .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()))
    {
        bail!("invalid attachment URI");
    }
    if !item
        .get("size")
        .is_some_and(|v| v.as_i64().is_some_and(|n| n >= 0))
    {
        bail!("invalid attachment size");
    }
    for field in ["id", "fileName", "summary"] {
        if item.get(field).is_some_and(|v| !v.is_string()) {
            bail!("invalid attachment {field}");
        }
    }
    for field in ["width", "height"] {
        if item.get(field).is_some_and(|v| {
            !v.as_i64()
                .is_some_and(|n| (0..=i64::from(i32::MAX)).contains(&n))
        }) {
            bail!("invalid attachment {field}");
        }
    }
    if item.get("durationMs").is_some_and(|value| !value.as_i64().is_some_and(|n| n >= 0)) {
        bail!("invalid attachment duration");
    }

    Ok(())
}
pub struct Filter {
    pub peer: Option<String>,
    pub channel: Option<String>,
    pub text: String,
}

impl Filter {
    fn condition(&self) -> Result<&'static str> {
        match (&self.peer, &self.channel) {
            (Some(_), Some(_)) => bail!("Choose either peer or channel, not both"),
            (Some(_), None) => Ok("channel_id IS NULL AND (from_id=?3 OR to_id=?3)"),
            (None, Some(_)) => Ok("channel_id=?3"),
            (None, None) => Ok("?3 IS NULL"),
        }
    }

    fn target(&self) -> Option<&str> {
        self.peer.as_deref().or(self.channel.as_deref())
    }

    fn pattern(&self) -> String {
        format!(
            "%{}%",
            self.text.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
        )
    }
}

const TEXT_CONDITION: &str = r"(?1='' OR content LIKE ?2 ESCAPE '\')";

pub fn list(
    db: &Db,
    filter: &Filter,
    offset: i64,
    limit: Option<i64>,
    descending: bool,
) -> Result<Vec<DChat>> {
    if offset < 0 || limit.is_some_and(|value| value < 0) {
        bail!("Invalid chat pagination");
    }
    let condition = filter.condition()?;
    let pattern = filter.pattern();
    let order = if descending { "DESC" } else { "ASC" };
    Ok(db.with_conn(|c| {
        let mut statement = c.prepare(&format!(
            "SELECT {CHAT_COLS} FROM chats WHERE {TEXT_CONDITION} AND {condition}
             ORDER BY julianday(created_at) {order}, id {order} LIMIT ?4 OFFSET ?5"
        ))?;
        statement
            .query_map(params![filter.text, pattern, filter.target(), limit.unwrap_or(-1), offset], row_to_chat)?
            .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
    })?)
}

pub fn count(db: &Db, filter: &Filter) -> Result<i64> {
    let condition = filter.condition()?;
    let pattern = filter.pattern();
    Ok(db.with_conn(|c| {
        c.query_row(
            &format!("SELECT count(*) FROM chats WHERE {TEXT_CONDITION} AND {condition}"),
            params![filter.text, pattern, filter.target()],
            |row| row.get(0),
        )
    })?)
}

pub fn latest_conversation_candidates(db: &Db) -> Result<Vec<DChat>> {
    Ok(db.with_conn(|c| {
        let mut statement = c.prepare(&format!(
            "SELECT {CHAT_COLS} FROM chats c WHERE julianday(c.created_at)=(
                SELECT max(julianday(other.created_at)) FROM chats other
                WHERE (c.channel_id IS NOT NULL AND other.channel_id=c.channel_id)
                   OR (c.channel_id IS NULL AND other.channel_id IS NULL AND other.from_id=c.from_id AND other.to_id=c.to_id)
            ) ORDER BY julianday(created_at) DESC, id DESC"
        ))?;
        statement.query_map([], row_to_chat)?.collect::<plain_libs::rusqlite::Result<Vec<_>>>()
    })?)
}

pub fn status(
    db: &Db,
    id: &str,
    status: crate::features::chat::enums::ChatStatus,
    data: Option<&str>,
) -> Result<bool> {
    if let Some(data) = data {
        if !data.is_empty() {
            plain_libs::serde_json::from_str::<plain_libs::serde_json::Value>(data)?;
        }
    }
    Ok(db.with_conn(|c| {
        c.execute(
            "UPDATE chats SET status=?2,status_data=coalesce(?3,status_data) WHERE id=?1",
            params![id, status, data],
        )
    })? == 1)
}
pub fn content(db: &Db, id: &str, content: &str) -> Result<bool> {
    validate_content(content)?;
    Ok(db.with_conn(|c| {
        c.execute(
            "UPDATE chats SET content=?2 WHERE id=?1",
            params![id, content],
        )
    })? == 1)
}
pub fn ids(db: &Db, query: &str) -> Result<Vec<String>> {
    let fields = plain_libs::utils::search_dsl::parse(query);
    if let Some(field) = fields.iter().find(|f| f.name == "ids") {
        return Ok(field
            .value
            .split(',')
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .collect());
    }
    let channel = fields
        .iter()
        .find(|f| f.name == "channel")
        .map(|f| f.value.clone());
    let peer = if channel.is_none() {
        fields
            .iter()
            .find(|f| f.name == "peer")
            .map(|f| f.value.clone())
    } else {
        None
    };
    if channel.is_none() && peer.is_none() {
        return Ok(Vec::new());
    }
    let rows = list(
        db,
        &Filter {
            peer,
            channel,
            text: String::new(),
        },
        0,
        None,
        false,
    )?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}
