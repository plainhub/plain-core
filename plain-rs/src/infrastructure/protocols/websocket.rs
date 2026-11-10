use plain_libs::crypto::xchacha_decrypt_raw_with_aad;
use plain_libs::crypto::xchacha_encrypt_raw_with_aad;
use plain_libs::utils::base64::base64_decode;
use sha2::{Digest, Sha256};

/// Client frames bind the route and client, then controls bind this connection.
pub fn request_aad(path: &str, client_id: &str, registration: Option<&[u8]>) -> Vec<u8> {
    let mut aad = b"ws-request\0".to_vec();
    aad.extend_from_slice(path.as_bytes());
    aad.push(0);
    aad.extend_from_slice(client_id.as_bytes());
    aad.push(0);
    if let Some(registration) = registration {
        aad.extend_from_slice(b"control\0");
        aad.extend_from_slice(&Sha256::digest(registration));
    } else {
        aad.extend_from_slice(b"register");
    }
    aad
}

pub fn encode_raw(event_name: &str, payload: &[u8]) -> Option<Vec<u8>> {
    if !valid_name(event_name) {
        return None;
    }
    let mut frame = Vec::with_capacity(event_name.len() + 1 + payload.len());
    frame.extend_from_slice(event_name.as_bytes());
    frame.push(0);
    frame.extend_from_slice(payload);
    Some(frame)
}

pub fn decode_raw(frame: &[u8]) -> Option<(&str, &[u8])> {
    let end = frame.iter().position(|&byte| byte == 0)?;
    let name = std::str::from_utf8(&frame[..end]).ok()?;
    if !valid_name(name) {
        return None;
    }
    Some((name, &frame[end + 1..]))
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte == b'_')
}

pub fn encode(event_name: &str, payload: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    encode_raw(
        event_name,
        &xchacha_encrypt_raw_with_aad(key, payload, &event_aad(event_name))?,
    )
}

pub fn decode(frame: &[u8], key: &[u8]) -> Option<(String, Vec<u8>)> {
    let (name, encrypted) = decode_raw(frame)?;
    Some((
        name.to_owned(),
        xchacha_decrypt_raw_with_aad(key, encrypted, &event_aad(name))?,
    ))
}

fn event_aad(name: &str) -> Vec<u8> {
    let mut aad = b"plain-event-v1\0".to_vec();
    aad.extend_from_slice(name.as_bytes());
    aad
}

pub struct Stream {
    binding: [u8; 32],
    next: u64,
}

impl Stream {
    pub fn new(registration: &[u8]) -> Self {
        Self {
            binding: Sha256::digest(registration).into(),
            next: 0,
        }
    }

    fn aad(&self, name: &str, sequence: u64) -> Vec<u8> {
        let mut aad = b"plain-ws-event-v1\0".to_vec();
        aad.extend_from_slice(&self.binding);
        aad.extend_from_slice(&sequence.to_be_bytes());
        aad.extend_from_slice(name.as_bytes());
        aad
    }

    pub fn encode(&mut self, name: &str, payload: &[u8], key: &[u8]) -> Option<Vec<u8>> {
        if !valid_name(name) {
            return None;
        }
        let next = self.next.checked_add(1)?;
        let cipher = xchacha_encrypt_raw_with_aad(key, payload, &self.aad(name, self.next))?;
        let mut body = self.next.to_be_bytes().to_vec();
        body.extend_from_slice(&cipher);
        let frame = encode_raw(name, &body)?;
        self.next = next;
        Some(frame)
    }

    pub fn decode(&mut self, frame: &[u8], key: &[u8]) -> Option<(String, Vec<u8>)> {
        let (name, body) = decode_raw(frame)?;
        let sequence = u64::from_be_bytes(body.get(..8)?.try_into().ok()?);
        if sequence != self.next {
            return None;
        }
        let next = self.next.checked_add(1)?;
        let payload = xchacha_decrypt_raw_with_aad(key, body.get(8..)?, &self.aad(name, sequence))?;
        self.next = next;
        Some((name.to_owned(), payload))
    }
}

pub fn encode_with_token(event_name: &str, payload: &[u8], token_b64: &str) -> Option<Vec<u8>> {
    encode(event_name, payload, &base64_decode(token_b64))
}

pub fn decode_with_token(frame: &[u8], token_b64: &str) -> Option<(String, Vec<u8>)> {
    decode(frame, &base64_decode(token_b64))
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/protocols/websocket.rs"]
mod tests;
