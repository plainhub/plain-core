use crate::features::chat::enums::DeviceType;
use crate::features::chat::enums::PeerStatus;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::PEER_COLS;
use crate::infrastructure::db::key_blob;
use crate::infrastructure::db::now_iso;
use crate::infrastructure::db::row_to_peer;
use plain_libs::anyhow::{Result, bail};
use plain_libs::rusqlite::{OptionalExtension, params};
#[derive(Clone, serde::Deserialize)]
pub struct Facts {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub device_type: DeviceType,
    pub key: String,
    pub public_key: String,
    pub encryption_public_key: String,
}
pub fn encryption_key(encoded: &str) -> Result<[u8; 32]> {
    let key = key_blob::encode(encoded)?;
    plain_libs::anyhow::ensure!(plain_libs::crypto::hpke::valid_public_key(&key), "Invalid peer encryption key");
    Ok(key)
}

impl Facts {
    pub fn verify_peer(&self, existing: &DPeer) -> Result<()> {
        verify_identity(
            existing,
            &key_blob::encode(&self.public_key)?,
            &key_blob::encode(&self.encryption_public_key)?,
        )
    }
}
pub(crate) fn verify_identity(
    existing: &DPeer,
    signature_public_key: &[u8; 32],
    encryption_public_key: &[u8; 32],
) -> Result<()> {
    if key_blob::optional(&existing.public_key)?
        .as_ref()
        .is_some_and(|key| key != signature_public_key)
        || existing
            .encryption_public_key
            .as_ref()
            .is_some_and(|key| key != encryption_public_key)
    {
        bail!("Peer identity changed");
    }
    Ok(())
}
pub fn save(db: &Db, facts: Facts) -> Result<DPeer> {
    store(db, facts, None, None)
}
pub fn save_confirmed(
    db: &Db,
    facts: Facts,
    previous_identity: Option<&str>,
    manage_token: Option<&str>,
) -> Result<DPeer> {
    store(db, facts, Some(previous_identity), manage_token)
}
fn store(
    db: &Db,
    facts: Facts,
    previous_identity: Option<Option<&str>>,
    manage_token: Option<&str>,
) -> Result<DPeer> {
    if facts.id.is_empty() {
        bail!("Invalid paired peer credentials");
    }
    let chat_token = key_blob::encode(&facts.key)?;
    let signature_public_key = key_blob::encode(&facts.public_key)?;
    let encryption_public_key = encryption_key(&facts.encryption_public_key)?;
    db.with_conn(|c| -> Result<DPeer> {
        let tx = c.unchecked_transaction()?;
        let existing = tx.query_row(
            &format!("SELECT {PEER_COLS} FROM peers WHERE id=?1"),
            [&facts.id], row_to_peer,
        ).optional()?;
        if previous_identity.is_some_and(|previous| existing.as_ref().map(|peer| peer.public_key.as_str()) != previous) {
            bail!("Peer identity changed while confirming");
        }
        if let Some(peer) = &existing
            && (previous_identity.is_none() || key_blob::optional(&peer.public_key)? == Some(signature_public_key))
        {
            verify_identity(peer, &signature_public_key, &encryption_public_key)?;
        }
        let new = existing.is_none();
        let mut peer = existing.unwrap_or_else(|| DPeer::new(&facts.id, &facts.name, "", Some(facts.port), facts.device_type));
        peer.encryption_public_key = Some(encryption_public_key);
        if !facts.name.is_empty() {
            peer.name = facts.name;
        }
        peer.ip = facts.ip;
        peer.port = Some(facts.port);
        peer.device_type = facts.device_type;
        peer.key = facts.key;
        peer.public_key = plain_libs::utils::base64::base64_encode(&signature_public_key);
        if let Some(token) = manage_token {
            peer.token = token.into();
        }
        let manage_token = key_blob::optional(&peer.token)?;
        peer.status = PeerStatus::Paired;
        peer.updated_at = now_iso();
        let changed = if new {
            tx.execute(
                "INSERT INTO peers(id,name,ip,chat_token,signature_public_key,status,port,device_type,manage_token,created_at,updated_at,encryption_public_key) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![peer.id,peer.name,peer.ip,chat_token,signature_public_key,peer.status,peer.port,peer.device_type,manage_token,peer.created_at,peer.updated_at,peer.encryption_public_key],
            )?
        } else {
            tx.execute(
                "UPDATE peers SET name=?2,ip=?3,chat_token=?4,signature_public_key=?5,status=?6,port=?7,device_type=?8,updated_at=?9,encryption_public_key=?10,manage_token=?11 WHERE id=?1",
                params![peer.id,peer.name,peer.ip,chat_token,signature_public_key,peer.status,peer.port,peer.device_type,peer.updated_at,peer.encryption_public_key,manage_token],
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
