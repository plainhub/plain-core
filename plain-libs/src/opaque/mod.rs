// Copyright (c) Meta Platforms, Inc. and affiliates; adapted under the retained source licenses.
#![forbid(unsafe_code)]
// Adapted from opaque-ke4.1.0-pre.2 for the existing fixed Ristretto255/SHA512/TripleDH suite; see SOURCE.json.
pub mod errors;
mod keys;
mod messages;
use crate::oprf::{self, Blind, NonZeroScalar};
use argon2::Argon2;
use curve25519_dalek::ristretto::RistrettoPoint;
use errors::ProtocolError;
use hkdf::{
    Hkdf, HkdfExtract,
    hmac::{Hmac, Mac},
};
use keys::{client_static_key, derive_keys, envelope_mac, expand, randomized_password, transcript};
pub use messages::{
    CredentialFinalization, CredentialRequest, CredentialResponse, PublicKey, RegistrationRequest,
    RegistrationResponse, RegistrationUpload,
};
use messages::{pack, take};
use rand_core::{CryptoRng, RngCore};
use sha2::{Digest, Sha512};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub struct ServerSetup {
    oprf_seed: Zeroizing<[u8; 64]>,
    secret: NonZeroScalar,
    public: PublicKey,
    dummy: PublicKey,
}
impl ServerSetup {
    pub fn new(rng: &mut (impl RngCore + CryptoRng)) -> Self {
        let secret = oprf::random_scalar(rng);
        let public = PublicKey(oprf::public_key(&secret));
        let mut oprf_seed = Zeroizing::new([0; 64]);
        rng.fill_bytes(&mut oprf_seed[..]);
        let dummy = PublicKey(oprf::public_key(&oprf::random_scalar(rng)));
        Self {
            oprf_seed,
            secret,
            public,
            dummy,
        }
    }
    pub fn serialize(&self) -> [u8; 128] {
        pack(&[
            &self.oprf_seed[..],
            &self.secret.to_bytes(),
            &self.dummy.serialize(),
        ])
    }
    pub fn deserialize(mut bytes: &[u8]) -> Result<Self, ProtocolError> {
        let oprf_seed = Zeroizing::new(take(&mut bytes, "oprf seed")?);
        let secret = oprf::decode_scalar(take(&mut bytes, "secret key")?)?;
        let public = PublicKey(oprf::public_key(&secret));
        let dummy = PublicKey(oprf::decode_point(take(&mut bytes, "public key")?)?);
        Ok(Self {
            oprf_seed,
            secret,
            public,
            dummy,
        })
    }
    pub fn public_key(&self) -> PublicKey {
        self.public
    }
    fn oprf_key(&self, credential: &[u8]) -> Result<NonZeroScalar, ProtocolError> {
        let mut seed = Zeroizing::new([0; 32]);
        expand(
            &self.oprf_seed[..],
            &[credential, b"OprfKey"],
            &mut seed[..],
        )?;
        oprf::derive_key(&seed[..], b"OPAQUE-DeriveKeyPair")
    }
}

#[derive(Clone, Copy, Default)]
pub struct Identifiers<'a> {
    pub client: Option<&'a [u8]>,
    pub server: Option<&'a [u8]>,
}
pub struct ClientRegistrationFinishParameters<'a> {
    pub identifiers: Identifiers<'a>,
    pub ksf: Option<&'a Argon2<'a>>,
}
impl<'a> ClientRegistrationFinishParameters<'a> {
    pub fn new(identifiers: Identifiers<'a>, ksf: Option<&'a Argon2<'a>>) -> Self {
        Self { identifiers, ksf }
    }
}
#[derive(Default)]
pub struct ClientLoginFinishParameters<'a> {
    pub context: Option<&'a [u8]>,
    pub identifiers: Identifiers<'a>,
    pub ksf: Option<&'a Argon2<'a>>,
}
impl<'a> ClientLoginFinishParameters<'a> {
    pub fn new(
        context: Option<&'a [u8]>,
        identifiers: Identifiers<'a>,
        ksf: Option<&'a Argon2<'a>>,
    ) -> Self {
        Self {
            context,
            identifiers,
            ksf,
        }
    }
}
#[derive(Default)]
pub struct ServerLoginParameters<'a> {
    pub context: Option<&'a [u8]>,
    pub identifiers: Identifiers<'a>,
}

