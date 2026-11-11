use sha2::{Digest, Sha256};

pub struct Context(Vec<u8>);

impl Context {
    pub fn new(request: &[u8]) -> Self {
        let mut aad = b"r\0".to_vec();
        aad.extend_from_slice(&Sha256::digest(request));
        Self(aad)
    }

    pub fn encrypt(&self, key: &[u8], plaintext: &[u8]) -> Option<Vec<u8>> {
        plain_libs::crypto::xchacha_encrypt_raw_with_aad(key, plaintext, &self.0)
    }

    pub fn decrypt(&self, key: &[u8], response: &[u8]) -> Option<Vec<u8>> {
        plain_libs::crypto::xchacha_decrypt_raw_with_aad(key, response, &self.0)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/transport/http_response.rs"]
mod tests;
