use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::Db;
use crate::infrastructure::transport::auth as peer_auth;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Mutex};

struct State {
    revision: u64,
    connections: HashMap<String, DPeer>,
    hints: HashMap<String, DPeer>,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Proof {
    client_id: String,
    challenge: String,
}

pub struct Connections {
    id: String,
    state: Mutex<State>,
}
impl Default for Connections {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            state: Mutex::new(State {
                revision: 0,
                connections: HashMap::new(),
                hints: HashMap::new(),
            }),
        }
    }
}
impl Connections {
    pub fn open(&self, db: &Db, id: &str, challenge: &str, body: &[u8]) -> Result<(String, bool)> {
        let authenticated = peer_auth::authenticate_status(db, id, body)
            .map_err(|e| anyhow::anyhow!(e.reason()))?;
        let proof: Proof = serde_json::from_str(&authenticated.graphql_json)?;
        if proof.client_id != id || proof.challenge != challenge {
            bail!("Status proof does not match connection");
        }
        self.admit(db, authenticated.peer)
    }
    pub fn admit(&self, db: &Db, peer: DPeer) -> Result<(String, bool)> {
        let id = &peer.id;
        let mut state = self.state.lock().unwrap();
        if state.connections.len() >= 128 {
            bail!("Too many status connections");
        }
        if !Self::current(db, &peer) {
            bail!("Peer identity changed");
        }
        let changed = !state
            .connections
            .values()
            .any(|p| p.id == *id && p.key == peer.key && p.public_key == peer.public_key);
        let lease = uuid::Uuid::new_v4().to_string();
        state.connections.insert(lease.clone(), peer);
        state.revision = state.revision.wrapping_add(1);
        Ok((lease, changed))
    }
    pub fn valid(&self, db: &Db, lease: &str) -> bool {
        let state = self.state.lock().unwrap();
        state
            .connections
            .get(lease)
            .is_some_and(|expected| Self::current(db, expected))
    }
    fn current(db: &Db, expected: &DPeer) -> bool {
        crate::infrastructure::db::chat_store::peers::get(db, &expected.id)
            .ok()
            .flatten()
            .is_some_and(|peer| {
                peer.is_paired()
                    && peer.key == expected.key
                    && peer.public_key == expected.public_key
            })
    }
    pub fn close(&self, db: &Db, lease: &str) -> Option<(String, bool)> {
        let mut state = self.state.lock().unwrap();
        let peer = state.connections.remove(lease)?;
        state.revision = state.revision.wrapping_add(1);
        if state
            .hints
            .get(&peer.id)
            .is_some_and(|p| p.key == peer.key && p.public_key == peer.public_key)
        {
            state.hints.remove(&peer.id);
        }
        let online = state
            .connections
            .values()
            .any(|p| p.id == peer.id && Self::current(db, p));
        Some((peer.id, online))
    }
    fn current_hint(db: &Db, expected: &DPeer) -> bool {
        crate::infrastructure::db::chat_store::peers::get(db, &expected.id)
            .ok()
            .flatten()
            .is_some_and(|p| {
                p.key == expected.key
                    && p.public_key == expected.public_key
                    && p.status == expected.status
            })
    }
    pub fn hint(&self, db: &Db, id: &str) -> Result<bool> {
        let Some(peer) = crate::infrastructure::db::chat_store::peers::get(db, id)? else {
            return Ok(false);
        };
        let mut state = self.state.lock().unwrap();
        if state.hints.len() >= 512 && !state.hints.contains_key(id) {
            state.hints.retain(|_, p| Self::current_hint(db, p));
            if state.hints.len() >= 512 {
                return Ok(false);
            }
        }
        let changed = !state
            .connections
            .values()
            .any(|p| p.id == id && Self::current(db, p))
            && !state
                .hints
                .get(id)
                .is_some_and(|p| Self::current_hint(db, p));
        state.hints.insert(id.to_owned(), peer);
        state.revision = state.revision.wrapping_add(1);
        Ok(changed)
    }
    pub fn clear_hints(&self) {
        let mut state = self.state.lock().unwrap();
        state.hints.clear();
        state.revision = state.revision.wrapping_add(1);
    }
    pub fn active_peers(&self, db: &Db) -> std::collections::HashSet<String> {
        let state = self.state.lock().unwrap();
        state
            .connections
            .values()
            .filter(|p| Self::current(db, p))
            .map(|p| p.id.clone())
            .collect()
    }
    pub fn snapshot_peers(&self, peers: &[DPeer]) -> Value {
        let peers = peers
            .iter()
            .map(|p| (p.id.as_str(), p))
            .collect::<HashMap<_, _>>();
        let state = self.state.lock().unwrap();
        let current = |expected: &DPeer| {
            peers
                .get(expected.id.as_str())
                .filter(|p| p.key == expected.key && p.public_key == expected.public_key)
        };
        let mut online = state
            .connections
            .values()
            .filter(|p| current(p).is_some_and(|p| p.is_paired()))
            .map(|p| p.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        online.extend(
            state
                .hints
                .values()
                .filter(|p| current(p).is_some_and(|row| row.status == p.status))
                .map(|p| p.id.clone()),
        );
        json!({"runtimeId":self.id,"revision":state.revision,"online":online})
    }
    pub fn snapshot(&self, db: &Db) -> Value {
        let state = self.state.lock().unwrap();
        let mut online: std::collections::BTreeSet<_> = state
            .connections
            .values()
            .filter(|p| Self::current(db, p))
            .map(|p| p.id.clone())
            .collect();
        online.extend(
            state
                .hints
                .values()
                .filter(|p| Self::current_hint(db, p))
                .map(|p| p.id.clone()),
        );
        json!({"runtimeId":self.id,"revision":state.revision,"online":online})
    }
}

pub fn handshake(key: &[u8], keypair: &[u8], client_id: &str, challenge: &str) -> Result<Vec<u8>> {
    if client_id.is_empty() {
        bail!("Missing client identity");
    }
    uuid::Uuid::parse_str(challenge)?;
    let content = serde_json::to_string(&Proof {
        client_id: client_id.to_owned(),
        challenge: challenge.to_owned(),
    })?;
    let timestamp = chrono::Utc::now().timestamp_millis();
    let signature = plain_libs::crypto::ed25519_sign(
        keypair,
        peer_auth::status_signature_payload(timestamp, &content).as_bytes(),
    );
    if signature.is_empty() {
        bail!("Invalid signing identity");
    }
    plain_libs::crypto::xchacha_encrypt_raw(
        key,
        format!("{signature}|{timestamp}|{content}").as_bytes(),
    )
    .ok_or_else(|| anyhow::anyhow!("Invalid peer key"))
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/peers/status.rs"]
mod tests;