pub struct ClientRegistration {
    blind: Blind,
    blinded: RistrettoPoint,
}
pub struct ClientRegistrationStartResult {
    pub state: ClientRegistration,
    pub message: RegistrationRequest,
}
pub struct ClientRegistrationFinishResult {
    pub message: RegistrationUpload,
    pub export_key: Zeroizing<[u8; 64]>,
    pub server_s_pk: PublicKey,
}
impl ClientRegistration {
    pub fn start(
        rng: &mut (impl RngCore + CryptoRng),
        password: &[u8],
    ) -> Result<ClientRegistrationStartResult, ProtocolError> {
        let (blind, element) = Blind::new(password, rng);
        let message = RegistrationRequest(element);
        Ok(ClientRegistrationStartResult {
            state: Self {
                blind,
                blinded: element,
            },
            message,
        })
    }
    pub fn finish(
        self,
        rng: &mut (impl RngCore + CryptoRng),
        password: &[u8],
        response: RegistrationResponse,
        params: ClientRegistrationFinishParameters<'_>,
    ) -> Result<ClientRegistrationFinishResult, ProtocolError> {
        if bool::from(self.blinded.ct_eq(&response.evaluation)) {
            return Err(ProtocolError::ReflectedValueError);
        }
        let password_key =
            randomized_password(&self.blind, password, response.evaluation, params.ksf)?;
        let mut masking_key = Zeroizing::new([0; 64]);
        password_key
            .expand(b"MaskingKey", &mut masking_key[..])
            .map_err(|_| ProtocolError::LibraryError)?;
        let mut nonce = [0; 32];
        rng.fill_bytes(&mut nonce);
        let client_secret = client_static_key(&password_key, &nonce)?;
        let client_public = PublicKey(oprf::public_key(&client_secret));
        let mac = envelope_mac(
            &password_key,
            &nonce,
            client_public,
            response.server_s_pk,
            params.identifiers,
        )?;
        let mut export_key = Zeroizing::new([0; 64]);
        password_key
            .expand_multi_info(&[&nonce, b"ExportKey"], &mut export_key[..])
            .map_err(|_| ProtocolError::LibraryError)?;
        Ok(ClientRegistrationFinishResult {
            message: RegistrationUpload {
                client_s_pk: client_public,
                masking_key,
                envelope_nonce: nonce,
                envelope_mac: mac,
            },
            export_key,
            server_s_pk: response.server_s_pk,
        })
    }
}

