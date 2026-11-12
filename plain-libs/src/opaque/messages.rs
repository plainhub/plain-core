// Copyright (c) Meta Platforms, Inc. and affiliates; adapted under the retained source licenses.
use super::{ProtocolError, RistrettoPoint, Zeroizing, oprf};

pub(super) fn take<const N: usize>(
    input: &mut &[u8],
    name: &'static str,
) -> Result<[u8; N], ProtocolError> {
    let (bytes, rest) = input
        .split_first_chunk::<N>()
        .ok_or(ProtocolError::SizeError {
            name,
            len: N,
            actual_len: input.len(),
        })?;
    *input = rest;
    Ok(*bytes)
}
pub(super) fn pack<const N: usize>(parts: &[&[u8]]) -> [u8; N] {
    let mut result = [0; N];
    let mut offset = 0;
    for part in parts {
        result[offset..offset + part.len()].copy_from_slice(part);
        offset += part.len();
    }
    debug_assert_eq!(offset, N);
    result
}
#[derive(Clone, Copy)]
pub struct PublicKey(pub(super) RistrettoPoint);
impl PublicKey {
    pub fn serialize(&self) -> [u8; 32] {
        self.0.compress().to_bytes()
    }
}
#[derive(Clone)]
pub struct RegistrationRequest(pub(super) RistrettoPoint);
impl RegistrationRequest {
    pub fn serialize(&self) -> [u8; 32] {
        self.0.compress().to_bytes()
    }
}
pub struct RegistrationResponse {
    pub(super) evaluation: RistrettoPoint,
    pub(super) server_s_pk: PublicKey,
}
impl RegistrationResponse {
    pub fn serialize(&self) -> [u8; 64] {
        pack(&[
            &self.evaluation.compress().to_bytes(),
            &self.server_s_pk.serialize(),
        ])
    }
}
pub struct RegistrationUpload {
    pub(super) client_s_pk: PublicKey,
    pub(super) masking_key: Zeroizing<[u8; 64]>,
    pub(super) envelope_nonce: [u8; 32],
    pub(super) envelope_mac: [u8; 64],
}
impl RegistrationUpload {
    pub fn serialize(&self) -> [u8; 192] {
        pack(&[
            &self.client_s_pk.serialize(),
            &self.masking_key[..],
            &self.envelope_nonce,
            &self.envelope_mac,
        ])
    }
    pub fn deserialize(mut bytes: &[u8]) -> Result<Self, ProtocolError> {
        Ok(Self {
            client_s_pk: PublicKey(oprf::decode_point(take(&mut bytes, "public key")?)?),
            masking_key: Zeroizing::new(take(&mut bytes, "masking key")?),
            envelope_nonce: take(&mut bytes, "nonce")?,
            envelope_mac: take(&mut bytes, "hmac")?,
        })
    }
}
#[derive(Clone)]
pub struct CredentialRequest {
    pub(super) blinded: RistrettoPoint,
    pub(super) nonce: [u8; 32],
    pub(super) public: PublicKey,
}
impl CredentialRequest {
    pub fn serialize(&self) -> [u8; 96] {
        pack(&[
            &self.blinded.compress().to_bytes(),
            &self.nonce,
            &self.public.serialize(),
        ])
    }
    pub fn deserialize(mut bytes: &[u8]) -> Result<Self, ProtocolError> {
        Ok(Self {
            blinded: oprf::decode_point(take(&mut bytes, "blinded element")?)?,
            nonce: take(&mut bytes, "client nonce")?,
            public: PublicKey(oprf::decode_point(take(&mut bytes, "public key")?)?),
        })
    }
}
pub struct CredentialResponse {
    pub(super) evaluation: RistrettoPoint,
    pub(super) masking_nonce: [u8; 32],
    pub(super) masked: [u8; 128],
    pub(super) nonce: [u8; 32],
    pub(super) public: PublicKey,
    pub(super) mac: [u8; 64],
}
impl CredentialResponse {
    pub fn serialize(&self) -> [u8; 320] {
        pack(&[
            &self.evaluation.compress().to_bytes(),
            &self.masking_nonce,
            &self.masked,
            &self.nonce,
            &self.public.serialize(),
            &self.mac,
        ])
    }
    pub fn deserialize(mut bytes: &[u8]) -> Result<Self, ProtocolError> {
        Ok(Self {
            evaluation: oprf::decode_point(take(&mut bytes, "evaluation element")?)?,
            masking_nonce: take(&mut bytes, "masking nonce")?,
            masked: take(&mut bytes, "masked response")?,
            nonce: take(&mut bytes, "server nonce")?,
            public: PublicKey(oprf::decode_point(take(&mut bytes, "public key")?)?),
            mac: take(&mut bytes, "mac")?,
        })
    }
}
pub struct CredentialFinalization(pub(super) [u8; 64]);
impl CredentialFinalization {
    pub fn serialize(&self) -> [u8; 64] {
        self.0
    }
    pub fn deserialize(mut bytes: &[u8]) -> Result<Self, ProtocolError> {
        Ok(Self(take(&mut bytes, "mac")?))
    }
}
