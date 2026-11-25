use super::graphql_request::{RequestId, SignedEnvelope};
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::PEER_COLS;
use crate::infrastructure::db::row_to_peer;
use plain_libs::crypto::ed25519_verify;
use plain_libs::rusqlite::OptionalExtension;
use plain_libs::utils::base64::base64_decode;
use std::time::{SystemTime, UNIX_EPOCH};
pub(crate) const GRAPHQL_WINDOW_MS: u64 = 60_000;

pub struct AuthenticatedPeer {
    pub peer: DPeer,
    pub key: Vec<u8>,
    pub graphql_json: String,
    pub message_id: Option<String>,
}
pub struct Envelope {
    pub request_id: RequestId,
    message_id: Option<String>,
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
fn verify(
    public_key: &str,
    channel_id: Option<&str>,
    recipient_id: &str,
    plaintext: &[u8],
) -> Result<Envelope, AuthError> {
    let (signed, id) = parse(plaintext)?;
    if signed.signature.is_empty()
        || signed.message_id.as_ref().is_some_and(|id| id.is_empty())
    {
        return Err(AuthError::MissingFields);
    }
    if !ed25519_verify(public_key, &signed.signing_bytes(channel_id, recipient_id), &signed.signature) {
        return Err(AuthError::BadSignature);
    }
    Ok(Envelope {
        content: signed.graphql_json(),
        request_id: id,
        message_id: signed.message_id,
    })
}

fn parse(plaintext: &[u8]) -> Result<(SignedEnvelope, RequestId), AuthError> {
    let plaintext = std::str::from_utf8(plaintext).map_err(|_| AuthError::NotUtf8)?;
    let signed: SignedEnvelope =
        plain_libs::serde_json::from_str(plaintext).map_err(|_| AuthError::MissingFields)?;
    let id = RequestId::parse(&signed.request_id).ok_or(AuthError::MissingFields)?;
    let now = plain_libs::chrono::Utc::now().timestamp_millis();
    if !id.is_fresh(now, GRAPHQL_WINDOW_MS) {
        return Err(AuthError::TimestampExpired);
    }
    Ok((signed, id))
}

#[cfg(feature = "prefs")]
pub fn authenticate(
    db: &Db,
    prefs: &crate::features::prefs::Prefs,
    client_id: &str,
    channel_id: &str,
    body: &[u8],
) -> Result<AuthenticatedPeer, AuthError> {
    let recipient_id = prefs
        .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)
        .map_err(|_| AuthError::Storage)?
        .filter(|id| !id.is_empty())
        .ok_or(AuthError::Storage)?;
    authenticate_for(db, client_id, &recipient_id, channel_id, body, || {
        crate::features::prefs::security::encryption_private_key(prefs)
            .map_err(|_| AuthError::Storage)
    })
}

fn authenticate_for(
    db: &Db,
    client_id: &str,
    recipient_id: &str,
    channel_id: &str,
    body: &[u8],
    channel_key: impl FnOnce() -> Result<[u8; 32], AuthError>,
) -> Result<AuthenticatedPeer, AuthError> {
    let peer = peer(db, client_id)?;
    let channel = (!channel_id.is_empty()).then_some(channel_id);
    let (plaintext, key) = if let Some(channel) = channel {
        let (plaintext, key) = super::channel_request::open(&channel_key()?, channel, body)
            .ok_or(AuthError::DecryptFailed)?;
        (plaintext, key.to_vec())
    } else {
        if !peer.is_paired() {
            return Err(AuthError::NotPaired);
        }
        let key = base64_decode(&peer.key);
        let plaintext = super::http_request::decrypt_raw(&key, "POST", "/peer_graphql", body)
            .ok_or(AuthError::DecryptFailed)?;
        (plaintext, key)
    };
    let envelope = verify(&peer.public_key, channel, recipient_id, &plaintext)?;
    admit(db, peer, channel_id, key, envelope)
}

fn peer(db: &Db, client_id: &str) -> Result<DPeer, AuthError> {
    db.with_conn(|c| {
        c
            .query_row(
                // A signature identifies the sender only if its key belongs to one peer.
                &format!(
                    "SELECT {PEER_COLS} FROM peers WHERE id=?1 AND NOT EXISTS (
                        SELECT 1 FROM peers AS other
                        WHERE other.id<>peers.id AND other.signature_public_key=peers.signature_public_key
                    )"
                ),
                [client_id],
                row_to_peer,
            )
            .optional()
            .map_err(|_| AuthError::Storage)?
            .ok_or(AuthError::UnknownPeer)
    })
}

fn admit(
    db: &Db,
    peer: DPeer,
    channel_id: &str,
    key: Vec<u8>,
    envelope: Envelope,
) -> Result<AuthenticatedPeer, AuthError> {
    if envelope.message_id.is_none() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let admitted = envelope
            .request_id
            .admit_persistent(
                db,
                &format!("peer_graphql:{channel_id}"),
                &peer.id,
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
        graphql_json: envelope.content,
        message_id: envelope.message_id,
    })
}
#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/transport/auth.rs"]
mod tests;
