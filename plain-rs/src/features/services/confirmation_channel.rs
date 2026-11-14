use plain_libs::aad::{AadPurpose, LoginPurpose};
use crate::infrastructure::signature::SignaturePurpose;
use plain_libs::anyhow::{Result, ensure};
use hmac::{Hmac, KeyInit, Mac};
use plain_libs::crypto::{
    EcdhSession, ed25519_public_from_keypair, ed25519_sign, ed25519_verify,
    xchacha_decrypt_raw_with_aad, xchacha_encrypt_raw_with_aad,
};
use plain_libs::utils::base64::{base64_decode, base64_encode};
use serde::{Deserialize, Serialize};
use plain_libs::serde_json::json;
use sha2::{Digest, Sha256};

const LIFETIME_MS: u64 = 300_000;

pub(super) fn verification_code(hash: &[u8; 32]) -> String {
    let digits = u64::from_be_bytes(hash[..8].try_into().unwrap()) % 100_000_000;
    format!("{digits:08}")
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientHello {
    pub public_key: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerHello {
    pub device_id: String,
    pub server_public_key: String,
    pub expires_at: u64,
    pub signature: String,
}
fn request_value(request: &ClientHello) -> plain_libs::serde_json::Value {
    json!({"publicKey":request.public_key})
}
// Struct serialization fixes insertion order independently of serde_json map features.
fn unsigned_response(response: &ServerHello) -> String {
    format!(
        "{{\"deviceId\":{},\"serverPublicKey\":{},\"expiresAt\":{}}}",
        json!(response.device_id),
        json!(response.server_public_key),
        response.expires_at
    )
}
fn response_bytes(response: &ServerHello) -> String {
    format!(
        "{},\"signature\":{}}}",
        unsigned_response(response).trim_end_matches('}'),
        json!(response.signature)
    )
}
fn signed_bytes(cid: &str, public: &str, request: &ClientHello, response: &ServerHello) -> Vec<u8> {
    format!(
        "[{},{},{},{},{},{}]",
        json!(SignaturePurpose::AuthServerHello),
        json!(response.device_id),
        json!(public),
        json!(cid),
        request_value(request),
        unsigned_response(response)
    )
    .into_bytes()
}
fn transcript(cid: &str, request: &ClientHello, response: &ServerHello) -> Vec<u8> {
    format!(
        "[{},{},{},{},{}]",
        json!(LoginPurpose::NoPassword.as_str()),
        json!(cid),
        json!(response.device_id),
        request_value(request),
        response_bytes(response)
    )
    .into_bytes()
}
pub struct Client {
    cid: String,
    key: EcdhSession,
    request: ClientHello,
}
impl Client {
    pub fn start(cid: &str) -> Result<(Self, ClientHello)> {
        identity(cid)?;
        let key = EcdhSession::generate();
        let request = ClientHello {
            public_key: base64_encode(&key.public_key_bytes),
        };
        Ok((
            Self {
                cid: cid.into(),
                key,
                request: request.clone(),
            },
            request,
        ))
    }
    pub fn accept(
        self,
        response: &ServerHello,
        public: &str,
        device: &str,
        now: u64,
    ) -> Result<Channel> {
        identity(&response.device_id)?;
        ensure!(
            device.is_empty() || response.device_id == device,
            "peer_identity_changed"
        );
        ensure!(response.expires_at > now, "login_expired");
        canonical(public, 32)?;
        ensure!(
            ed25519_verify(
                public,
                &signed_bytes(&self.cid, public, &self.request, response),
                &response.signature
            ),
            "signature_verification_failed"
        );
        let shared = self
            .key
            .compute_shared_key(&canonical(&response.server_public_key, 65)?)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_auth_response"))?;
        Ok(Channel::new(
            &shared,
            &transcript(&self.cid, &self.request, response),
            response.expires_at.min(now.saturating_add(LIFETIME_MS)),
        ))
    }
}
pub struct ServerHandshake;
impl ServerHandshake {
    pub fn start(
        cid: &str,
        device: &str,
        request: &ClientHello,
        signing: &[u8],
        now: u64,
    ) -> Result<(Channel, ServerHello)> {
        identity(cid)?;
        identity(device)?;
        let client_public = canonical(&request.public_key, 65)?;
        let key = EcdhSession::generate();
        let public = base64_encode(
            &ed25519_public_from_keypair(signing)
                .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_identity"))?,
        );
        let mut response = ServerHello {
            device_id: device.into(),
            server_public_key: base64_encode(&key.public_key_bytes),
            expires_at: now
                .checked_add(LIFETIME_MS)
                .ok_or_else(|| plain_libs::anyhow::anyhow!("login_expired"))?,
            signature: String::new(),
        };
        response.signature = ed25519_sign(signing, &signed_bytes(cid, &public, request, &response));
        ensure!(
            !response.signature.is_empty(),
            "signature_verification_failed"
        );
        let shared = key
            .compute_shared_key(&client_public)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_login_request"))?;
        Ok((
            Channel::new(
                &shared,
                &transcript(cid, request, &response),
                response.expires_at,
            ),
            response,
        ))
    }
}
pub struct Channel {
    key: [u8; 32],
    hash: [u8; 32],
    pub context: String,
    pub verification_code: String,
    expires_at: u64,
}
impl Channel {
    fn new(shared: &[u8; 32], transcript: &[u8], expires_at: u64) -> Self {
        let hash: [u8; 32] = Sha256::digest(transcript).into();
        let mut extract = Hmac::<sha2_mac::Sha256>::new_from_slice(&hash).unwrap();
        extract.update(shared);
        let prk = extract.finalize().into_bytes();
        let mut expand = Hmac::<sha2_mac::Sha256>::new_from_slice(&prk).unwrap();
        expand.update(b"api");
        expand.update(&[1]);
        let key = expand.finalize().into_bytes().into();
        let context = plain_libs::utils::hex::bytes_to_hex(&hash);
        Self {
            key,
            hash,
            verification_code: verification_code(&hash),
            context,
            expires_at,
        }
    }
    pub fn api_key(&self) -> [u8; 32] {
        self.key
    }
    pub fn fresh(&self, now: u64) -> Result<()> {
        ensure!(now < self.expires_at, "login_expired");
        Ok(())
    }
    fn aad(&self, response: bool) -> Vec<u8> {
        let mut a = if response {
            AadPurpose::AuthResponse.as_str().as_bytes().to_vec()
        } else {
            AadPurpose::AuthRequest.as_str().as_bytes().to_vec()
        };
        a.extend(self.hash);
        a
    }
    pub fn encrypt_request(&self, b: &[u8], now: u64) -> Result<Vec<u8>> {
        self.fresh(now)?;
        xchacha_encrypt_raw_with_aad(&self.key, b, &self.aad(false))
            .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_login_request"))
    }
    pub fn decrypt_request(&self, b: &[u8], now: u64) -> Result<Vec<u8>> {
        self.fresh(now)?;
        xchacha_decrypt_raw_with_aad(&self.key, b, &self.aad(false))
            .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_login_request"))
    }
    pub fn encrypt_response(&self, b: &[u8], now: u64) -> Result<Vec<u8>> {
        self.fresh(now)?;
        xchacha_encrypt_raw_with_aad(&self.key, b, &self.aad(true))
            .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_auth_response"))
    }
    pub fn decrypt_response(&self, b: &[u8], now: u64) -> Result<Vec<u8>> {
        self.fresh(now)?;
        xchacha_decrypt_raw_with_aad(&self.key, b, &self.aad(true))
            .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_auth_response"))
    }
}
fn identity(s: &str) -> Result<()> {
    ensure!(!s.is_empty() && s.len() <= 256, "invalid_login_identity");
    Ok(())
}
fn canonical(s: &str, len: usize) -> Result<Vec<u8>> {
    let b = base64_decode(s);
    ensure!(
        b.len() == len && base64_encode(&b) == s,
        "invalid_login_key"
    );
    Ok(b)
}
#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/services/confirmation_channel.rs"]
mod tests;
