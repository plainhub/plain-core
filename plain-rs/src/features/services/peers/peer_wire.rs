use crate::features::prefs::Prefs;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::chat_store::channels;
use crate::infrastructure::db::chat_store::peers;
use crate::infrastructure::transport::contract::START_AWARE;
use crate::infrastructure::transport::contract::chat_item_request;
use crate::infrastructure::transport::contract::signed_request;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::json;
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Operation {
    Chat { content: String, channel_id: String },
    StartAware,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prepared {
    #[serde(serialize_with = "crate::features::services::peers::peer_address::serialize_view")]
    pub peer: DPeer,
    pub body: String,
    pub key: String,
    pub channel_id: String,
}
pub fn prepare(db: &Db, prefs: &Prefs, id: &str, operation: Operation) -> Result<Prepared> {
    let peer = peers::get(db, id)?.ok_or_else(|| anyhow::anyhow!("Unknown peer"))?;
    let (body, channel_id, key) = match operation {
        Operation::Chat {
            content,
            channel_id,
        } => {
            let key = if channel_id.is_empty() {
                peer.key.clone()
            } else {
                channels::get(db, &channel_id)?
                    .ok_or_else(|| anyhow::anyhow!("Unknown channel"))?
                    .key
            };
            (
                chat_item_request(&signing_keypair(prefs)?, &content, &channel_id)
                    .map_err(anyhow::Error::msg)?,
                channel_id,
                key,
            )
        }
        Operation::StartAware => (
            signed_request(&signing_keypair(prefs)?, START_AWARE, json!({}), "")
                .map_err(anyhow::Error::msg)?,
            String::new(),
            peer.key.clone(),
        ),
    };
    if plain_libs::utils::base64::base64_decode(&key).len() != 32 {
        bail!("Invalid peer transport key");
    }
    Ok(Prepared {
        peer,
        body,
        key,
        channel_id,
    })
}
pub fn signing_keypair(prefs: &Prefs) -> Result<Vec<u8>> {
    crate::features::prefs::security::signing_keypair(prefs)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/peers/peer_wire.rs"]
mod tests;
