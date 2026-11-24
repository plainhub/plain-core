use crate::infrastructure::db::key_blob;
use super::{assets::Assets, rows::for_each, short_id};
use crate::features::chat::channels::messages::ChannelMember;
use crate::features::chat::enums::MemberStatus;
use crate::infrastructure::types::Instant;
use plain_libs::anyhow::{Context, Result, ensure};
use plain_libs::rusqlite::{Connection, params};
use plain_libs::serde_json::{self, Value, json};
use plain_libs::utils::dbtime::normalize;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "SCREAMING_SNAKE_CASE")]
enum LegacyContent {
    #[serde(alias = "text")]
    Text {
        text: String,
        #[serde(default, rename = "linkPreviews")]
        link_previews: Vec<LegacyPreview>,
    },
    #[serde(alias = "images")]
    Images { items: Vec<LegacyAttachment> },
    #[serde(alias = "files")]
    Files { items: Vec<LegacyAttachment> },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyPreview {
    url: String,
    title: Option<String>,
    description: Option<String>,
    image_url: Option<String>,
    image_local_path: Option<String>,
    #[serde(default)]
    image_width: i32,
    #[serde(default)]
    image_height: i32,
    site_name: Option<String>,
    domain: Option<String>,
    created_at: Option<Instant>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAttachment {
    id: Option<String>,
    uri: String,
    size: i64,
    #[serde(default)]
    duration: i64,
    #[serde(default)]
    width: i32,
    #[serde(default)]
    height: i32,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    file_name: String,
}

pub(super) fn import(
    source: &Connection,
    target: &Connection,
    assets: &mut Assets<'_>,
    self_id: &str,
) -> Result<()> {
    import_peers(source, target)?;
    import_channels(source, target, self_id)?;
    for_each(source, "SELECT * FROM chats", |row| {
        let id = short_id::convert(&row.get::<_, String>("id")?)?;
        let created = normalize(&row.get::<_, String>("created_at")?)?;
        let updated = normalize(&row.get::<_, String>("updated_at")?)?;
        let from = short_id::reference(&row.get::<_, String>("from_id")?)?;
        let to = short_id::reference(&row.get::<_, String>("to_id")?)?;
        let channel = short_id::optional(&row.get::<_, String>("channel_id")?)?;
        let status = enum_text(row.get("status")?, "PENDING");
        let status_data = convert_deliveries(&row.get::<_, String>("status_data")?)?;
        let content = convert_content(
            &row.get::<_, String>("content")?,
            &created,
            &updated,
            assets,
        )?;
        target.execute(
            "INSERT INTO chats(id,from_id,to_id,channel_id,status,status_data,content,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO NOTHING",
            params![id, from, to, channel, status, status_data, content, created, updated],
        )?;
        Ok(())
    })
}

fn import_peers(source: &Connection, target: &Connection) -> Result<()> {
    for_each(source, "SELECT * FROM peers", |row| {
        let id = short_id::convert(&row.get::<_, String>("id")?)?;
        let name: String = row.get("name")?;
        let ip: String = row.get("ip")?;
        let key: String = row.get("key")?;
        let public_key: String = row.get("public_key")?;
        let port: i64 = row.get("port")?;
        let status = enum_text(row.get("status")?, "UNPAIRED");
        let device_type = enum_text(row.get("device_type")?, "OTHER");
        let (created, updated) = super::rows::dates(row)?;
        target.execute(
            "INSERT INTO peers(id,name,ip,key,public_key,port,status,device_type,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,NULLIF(?6,0),?7,?8,?9,?10) ON CONFLICT(id) DO NOTHING",
            params![id, name, ip, key_blob::optional(&key)?, key_blob::optional(&public_key)?, port, status, device_type, created, updated],
        )?;
        Ok(())
    })
}

fn import_channels(source: &Connection, target: &Connection, self_id: &str) -> Result<()> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Member {
        Id(String),
        Entry { id: String, status: Option<String> },
    }

    for_each(source, "SELECT * FROM chat_channels", |row| {
        let id = short_id::convert(&row.get::<_, String>("id")?)?;
        let name: String = row.get("name")?;
        let key: String = row.get("key")?;
        let owner: String = row.get("owner")?;
        let owner = (!owner.is_empty())
            .then(|| channel_peer(&owner, self_id))
            .transpose()?;
        let members: Vec<Member> = serde_json::from_str(&row.get::<_, String>("members")?)?;
        let members = members
            .into_iter()
            .map(|member| {
                let (id, status) = match member {
                    Member::Id(id) => (id, MemberStatus::Joined),
                    Member::Entry { id, status } => {
                        let status = enum_text(status.unwrap_or_default(), "JOINED")
                            .parse()
                            .map_err(plain_libs::anyhow::Error::msg)?;
                        (id, status)
                    }
                };
                Ok(ChannelMember {
                    peer_id: channel_peer(&id, self_id)?,
                    status,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let version: i64 = row.get("version")?;
        let status = enum_text(row.get("status")?, "JOINED");
        let (created, updated) = super::rows::dates(row)?;
        target.execute(
            "INSERT INTO chat_channels(id,name,key,owner_id,members,version,status,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO NOTHING",
            params![id, name, key_blob::optional(&key)?, owner, serde_json::to_string(&members)?, version, status, created, updated],
        )?;
        Ok(())
    })
}

fn channel_peer(value: &str, self_id: &str) -> Result<String> {
    if value == "me" {
        ensure!(!self_id.is_empty(), "Local channel identity is unavailable");
        return Ok(self_id.to_owned());
    }
    short_id::convert(value)
}

fn enum_text(value: String, default: &str) -> String {
    if value.is_empty() {
        default.to_owned()
    } else {
        value.to_ascii_uppercase()
    }
}

fn convert_deliveries(data: &str) -> Result<Option<String>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Delivery {
        peer_id: String,
        peer_name: String,
        error: Option<String>,
    }
    #[derive(Deserialize)]
    struct StatusData {
        #[serde(default)]
        results: Vec<Delivery>,
    }

    if data.is_empty() {
        return Ok(None);
    }
    let data: StatusData = serde_json::from_str(data)?;
    let results = data.results.into_iter().map(|entry| -> Result<Value> {
        Ok(json!({"peerId": short_id::reference(&entry.peer_id)?, "peerName": entry.peer_name, "error": entry.error}))
    }).collect::<Result<Vec<_>>>()?;
    Ok(Some(json!({"results": results}).to_string()))
}

fn convert_content(
    data: &str,
    created: &str,
    updated: &str,
    assets: &mut Assets<'_>,
) -> Result<String> {
    let content: LegacyContent =
        serde_json::from_str(data).context("Invalid legacy chat message")?;
    let mut text = None;
    let mut items = Vec::new();
    let mut previews = Vec::new();
    let mut removed_remote = false;
    match content {
        LegacyContent::Text {
            text: message,
            link_previews,
        } => {
            text = Some(message);
            for preview in link_previews {
                let image = match preview
                    .image_local_path
                    .as_deref()
                    .filter(|value| !value.is_empty())
                {
                    Some(path) => Some(assets.chat_uri(path, created, updated)?),
                    None => preview.image_url,
                };
                let created_at = preview
                    .created_at
                    .map_or_else(|| json!(created), |instant| json!(instant));
                previews.push(json!({
                    "url": preview.url, "title": preview.title, "description": preview.description,
                    "image": image, "imageWidth": preview.image_width, "imageHeight": preview.image_height,
                    "siteName": preview.site_name, "domain": preview.domain, "createdAt": created_at,
                }));
            }
        }
        attachments => {
            let (kind, files) = match attachments {
                LegacyContent::Files { items } => ("FILE", items),
                LegacyContent::Images { items } => ("IMAGE", items),
                LegacyContent::Text { .. } => unreachable!(),
            };
            for file in files {
                if file.uri.starts_with("fsid:") {
                    removed_remote = true;
                    continue;
                }
                let uri = assets.chat_uri(&file.uri, created, updated)?;
                let id = match file.id.as_deref() {
                    Some(id) => short_id::convert(id)?,
                    None => crate::features::record_id::generate(),
                };
                items.push(json!({"type": kind, "attachment": {
                    "id": id, "uri": uri,
                    "size": file.size, "durationMs": file.duration, "width": file.width, "height": file.height,
                    "summary": file.summary, "fileName": file.file_name,
                }}));
            }
        }
    }
    let attachment_only_without_files = removed_remote && items.is_empty();
    let content = json!({"type": "MESSAGE", "value": {"text": text, "format": "PLAIN", "items": items, "linkPreviews": previews}}).to_string();
    if !attachment_only_without_files {
        crate::infrastructure::db::chat_store::messages::validate_content(&content)
            .context("Invalid converted chat message")?;
    }
    Ok(content)
}
