use crate::features::chat::channels::messages::ChannelMember;
use crate::features::chat::channels::messages::MemberSnapshot;
use crate::features::chat::enums::PeerStatus;
use crate::infrastructure::db::CHANNEL_COLS;
use crate::infrastructure::db::DChannel;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::PEER_COLS;
use crate::infrastructure::db::key_blob;
use crate::infrastructure::db::now_iso;
use crate::infrastructure::db::row_to_channel;
use crate::infrastructure::db::row_to_peer;
use plain_libs::anyhow::{Result, bail};
use plain_libs::rusqlite::{Connection, OptionalExtension, params};

pub(crate) fn channel(c: &Connection, id: &str) -> Result<Option<DChannel>> {
    Ok(c.query_row(
        &format!("SELECT {CHANNEL_COLS} FROM chat_channels WHERE id=?1"),
        [id],
        row_to_channel,
    )
    .optional()?)
}
pub(crate) fn peer(c: &Connection, id: &str) -> Result<Option<DPeer>> {
    Ok(c.query_row(
        &format!("SELECT {PEER_COLS} FROM peers WHERE id=?1"),
        [id],
        row_to_peer,
    )
    .optional()?)
}
pub(crate) fn members(raw: &str) -> Result<Vec<ChannelMember>> {
    Ok(plain_libs::serde_json::from_str(raw)?)
}
pub(crate) fn public_key(key: &str) -> Result<[u8; 32]> {
    Ok(key_blob::encode(key)?)
}
pub(crate) fn ensure_peers(c: &Connection, items: &[MemberSnapshot], actor: &str) -> Result<()> {
    for item in items {
        let signature = public_key(&item.signature_public_key)?;
        let encryption = crate::features::peers::pairing::peer_store::encryption_key(&item.encryption_public_key)?;
        if item.peer_id == actor {
            continue;
        }
        if let Some(existing) = peer(c, &item.peer_id)? {
            crate::features::peers::pairing::peer_store::verify_identity(&existing, &signature, &encryption)
                .map_err(|_| plain_libs::anyhow::anyhow!("Channel member identity changed"))?;
            c.execute(
                "UPDATE peers SET signature_public_key=?2,encryption_public_key=?3 WHERE id=?1 AND (signature_public_key IS NULL OR encryption_public_key IS NULL)",
                params![item.peer_id, signature, encryption],
            )?;
            continue;
        }
        let now = now_iso();
        let device_type = item
            .device_type
            .unwrap_or(crate::features::chat::enums::DeviceType::Unknown);
        if c.execute(
            "INSERT INTO peers(id,name,ip,signature_public_key,encryption_public_key,status,port,device_type,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",
            params![item.peer_id,item.name.as_deref().unwrap_or_default(),item.ip.as_deref().unwrap_or_default(),signature,encryption,PeerStatus::Channel,item.port,device_type,now],
        )? != 1 {
            bail!("Member peer insert failed");
        }
    }
    Ok(())
}
pub(crate) fn save(c: &Connection, channel: &DChannel, new: bool) -> Result<()> {
    let sql = if new {
        "INSERT INTO chat_channels(id,name,owner_id,members,version,status,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)"
    } else {
        "UPDATE chat_channels SET name=?2,owner_id=?3,members=?4,version=?5,status=?6,created_at=?7,updated_at=?8 WHERE id=?1"
    };
    if c.execute(
        sql,
        params![
            channel.id,
            channel.name,
            channel
                .owner_id
                .as_deref()
                .filter(|value| !value.is_empty()),
            channel.members,
            channel.version,
            channel.status,
            plain_libs::utils::dbtime::normalize(&channel.created_at)?,
            plain_libs::utils::dbtime::normalize(&channel.updated_at)?
        ],
    )? != 1
    {
        bail!("Channel receive write failed");
    }
    Ok(())
}
