use sha2::{Digest, Sha256};

pub struct Context(Vec<u8>);

impl Context {
    pub fn new(request: &[u8]) -> Self {
        let mut binding = vec![0];
        binding.extend_from_slice(&Sha256::digest(request));
        Self(binding)
    }

    pub fn encrypt(&self, key: &[u8], plaintext: &[u8]) -> Option<Vec<u8>> {
        let mut body = self.0.clone();
        body.extend_from_slice(plaintext);
        plain_libs::crypto::xchacha_encrypt_raw(key, &body)
    }

    pub fn decrypt(&self, key: &[u8], response: &[u8]) -> Option<Vec<u8>> {
        let body = plain_libs::crypto::xchacha_decrypt_raw(key, response)?;
        Some(body.strip_prefix(self.0.as_slice())?.to_vec())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/transport/http_response.rs"]
mod tests;
