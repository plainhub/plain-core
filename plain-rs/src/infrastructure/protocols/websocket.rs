use plain_libs::crypto::xchacha_decrypt_raw_with_aad;
use plain_libs::crypto::xchacha_encrypt_raw_with_aad;
use sha2::{Digest, Sha256};

fn fields(values: &[&[u8]]) -> Option<Vec<u8>> {
    let mut encoded = Vec::new();
    for value in values {
        encoded.extend_from_slice(&u32::try_from(value.len()).ok()?.to_be_bytes());
        encoded.extend_from_slice(value);
    }
    Some(encoded)
}
fn init_aad(client: &str, path: &str) -> Option<Vec<u8>> {
    fields(&[b"plain-ws-init", path.as_bytes(), client.as_bytes()])
}
pub fn seal_init(key: &[u8], client: &str, path: &str, plain: &[u8]) -> Option<Vec<u8>> {
    xchacha_encrypt_raw_with_aad(key, plain, &init_aad(client, path)?)
}
pub fn open_init(key: &[u8], client: &str, path: &str, frame: &[u8]) -> Option<Vec<u8>> {
    xchacha_decrypt_raw_with_aad(key, frame, &init_aad(client, path)?)
}
fn ack_aad(client: &str, path: &str, init: &[u8]) -> Option<Vec<u8>> {
    fields(&[
        b"plain-ws-ack",
        path.as_bytes(),
        client.as_bytes(),
        &Sha256::digest(init),
    ])
}
pub fn seal_ack(
    key: &[u8],
    client: &str,
    path: &str,
    init: &[u8],
    plain: &[u8],
) -> Option<Vec<u8>> {
    xchacha_encrypt_raw_with_aad(key, plain, &ack_aad(client, path, init)?)
}
pub fn open_ack(
    key: &[u8],
    client: &str,
    path: &str,
    init: &[u8],
    frame: &[u8],
) -> Option<Vec<u8>> {
    xchacha_decrypt_raw_with_aad(key, frame, &ack_aad(client, path, init)?)
}
#[derive(Clone, Copy)]
pub enum Role {
    Client,
    Server,
}
pub struct Stream {
    path: String,
    client: String,
    binding: [u8; 32],
    role: Role,
    sent: u64,
    received: u64,
}
impl Stream {
    pub fn new(path: &str, client: &str, init: &[u8], ack: &[u8], role: Role) -> Option<Self> {
        Some(Self {
            path: path.into(),
            client: client.into(),
            binding: Sha256::digest(fields(&[init, ack])?).into(),
            role,
            sent: 0,
            received: 0,
        })
    }
    fn aad(&self, sending: bool, sequence: u64) -> Option<Vec<u8>> {
        let client_direction = matches!(
            (self.role, sending),
            (Role::Client, true) | (Role::Server, false)
        );
        fields(&[
            b"plain-ws-frame",
            self.path.as_bytes(),
            self.client.as_bytes(),
            &self.binding,
            if client_direction {
                b"client"
            } else {
                b"server"
            },
            &sequence.to_be_bytes(),
        ])
    }
    pub fn encode(&mut self, plain: &[u8], key: &[u8]) -> Option<Vec<u8>> {
        let next = self.sent.checked_add(1)?;
        let mut frame = next.to_be_bytes().to_vec();
        frame.extend_from_slice(&xchacha_encrypt_raw_with_aad(
            key,
            plain,
            &self.aad(true, next)?,
        )?);
        self.sent = next;
        Some(frame)
    }
    pub fn decode(&mut self, frame: &[u8], key: &[u8]) -> Option<Vec<u8>> {
        let sequence = u64::from_be_bytes(frame.get(..8)?.try_into().ok()?);
        let next = self.received.checked_add(1)?;
        if sequence != next {
            return None;
        }
        let plain =
            xchacha_decrypt_raw_with_aad(key, frame.get(8..)?, &self.aad(false, sequence)?)?;
        self.received = next;
        Some(plain)
    }
}

use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Init {
    pub client_nonce: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ack {
    pub server_nonce: String,
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

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/protocols/websocket.rs"]
mod tests;
