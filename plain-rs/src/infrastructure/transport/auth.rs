use crate::infrastructure::db::CHANNEL_COLS;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::PEER_COLS;
use crate::infrastructure::db::row_to_channel;
use crate::infrastructure::db::row_to_peer;
use plain_libs::crypto::ed25519_verify;
use plain_libs::crypto::xchacha_decrypt_raw;
use plain_libs::utils::base64::base64_decode;
use rusqlite::OptionalExtension;
use std::time::{SystemTime, UNIX_EPOCH};
// Existing chat receipt lifetime; independent of HTTP request freshness.
pub(crate) const TIMESTAMP_WINDOW_MS: u64 = 5 * 60 * 1000;
const GRAPHQL_WINDOW_MS: u64 = 60_000;
const STATUS_WINDOW_MS: u64 = 300_000;

pub struct AuthenticatedPeer {
    pub peer: DPeer,
    pub key: Vec<u8>,
    pub signature_b64: String,
    pub timestamp: i64,
    pub graphql_json: String,
}
pub struct Envelope {
    pub nonce: String,
    pub signature_b64: String,
    pub timestamp: i64,
    pub content: String,
}
#[derive(Debug)]
pub enum AuthError {
    UnknownPeer,
    NotPaired,
    DecryptFailed,
    NotUtf8,
    TimestampExpired,
    BadSignature,
    MissingFields,
    NoChannelKey,
    Storage,
    Replay,
}
impl AuthError {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::UnknownPeer => "unknown peer",
            Self::NotPaired => "not paired",
            Self::DecryptFailed => "decrypt failed",
            Self::NotUtf8 => "not utf-8",
            Self::TimestampExpired => "timestamp expired",
            Self::BadSignature => "bad signature",
            Self::MissingFields => "missing fields",
            Self::NoChannelKey => "no channel key",
            Self::Storage => "peer storage failed",
            Self::Replay => "replayed request",
        }
    }
    pub fn http_status(&self) -> u16 {
        match self {
            Self::NotPaired => 403,
            Self::TimestampExpired | Self::MissingFields | Self::Replay => 400,
            Self::Storage => 500,
            _ => 401,
        }
    }
}
#[derive(Clone, Copy)]
enum Purpose {
    Graphql,
    Status,
}
pub fn signature_payload(timestamp: i64, channel_id: &str, nonce: &str, content: &str) -> String {
    serde_json::json!(["peer", channel_id, timestamp.to_string(), nonce, content]).to_string()
}
pub(crate) fn status_signature_payload(timestamp: i64, content: &str) -> String {
    serde_json::json!(["status", "", timestamp.to_string(), content]).to_string()
}
fn decrypt(
    key: &[u8],
    public_key: &str,
    channel_id: &str,
    body: &[u8],
    purpose: Purpose,
) -> Result<Envelope, AuthError> {
    let plaintext = match purpose {
        Purpose::Graphql => super::http_request::decrypt_raw(key, "POST", "/peer_graphql", body),
        Purpose::Status => xchacha_decrypt_raw(key, body),
    }
    .ok_or(AuthError::DecryptFailed)?;
    let plaintext = std::str::from_utf8(&plaintext).map_err(|_| AuthError::NotUtf8)?;
    let mut parts = plaintext.splitn(
        if matches!(purpose, Purpose::Graphql) {
            4
        } else {
            3
        },
        '|',
    );
    let signature_b64 = parts.next().unwrap_or_default();
    let raw_timestamp = parts.next().ok_or(AuthError::MissingFields)?;
    let nonce = if matches!(purpose, Purpose::Graphql) {
        let nonce = parts.next().ok_or(AuthError::MissingFields)?;
        if nonce.len() != 36 {
            return Err(AuthError::MissingFields);
        }
        let parsed = plain_libs::utils::uuid::Uuid::parse_str(nonce)
            .map_err(|_| AuthError::MissingFields)?;
        if parsed.to_string() != nonce {
            return Err(AuthError::MissingFields);
        }
        nonce
    } else {
        ""
    };
    let content = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or(AuthError::MissingFields)?;
    if signature_b64.is_empty() {
        return Err(AuthError::MissingFields);
    }
    let timestamp = raw_timestamp
        .parse::<i64>()
        .map_err(|_| AuthError::MissingFields)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    if now.abs_diff(timestamp)
        > match purpose {
            Purpose::Graphql => GRAPHQL_WINDOW_MS,
            Purpose::Status => STATUS_WINDOW_MS,
        }
    {
        return Err(AuthError::TimestampExpired);
    }
    if !ed25519_verify(
        public_key,
        match purpose {
            Purpose::Graphql => signature_payload(timestamp, channel_id, nonce, content),
            Purpose::Status => status_signature_payload(timestamp, content),
        }
        .as_bytes(),
        signature_b64,
    ) {
        return Err(AuthError::BadSignature);
    }
    Ok(Envelope {
        nonce: nonce.into(),
        signature_b64: signature_b64.into(),
        timestamp,
        content: content.into(),
    })
}
pub fn authenticate(
    db: &Db,
    client_id: &str,
    channel_id: &str,
    body: &[u8],
) -> Result<AuthenticatedPeer, AuthError> {
    authenticate_for(db, client_id, channel_id, body, Purpose::Graphql)
}
pub(crate) fn authenticate_status(
    db: &Db,
    client_id: &str,
    body: &[u8],
) -> Result<AuthenticatedPeer, AuthError> {
    authenticate_for(db, client_id, "", body, Purpose::Status)
}
fn authenticate_for(
    db: &Db,
    client_id: &str,
    channel_id: &str,
    body: &[u8],
    purpose: Purpose,
) -> Result<AuthenticatedPeer, AuthError> {
    let (peer, key) = db.with_conn(|c| -> Result<_, AuthError> {
        let peer = c
            .query_row(
                &format!("SELECT {PEER_COLS} FROM peers WHERE id=?1"),
                [client_id],
                row_to_peer,
            )
            .optional()
            .map_err(|_| AuthError::Storage)?
            .ok_or(AuthError::UnknownPeer)?;
        let key = if channel_id.is_empty() {
            if !peer.is_paired() {
                return Err(AuthError::NotPaired);
            }
            base64_decode(&peer.key)
        } else {
            let channel = c
                .query_row(
                    &format!("SELECT {CHANNEL_COLS} FROM chat_channels WHERE id=?1"),
                    [channel_id],
                    row_to_channel,
                )
                .optional()
                .map_err(|_| AuthError::Storage)?
                .ok_or(AuthError::NoChannelKey)?;
            base64_decode(&channel.key)
        };
        Ok((peer, key))
    })?;
    let envelope = decrypt(&key, &peer.public_key, channel_id, body, purpose)?;
    if matches!(purpose, Purpose::Graphql) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let admitted = db
            .admit_request(
                &format!("peer_graphql:{channel_id}"),
                client_id,
                envelope.timestamp,
                &envelope.nonce,
                now,
                GRAPHQL_WINDOW_MS as i64,
            )
            .map_err(|_| AuthError::Storage)?;
        if !admitted {
            return Err(AuthError::Replay);
        }
    }
    Ok(AuthenticatedPeer {
        peer,
        key,
        signature_b64: envelope.signature_b64,
        timestamp: envelope.timestamp,
        graphql_json: envelope.content,
    })
}
#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/transport/auth.rs"]
mod tests;
