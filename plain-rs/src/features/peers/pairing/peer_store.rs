use crate::features::chat::enums::DeviceType;
use crate::features::chat::enums::PeerStatus;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::PEER_COLS;
use crate::infrastructure::db::now_iso;
use crate::infrastructure::db::row_to_peer;
use plain_libs::anyhow::{Result, bail};
use plain_libs::rusqlite::{OptionalExtension, params};
#[derive(Clone, serde::Deserialize)]
pub struct Facts {
    pub id: String,
    pub name: String,
    pub ips: Vec<String>,
    pub port: u16,
    pub device_type: DeviceType,
    pub key: String,
    pub public_key: String,
}
pub fn save(db: &Db, facts: Facts) -> Result<DPeer> {
    save_confirmed(db, facts, None, false)
}
pub fn save_confirmed(
    db: &Db,
    facts: Facts,
    previous_identity: Option<&str>,
    allow_change: bool,
) -> Result<DPeer> {
    if facts.id.is_empty()
        || plain_libs::utils::base64::base64_decode(&facts.key).len() != 32
        || plain_libs::utils::base64::base64_decode(&facts.public_key).len() != 32
    {
        bail!("Invalid paired peer credentials");
    }
    db.with_conn(|c| -> Result<DPeer> {
        let tx = c.unchecked_transaction()?;
        let existing = tx.query_row(
            &format!("SELECT {PEER_COLS} FROM peers WHERE id=?1"),
            [&facts.id], row_to_peer,
        ).optional()?;
        if allow_change && existing.as_ref().map(|peer| peer.public_key.as_str()) != previous_identity {
            bail!("Peer identity changed while confirming");
        }
        if !allow_change && existing.as_ref().is_some_and(|peer| !peer.public_key.is_empty() && peer.public_key != facts.public_key) {
            bail!("Peer identity changed");
        }
        let new = existing.is_none();
        let mut peer = existing.unwrap_or_else(|| DPeer::new(&facts.id, &facts.name, "", Some(facts.port), facts.device_type));
        peer.name = facts.name;
        peer.ip = crate::features::peers::pairing::prefer_sender_ip(&facts.ips, "");
        peer.port = Some(facts.port);
        peer.device_type = facts.device_type;
        peer.key = facts.key;
        peer.public_key = facts.public_key;
        peer.status = PeerStatus::Paired;
        peer.updated_at = now_iso();
        let changed = if new {
            tx.execute(
                "INSERT INTO peers(id,name,ip,key,public_key,status,port,device_type,token,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![peer.id,peer.name,peer.ip,peer.key,peer.public_key,peer.status,peer.port,peer.device_type,peer.token,peer.created_at,peer.updated_at],
            )?
        } else {
            tx.execute(
                "UPDATE peers SET name=?2,ip=?3,key=?4,public_key=?5,status=?6,port=?7,device_type=?8,updated_at=?9 WHERE id=?1",
                params![peer.id,peer.name,peer.ip,peer.key,peer.public_key,peer.status,peer.port,peer.device_type,peer.updated_at],
            )?
        };
        if changed != 1 { bail!("Paired peer write failed"); }
        tx.commit()?;
        Ok(peer)
    })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/peers/pairing/peer_store.rs"]
mod tests;
