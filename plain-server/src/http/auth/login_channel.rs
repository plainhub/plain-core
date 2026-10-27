use anyhow::{Result, ensure};
use hmac::{Hmac, KeyInit, Mac};
use plain_rs::{
    crypto::{EcdhSession, ed25519_sign, ed25519_verify, xchacha_decrypt_raw, xchacha_encrypt_raw},
    utils::base64::{base64_decode, base64_encode},
};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const DOMAIN: &str = "plain-web-login-v2";
const LIFETIME_MS: u64 = 300_000;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClientHello {
    pub commitment: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClientReveal {
    pub public_key: String,
    pub nonce: String,
}

impl ClientReveal {
    fn commitment(&self, client_id: &str) -> Result<String> {
        Ok(base64_encode(&Sha256::digest(serde_json::to_vec(
            &serde_json::json!([DOMAIN, client_id, self.public_key, self.nonce,]),
        )?)))
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ServerHello {
    pub client_id: String,
    pub device_id: String,
    pub client_commitment: String,
    pub server_public_key: String,
    pub server_nonce: String,
    pub expires_at: u64,
    pub signature: String,
}

impl ServerHello {
    fn fields(&self) -> Vec<serde_json::Value> {
        vec![
            DOMAIN.into(),
            self.client_id.clone().into(),
            self.device_id.clone().into(),
            self.client_commitment.clone().into(),
            self.server_public_key.clone().into(),
            self.server_nonce.clone().into(),
            self.expires_at.into(),
        ]
    }

    fn signed_transcript(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&self.fields())?)
    }

    fn transcript(&self, reveal: &ClientReveal) -> Result<Vec<u8>> {
        let mut fields = self.fields();
        fields.extend([
            reveal.public_key.clone().into(),
            reveal.nonce.clone().into(),
        ]);
        Ok(serde_json::to_vec(&fields)?)
    }
}

pub(crate) struct Client {
    client_id: String,
    key: EcdhSession,
    nonce: String,
}

impl Client {
    pub(crate) fn start(client_id: &str) -> Result<(Self, ClientHello)> {
        identity(client_id)?;
        let client = Self {
            client_id: client_id.into(),
            key: EcdhSession::generate(),
            nonce: nonce(),
        };
        let hello = ClientHello {
            commitment: client.reveal().commitment(client_id)?,
        };
        Ok((client, hello))
    }

    fn reveal(&self) -> ClientReveal {
        ClientReveal {
            public_key: base64_encode(&self.key.public_key_bytes),
            nonce: self.nonce.clone(),
        }
    }

    pub(crate) fn accept(
        self,
        hello: &ServerHello,
        public_key: &str,
        expected_device: &str,
        now: u64,
    ) -> Result<(Channel, ClientReveal)> {
        let reveal = self.reveal();
        ensure!(hello.client_id == self.client_id, "invalid_auth_response");
        ensure!(
            hello.client_commitment == reveal.commitment(&self.client_id)?,
            "invalid_auth_response"
        );
        identity(&hello.device_id)?;
        ensure!(
            expected_device.is_empty() || expected_device == hello.device_id,
            "peer_identity_changed"
        );
        ensure!(hello.expires_at > now, "login_expired");
        canonical(&hello.server_nonce, 32)?;
        let server_public = canonical(&hello.server_public_key, 65)?;
        canonical(public_key, 32)?;
        ensure!(
            ed25519_verify(public_key, &hello.signed_transcript()?, &hello.signature),
            "signature_verification_failed"
        );
        let shared = self
            .key
            .compute_shared_key(&server_public)
            .ok_or_else(|| anyhow::anyhow!("invalid_auth_response"))?;
        Ok((
            Channel::new(
                &shared,
                &hello.transcript(&reveal)?,
                hello.expires_at.min(now.saturating_add(LIFETIME_MS)),
            ),
            reveal,
        ))
    }
}

pub(crate) struct ServerHandshake {
    key: EcdhSession,
    hello: ServerHello,
}

impl ServerHandshake {
    pub(crate) fn start(
        client_id: &str,
        device_id: &str,
        hello: &ClientHello,
        signing_key: &[u8],
        now: u64,
    ) -> Result<(Self, ServerHello)> {
        identity(client_id)?;
        identity(device_id)?;
        canonical(&hello.commitment, 32)?;
        let key = EcdhSession::generate();
        let mut response = ServerHello {
            client_id: client_id.into(),
            device_id: device_id.into(),
            client_commitment: hello.commitment.clone(),
            server_public_key: base64_encode(&key.public_key_bytes),
            server_nonce: nonce(),
            expires_at: now
                .checked_add(LIFETIME_MS)
                .ok_or_else(|| anyhow::anyhow!("login_expired"))?,
            signature: String::new(),
        };
        response.signature = ed25519_sign(signing_key, &response.signed_transcript()?);
        ensure!(
            !response.signature.is_empty(),
            "signature_verification_failed"
        );
        Ok((
            Self {
                key,
                hello: response.clone(),
            },
            response,
        ))
    }

    pub(crate) fn finish(self, reveal: &ClientReveal, now: u64) -> Result<Channel> {
        ensure!(now < self.hello.expires_at, "login_expired");
        canonical(&reveal.nonce, 32)?;
        ensure!(
            reveal.commitment(&self.hello.client_id)? == self.hello.client_commitment,
            "invalid_login_reveal"
        );
        let client_public = canonical(&reveal.public_key, 65)?;
        let shared = self
            .key
            .compute_shared_key(&client_public)
            .ok_or_else(|| anyhow::anyhow!("invalid_login_reveal"))?;
        Ok(Channel::new(
            &shared,
            &self.hello.transcript(reveal)?,
            self.hello.expires_at,
        ))
    }
}

pub(crate) struct Channel {
    request_key: [u8; 32],
    response_key: [u8; 32],
    pub context: String,
    pub verification_code: String,
    expires_at: u64,
}

impl Channel {
    fn new(shared: &[u8; 32], transcript: &[u8], expires_at: u64) -> Self {
        let context = Sha256::digest(transcript);
        let sas = derive(shared, &context, b"plain-web-login-v2/sas");
        let digits = format!(
            "{:08}",
            u64::from_be_bytes(sas[..8].try_into().unwrap()) % 100_000_000
        );
        Self {
            request_key: derive(shared, &context, b"plain-web-login-v2/request"),
            response_key: derive(shared, &context, b"plain-web-login-v2/response"),
            context: base64_encode(&context),
            verification_code: format!("{} {}", &digits[..4], &digits[4..]),
            expires_at,
        }
    }

    pub(crate) fn fresh(&self, now: u64) -> Result<()> {
        ensure!(now < self.expires_at, "login_expired");
        Ok(())
    }

    pub(crate) fn encrypt_request(&self, bytes: &[u8], now: u64) -> Result<Vec<u8>> {
        self.fresh(now)?;
        xchacha_encrypt_raw(&self.request_key, bytes)
            .ok_or_else(|| anyhow::anyhow!("invalid_login_request"))
    }

    pub(crate) fn decrypt_request(&self, bytes: &[u8], now: u64) -> Result<Vec<u8>> {
        self.fresh(now)?;
        xchacha_decrypt_raw(&self.request_key, bytes)
            .ok_or_else(|| anyhow::anyhow!("invalid_login_request"))
    }

    pub(crate) fn encrypt_response(&self, bytes: &[u8], now: u64) -> Result<Vec<u8>> {
        self.fresh(now)?;
        xchacha_encrypt_raw(&self.response_key, bytes)
            .ok_or_else(|| anyhow::anyhow!("invalid_auth_response"))
    }

    pub(crate) fn decrypt_response(&self, bytes: &[u8], now: u64) -> Result<Vec<u8>> {
        self.fresh(now)?;
        xchacha_decrypt_raw(&self.response_key, bytes)
            .ok_or_else(|| anyhow::anyhow!("invalid_auth_response"))
    }
}

fn identity(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 256,
        "invalid_login_identity"
    );
    Ok(())
}

fn canonical(value: &str, length: usize) -> Result<Vec<u8>> {
    let bytes = base64_decode(value);
    ensure!(
        bytes.len() == length && base64_encode(&bytes) == value,
        "invalid_login_key"
    );
    Ok(bytes)
}

fn nonce() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64_encode(&bytes)
}

fn derive(shared: &[u8], salt: &[u8], info: &[u8]) -> [u8; 32] {
    let mut extract = Hmac::<sha2_mac::Sha256>::new_from_slice(salt).unwrap();
    extract.update(shared);
    let prk = extract.finalize().into_bytes();
    let mut expand = Hmac::<sha2_mac::Sha256>::new_from_slice(&prk).unwrap();
    expand.update(info);
    expand.update(&[1]);
    expand.finalize().into_bytes().into()
}

#[cfg(test)]
#[path = "../../../tests/unit/http/auth/login_channel.rs"]
mod tests;
