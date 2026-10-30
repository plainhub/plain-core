use crate::utils::base64::base64_encode;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientHello {
    pub commitment: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientReveal {
    pub public_key: String,
    pub nonce: String,
}

impl ClientReveal {
    pub fn commitment(&self, client_id: &str, domain: &str) -> Result<String> {
        Ok(base64_encode(&Sha256::digest(serde_json::to_vec(
            &serde_json::json!([domain, client_id, self.public_key, self.nonce]),
        )?)))
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerHello {
    pub client_id: String,
    pub device_id: String,
    pub client_commitment: String,
    pub server_public_key: String,
    pub server_nonce: String,
    pub expires_at: u64,
    pub signature: String,
}

impl ServerHello {
    pub fn fields(&self, domain: &str) -> Vec<serde_json::Value> {
        vec![
            domain.into(),
            self.client_id.clone().into(),
            self.device_id.clone().into(),
            self.client_commitment.clone().into(),
            self.server_public_key.clone().into(),
            self.server_nonce.clone().into(),
            self.expires_at.into(),
        ]
    }

    pub fn signed_transcript(&self, domain: &str) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&self.fields(domain))?)
    }

    pub fn transcript(&self, reveal: &ClientReveal, domain: &str) -> Result<Vec<u8>> {
        let mut fields = self.fields(domain);
        fields.extend([
            reveal.public_key.clone().into(),
            reveal.nonce.clone().into(),
        ]);
        Ok(serde_json::to_vec(&fields)?)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(
    tag = "step",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Verification {
    Hello {
        from_id: String,
        request_signature: String,
        hello: ClientHello,
    },
    Challenge {
        request_signature: String,
        hello: ServerHello,
        signature_public_key: String,
    },
    Reveal {
        from_id: String,
        request_signature: String,
        reveal: ClientReveal,
    },
}
