use crate::crypto::{xchacha_decrypt_raw_with_aad, xchacha_encrypt_raw_with_aad};
use sha2::{Digest, Sha256};

fn fields(values: &[&[u8]]) -> Option<Vec<u8>> {
    let mut encoded = Vec::new();
    for value in values {
        encoded.extend_from_slice(&u32::try_from(value.len()).ok()?.to_be_bytes());
        encoded.extend_from_slice(value);
    }
    Some(encoded)
}
fn init_aad(client: &str) -> Option<Vec<u8>> {
    fields(&[b"plain-graphql-init-v1", b"/graphql", client.as_bytes()])
}
pub fn seal_init(key: &[u8], client: &str, plain: &[u8]) -> Option<Vec<u8>> {
    let mut frame = vec![1];
    frame.extend_from_slice(&xchacha_encrypt_raw_with_aad(
        key,
        plain,
        &init_aad(client)?,
    )?);
    Some(frame)
}
pub fn open_init(key: &[u8], client: &str, frame: &[u8]) -> Option<Vec<u8>> {
    if frame.first() != Some(&1) {
        return None;
    }
    xchacha_decrypt_raw_with_aad(key, frame.get(1..)?, &init_aad(client)?)
}
fn ack_aad(client: &str, init: &[u8]) -> Option<Vec<u8>> {
    fields(&[
        b"plain-graphql-ack-v1",
        b"/graphql",
        client.as_bytes(),
        &Sha256::digest(init),
    ])
}
pub fn seal_ack(key: &[u8], client: &str, init: &[u8], plain: &[u8]) -> Option<Vec<u8>> {
    let mut frame = vec![1];
    frame.extend_from_slice(&xchacha_encrypt_raw_with_aad(
        key,
        plain,
        &ack_aad(client, init)?,
    )?);
    Some(frame)
}
pub fn open_ack(key: &[u8], client: &str, init: &[u8], frame: &[u8]) -> Option<Vec<u8>> {
    if frame.first() != Some(&1) {
        return None;
    }
    xchacha_decrypt_raw_with_aad(key, frame.get(1..)?, &ack_aad(client, init)?)
}
#[derive(Clone, Copy)]
pub enum Role {
    Client,
    Server,
}
pub struct Stream {
    client: String,
    binding: [u8; 32],
    role: Role,
    sent: u64,
    received: u64,
}
impl Stream {
    pub fn new(client: &str, init: &[u8], ack: &[u8], role: Role) -> Option<Self> {
        Some(Self {
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
            b"plain-graphql-frame-v1",
            b"/graphql",
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
        let mut frame = vec![1];
        frame.extend_from_slice(&self.sent.to_be_bytes());
        frame.extend_from_slice(&xchacha_encrypt_raw_with_aad(
            key,
            plain,
            &self.aad(true, self.sent)?,
        )?);
        self.sent = next;
        Some(frame)
    }
    pub fn decode(&mut self, frame: &[u8], key: &[u8]) -> Option<Vec<u8>> {
        if frame.first() != Some(&1) {
            return None;
        }
        let sequence = u64::from_be_bytes(frame.get(1..9)?.try_into().ok()?);
        if sequence != self.received {
            return None;
        }
        let next = self.received.checked_add(1)?;
        let plain =
            xchacha_decrypt_raw_with_aad(key, frame.get(9..)?, &self.aad(false, sequence)?)?;
        self.received = next;
        Some(plain)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeerSubscriptionTarget {
    pub peer_id: String,
    pub public_key: String,
}

#[cfg(feature = "graphql")]
pub mod messages;

use serde::{Deserialize, Serialize};
pub const PROTOCOL: &str = "plain-graphql-ws-v1";
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Init {
    pub timestamp_millis: i64,
    pub client_nonce: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ack {
    pub server_nonce: String,
}

pub mod events;