pub struct ServerRegistration(pub(crate) RegistrationUpload);
pub struct ServerRegistrationStartResult {
    pub message: RegistrationResponse,
}
impl ServerRegistration {
    pub fn start(
        setup: &ServerSetup,
        request: RegistrationRequest,
        credential: &[u8],
    ) -> Result<ServerRegistrationStartResult, ProtocolError> {
        let key = setup.oprf_key(credential)?;
        Ok(ServerRegistrationStartResult {
            message: RegistrationResponse {
                evaluation: request.0 * *key,
                server_s_pk: setup.public,
            },
        })
    }
    pub fn finish(message: RegistrationUpload) -> Self {
        Self(message)
    }
    pub fn serialize(&self) -> [u8; 192] {
        self.0.serialize()
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self, ProtocolError> {
        Ok(Self(RegistrationUpload::deserialize(bytes)?))
    }
    fn dummy(rng: &mut (impl RngCore + CryptoRng), setup: &ServerSetup) -> Self {
        let mut masking_key = Zeroizing::new([0; 64]);
        rng.fill_bytes(&mut masking_key[..]);
        Self(RegistrationUpload {
            client_s_pk: setup.dummy,
            masking_key,
            envelope_nonce: [0; 32],
            envelope_mac: [0; 64],
        })
    }
}

pub struct ClientLogin {
    blind: Blind,
    secret: NonZeroScalar,
    request: CredentialRequest,
}
pub struct ClientLoginStartResult {
    pub state: ClientLogin,
    pub message: CredentialRequest,
}
pub struct ClientLoginFinishResult {
    pub message: CredentialFinalization,
    pub session_key: Zeroizing<[u8; 64]>,
    pub export_key: Zeroizing<[u8; 64]>,
    pub server_s_pk: PublicKey,
}
impl ClientLogin {
    pub fn start(
        rng: &mut (impl RngCore + CryptoRng),
        password: &[u8],
    ) -> Result<ClientLoginStartResult, ProtocolError> {
        let (blind, element) = Blind::new(password, rng);
        let mut seed = Zeroizing::new([0; 32]);
        rng.fill_bytes(&mut seed[..]);
        let secret = oprf::derive_key(&seed[..], b"OPAQUE-DeriveDiffieHellmanKeyPair")?;
        let mut nonce = [0; 32];
        rng.fill_bytes(&mut nonce);
        let request = CredentialRequest {
            blinded: element,
            nonce,
            public: PublicKey(oprf::public_key(&secret)),
        };
        Ok(ClientLoginStartResult {
            state: Self {
                blind,
                secret,
                request: request.clone(),
            },
            message: request,
        })
    }
    pub fn finish(
        self,
        _rng: &mut (impl RngCore + CryptoRng),
        password: &[u8],
        response: CredentialResponse,
        params: ClientLoginFinishParameters<'_>,
    ) -> Result<ClientLoginFinishResult, ProtocolError> {
        if bool::from(self.request.blinded.ct_eq(&response.evaluation)) {
            return Err(ProtocolError::ReflectedValueError);
        }
        let password_key =
            randomized_password(&self.blind, password, response.evaluation, params.ksf)?;
        let mut masking_key = Zeroizing::new([0; 64]);
        password_key
            .expand(b"MaskingKey", &mut masking_key[..])
            .map_err(|_| ProtocolError::LibraryError)?;
        let mut unmasked = Zeroizing::new([0; 128]);
        expand(
            &masking_key[..],
            &[&response.masking_nonce, b"CredentialResponsePad"],
            &mut unmasked[..],
        )?;
        for (a, b) in unmasked.iter_mut().zip(response.masked.iter()) {
            *a ^= b;
        }
        let mut bytes = &unmasked[..];
        let server_s_pk = PublicKey(
            oprf::decode_point(take(&mut bytes, "public key")?)
                .map_err(|_| ProtocolError::InvalidLoginError)?,
        );
        let nonce = take(&mut bytes, "nonce")?;
        let received_mac: [u8; 64] = take(&mut bytes, "hmac")?;
        let client_secret = client_static_key(&password_key, &nonce)?;
        let client_public = PublicKey(oprf::public_key(&client_secret));
        let expected = envelope_mac(
            &password_key,
            &nonce,
            client_public,
            server_s_pk,
            params.identifiers,
        )?;
        if !bool::from(expected.ct_eq(&received_mac)) {
            return Err(ProtocolError::InvalidLoginError);
        }
        let mut hash = transcript(
            params.context,
            params.identifiers,
            client_public,
            server_s_pk,
            &self.request,
            &response,
        )?;
        let dh1 = Zeroizing::new((response.public.0 * *self.secret).compress().to_bytes());
        let dh2 = Zeroizing::new((server_s_pk.0 * *self.secret).compress().to_bytes());
        let dh3 = Zeroizing::new((response.public.0 * *client_secret).compress().to_bytes());
        let digest = hash.clone().finalize();
        let keys = derive_keys(&[&dh1[..], &dh2[..], &dh3[..]], &digest)?;
        let server_mac = keys.mac(b"ServerMAC", &digest)?;
        if !bool::from(server_mac.ct_eq(&response.mac)) {
            return Err(ProtocolError::InvalidLoginError);
        }
        hash.update(response.mac);
        let client_mac = keys.mac(b"ClientMAC", &hash.finalize())?;
        let mut export_key = Zeroizing::new([0; 64]);
        password_key
            .expand_multi_info(&[&nonce, b"ExportKey"], &mut export_key[..])
            .map_err(|_| ProtocolError::LibraryError)?;
        Ok(ClientLoginFinishResult {
            message: CredentialFinalization(client_mac),
            session_key: keys.session,
            export_key,
            server_s_pk,
        })
    }
}

pub struct ServerLogin {
    session: Zeroizing<[u8; 64]>,
    expected_mac: Zeroizing<[u8; 64]>,
}
pub struct ServerLoginStartResult {
    pub state: ServerLogin,
    pub message: CredentialResponse,
}
pub struct ServerLoginFinishResult {
    pub session_key: Zeroizing<[u8; 64]>,
}
impl ServerLogin {
    pub fn start(
        rng: &mut (impl RngCore + CryptoRng),
        setup: &ServerSetup,
        record: Option<ServerRegistration>,
        request: CredentialRequest,
        credential: &[u8],
        params: ServerLoginParameters<'_>,
    ) -> Result<ServerLoginStartResult, ProtocolError> {
        let dummy = ServerRegistration::dummy(rng, setup);
        let record = record.unwrap_or(dummy);
        let mut masking_nonce = [0; 32];
        rng.fill_bytes(&mut masking_nonce);
        let mut masked = [0; 128];
        expand(
            &record.0.masking_key[..],
            &[&masking_nonce, b"CredentialResponsePad"],
            &mut masked,
        )?;
        let envelope = pack::<128>(&[
            &setup.public.serialize(),
            &record.0.envelope_nonce,
            &record.0.envelope_mac,
        ]);
        for (a, b) in masked.iter_mut().zip(envelope.iter()) {
            *a ^= b;
        }
        let key = setup.oprf_key(credential)?;
        let evaluation = request.blinded * *key;
        let mut seed = Zeroizing::new([0; 32]);
        rng.fill_bytes(&mut seed[..]);
        let secret = oprf::derive_key(&seed[..], b"OPAQUE-DeriveDiffieHellmanKeyPair")?;
        let mut nonce = [0; 32];
        rng.fill_bytes(&mut nonce);
        let mut response = CredentialResponse {
            evaluation,
            masking_nonce,
            masked,
            nonce,
            public: PublicKey(oprf::public_key(&secret)),
            mac: [0; 64],
        };
        let mut hash = transcript(
            params.context,
            params.identifiers,
            record.0.client_s_pk,
            setup.public,
            &request,
            &response,
        )?;
        let dh1 = Zeroizing::new((request.public.0 * *secret).compress().to_bytes());
        let dh2 = Zeroizing::new((request.public.0 * *setup.secret).compress().to_bytes());
        let dh3 = Zeroizing::new((record.0.client_s_pk.0 * *secret).compress().to_bytes());
        let keys = derive_keys(&[&dh1[..], &dh2[..], &dh3[..]], &hash.clone().finalize())?;
        response.mac = keys.mac(b"ServerMAC", &hash.clone().finalize())?;
        hash.update(response.mac);
        let expected_mac = Zeroizing::new(keys.mac(b"ClientMAC", &hash.finalize())?);
        Ok(ServerLoginStartResult {
            state: Self {
                session: keys.session,
                expected_mac,
            },
            message: response,
        })
    }
    pub fn finish(
        self,
        message: CredentialFinalization,
        _params: ServerLoginParameters<'_>,
    ) -> Result<ServerLoginFinishResult, ProtocolError> {
        if !bool::from(self.expected_mac.ct_eq(&message.0)) {
            return Err(ProtocolError::InvalidLoginError);
        }
        Ok(ServerLoginFinishResult {
            session_key: self.session,
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/opaque.rs"]
mod tests;
