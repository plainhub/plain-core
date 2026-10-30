use anyhow::{Result, ensure};
use hmac::{Hmac, KeyInit, Mac};
use plain_rs::{
    crypto::{EcdhSession, ed25519_sign, ed25519_verify, xchacha_decrypt_raw, xchacha_encrypt_raw},
    utils::base64::{base64_decode, base64_encode},
};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfirmationMode {
    Password,
    DeviceConfirmation,
    ChatPairing,
}

impl ConfirmationMode {
    fn domain(self) -> &'static str {
        match self {
            Self::Password => "plain-web-password-login-v1",
            Self::DeviceConfirmation => "plain-web-login-v2",
            Self::ChatPairing => "plain-chat-pairing-v1",
        }
    }
}
const LIFETIME_MS: u64 = 300_000;

pub(crate) use plain_rs::peers::pairing::confirmation::{ClientHello, ClientReveal, ServerHello};

pub(crate) struct Client {
    mode: ConfirmationMode,
    client_id: String,
    key: EcdhSession,
    nonce: String,
}

impl Client {
    pub(crate) fn start(client_id: &str, mode: ConfirmationMode) -> Result<(Self, ClientHello)> {
        identity(client_id)?;
        let client = Self {
            mode,
            client_id: client_id.into(),
            key: EcdhSession::generate(),
            nonce: nonce(),
        };
        let hello = ClientHello {
            commitment: client.reveal().commitment(client_id, mode.domain())?,
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
            hello.client_commitment == reveal.commitment(&self.client_id, self.mode.domain())?,
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
            ed25519_verify(
                public_key,
                &hello.signed_transcript(self.mode.domain())?,
                &hello.signature
            ),
            "signature_verification_failed"
        );
        let shared = self
            .key
            .compute_shared_key(&server_public)
            .ok_or_else(|| anyhow::anyhow!("invalid_auth_response"))?;
        Ok((
            Channel::new(
                &shared,
                &hello.transcript(&reveal, self.mode.domain())?,
                self.mode,
                hello.expires_at.min(now.saturating_add(LIFETIME_MS)),
            ),
            reveal,
        ))
    }
}

pub(crate) struct ServerHandshake {
    mode: ConfirmationMode,
    key: EcdhSession,
    hello: ServerHello,
}

impl ServerHandshake {
    pub(crate) fn start(
        client_id: &str,
        mode: ConfirmationMode,
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
        response.signature = ed25519_sign(signing_key, &response.signed_transcript(mode.domain())?);
        ensure!(
            !response.signature.is_empty(),
            "signature_verification_failed"
        );
        Ok((
            Self {
                mode,
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
            reveal.commitment(&self.hello.client_id, self.mode.domain())?
                == self.hello.client_commitment,
            "invalid_login_reveal"
        );
        let client_public = canonical(&reveal.public_key, 65)?;
        let shared = self
            .key
            .compute_shared_key(&client_public)
            .ok_or_else(|| anyhow::anyhow!("invalid_login_reveal"))?;
        Ok(Channel::new(
            &shared,
            &self.hello.transcript(reveal, self.mode.domain())?,
            self.mode,
            self.hello.expires_at,
        ))
    }
}

pub(crate) struct Channel {
    pub mode: ConfirmationMode,
    request_key: [u8; 32],
    response_key: [u8; 32],
    pub context: String,
    pub verification_code: String,
    expires_at: u64,
}

impl Channel {
    fn new(shared: &[u8; 32], transcript: &[u8], mode: ConfirmationMode, expires_at: u64) -> Self {
        let context = Sha256::digest(transcript);
        let domain = mode.domain();
        let sas = derive(shared, &context, format!("{domain}/sas").as_bytes());
        let digits = format!(
            "{:08}",
            u64::from_be_bytes(sas[..8].try_into().unwrap()) % 100_000_000
        );
        Self {
            mode,
            request_key: derive(shared, &context, format!("{domain}/request").as_bytes()),
            response_key: derive(shared, &context, format!("{domain}/response").as_bytes()),
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
#[path = "../../tests/unit/services/confirmation_channel.rs"]
mod tests;
