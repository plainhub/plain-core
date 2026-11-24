use crate::infrastructure::db::key_blob;
use plain_libs::rusqlite::params;

use crate::features::chat::enums::DeviceType;
use crate::features::chat::enums::PeerStatus;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::now_iso;

pub(crate) const PEER_COLS: &str =
    "id,name,ip,chat_token,signature_public_key,status,port,device_type,manage_token,created_at,updated_at,encryption_public_key";

pub(crate) fn row_to_peer(row: &plain_libs::rusqlite::Row<'_>) -> plain_libs::rusqlite::Result<DPeer> {
    Ok(DPeer {
        id: row.get(0)?,
        name: row.get(1)?,
        ip: row.get(2)?,
        key: key_blob::read_optional(row, 3)?,
        public_key: key_blob::read_optional(row, 4)?,
        status: row.get(5)?,
        port: row.get(6)?,
        device_type: row.get(7)?,
        token: key_blob::read_optional(row, 8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        encryption_public_key: row.get(11)?,
    })
}

impl Db {
    pub fn get_peers(&self) -> Vec<DPeer> {
        let conn = self.0.lock().unwrap();
        let mut stmt =
            match conn.prepare(&format!("SELECT {PEER_COLS} FROM peers ORDER BY name ASC")) {
                Ok(s) => s,
                Err(_) => return vec![],
            };
        stmt.query_map(params![], row_to_peer)
            .ok()
            .map(|iter| iter.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    pub fn get_peer_by_id(&self, id: &str) -> Option<DPeer> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            &format!("SELECT {PEER_COLS} FROM peers WHERE id=?"),
            params![id],
            row_to_peer,
        )
        .ok()
    }

    pub fn upsert_peer(&self, peer: &DPeer) {
        let conn = self.0.lock().unwrap();
        let result = (|| -> plain_libs::rusqlite::Result<usize> {
            conn.execute(
            "INSERT INTO peers (id,name,ip,chat_token,signature_public_key,status,port,device_type,manage_token,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, ip=excluded.ip, chat_token=excluded.chat_token,
               signature_public_key=excluded.signature_public_key, status=excluded.status,
               port=excluded.port, device_type=excluded.device_type,
               manage_token=excluded.manage_token, updated_at=excluded.updated_at",
            params![
                peer.id, peer.name, peer.ip, key_blob::optional(&peer.key)?, key_blob::optional(&peer.public_key)?,
                peer.status, peer.port.map(i64::from), peer.device_type, key_blob::optional(&peer.token)?,
                plain_libs::utils::dbtime::normalize(&peer.created_at)?, plain_libs::utils::dbtime::normalize(&peer.updated_at)?
            ],
        )
        })();
        if let Err(e) = result {
            log::error!("upsert_peer failed id={} err={e}", peer.id);
        } else {
            log::info!(
                "upsert_peer ok id={} name={} status={}",
                peer.id,
                peer.name,
                peer.status
            );
        }
    }

    pub fn delete_peer(&self, id: &str) {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute("DELETE FROM peers WHERE id=?", params![id]);
    }

    pub fn update_peer_status(&self, id: &str, status: PeerStatus) {
        let now = now_iso();
        let conn = self.0.lock().unwrap();
        let _ = conn.execute(
            "UPDATE peers SET status=?1, updated_at=?2 WHERE id=?3",
            params![status, now, id],
        );
    }

    pub fn update_peer_status_and_key(&self, id: &str, status: PeerStatus, key: &str) {
        let now = now_iso();
        let conn = self.0.lock().unwrap();
        let result = key_blob::optional(key).and_then(|key| {
            conn.execute(
                "UPDATE peers SET status=?1, chat_token=?2, updated_at=?3 WHERE id=?4",
                params![status, key, now, id],
            )
        });
        if let Err(error) = result {
            log::error!("update_peer_status_and_key failed id={id} err={error}");
        }
    }

    pub fn logout_peer(&self, id: &str) -> Result<(), String> {
        let now = now_iso();
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE peers SET manage_token=NULL, updated_at=?1 WHERE id=?2",
            params![now, id],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Peers with an active login token (the device-switcher list).
    pub fn get_login_peers(&self) -> Vec<DPeer> {
        let conn = self.0.lock().unwrap();
        let mut stmt = match conn.prepare(&format!(
            "SELECT {PEER_COLS} FROM peers WHERE manage_token IS NOT NULL ORDER BY created_at DESC"
        )) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![], row_to_peer)
            .ok()
            .map(|iter| iter.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/db/repositories/peer.rs"]
mod tests;
