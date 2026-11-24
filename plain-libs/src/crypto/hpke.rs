use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use curve25519_dalek::montgomery::MontgomeryPoint;
use hkdf::Hkdf;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

// RFC 9180 Base with DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, and ChaCha20-Poly1305.
const KEM_SUITE: &[u8] = b"KEM\x00\x20";
const HPKE_SUITE: &[u8] = b"HPKE\x00\x20\x00\x01\x00\x03";

fn labeled_extract(salt: &[u8], suite: &[u8], label: &[u8], ikm: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut input = Zeroizing::new(Vec::with_capacity(
        7 + suite.len() + label.len() + ikm.len(),
    ));
    input.extend_from_slice(b"HPKE-v1");
    input.extend_from_slice(suite);
    input.extend_from_slice(label);
    input.extend_from_slice(ikm);
    let (result, _) = Hkdf::<Sha256>::extract(Some(salt), &input);
    Zeroizing::new(result.into())
}

fn labeled_expand<const N: usize>(
    prk: &[u8; 32],
    suite: &[u8],
    label: &[u8],
    info: &[u8],
) -> [u8; N] {
    let mut input = Vec::with_capacity(9 + suite.len() + label.len() + info.len());
    input.extend_from_slice(&u16::try_from(N).expect("HPKE output length").to_be_bytes());
    input.extend_from_slice(b"HPKE-v1");
    input.extend_from_slice(suite);
    input.extend_from_slice(label);
    input.extend_from_slice(info);
    let mut result = [0_u8; N];
    Hkdf::<Sha256>::from_prk(prk)
        .expect("32-byte HKDF PRK")
        .expand(&input, &mut result)
        .expect("HPKE output fits HKDF");
    result
}

pub fn public_key(secret: &[u8; 32]) -> [u8; 32] {
    MontgomeryPoint::mul_base_clamped(*secret).to_bytes()
}

fn shared_secret(secret: &[u8; 32], peer: &[u8; 32]) -> Option<Zeroizing<[u8; 32]>> {
    let dh = Zeroizing::new(MontgomeryPoint(*peer).mul_clamped(*secret).to_bytes());
    (!bool::from(dh.as_ref().ct_eq(&[0; 32]))).then_some(dh)
}

pub fn valid_public_key(peer: &[u8; 32]) -> bool {
    shared_secret(&[0; 32], peer).is_some()
}

fn kem_secret(
    secret: &[u8; 32],
    peer: &[u8; 32],
    enc: &[u8; 32],
    recipient: &[u8; 32],
) -> Option<Zeroizing<[u8; 32]>> {
    let dh = shared_secret(secret, peer)?;
    let prk = labeled_extract(&[], KEM_SUITE, b"eae_prk", dh.as_ref());
    let mut context = [0_u8; 64];
    context[..32].copy_from_slice(enc);
    context[32..].copy_from_slice(recipient);
    Some(Zeroizing::new(labeled_expand(
        &prk,
        KEM_SUITE,
        b"shared_secret",
        &context,
    )))
}

pub struct BaseContext {
    key: Zeroizing<[u8; 32]>,
    nonce: [u8; 12],
    exporter_secret: Zeroizing<[u8; 32]>,
}

impl BaseContext {
    fn new(secret: &[u8; 32], info: &[u8]) -> Self {
        let psk_id_hash = labeled_extract(&[], HPKE_SUITE, b"psk_id_hash", &[]);
        let info_hash = labeled_extract(&[], HPKE_SUITE, b"info_hash", info);
        let mut context = [0_u8; 65];
        context[1..33].copy_from_slice(psk_id_hash.as_ref());
        context[33..].copy_from_slice(info_hash.as_ref());
        let secret = labeled_extract(secret, HPKE_SUITE, b"secret", &[]);
        Self {
            key: Zeroizing::new(labeled_expand(&secret, HPKE_SUITE, b"key", &context)),
            nonce: labeled_expand(&secret, HPKE_SUITE, b"base_nonce", &context),
            exporter_secret: Zeroizing::new(labeled_expand(&secret, HPKE_SUITE, b"exp", &context)),
        }
    }

    pub fn export(&self, info: &[u8]) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(labeled_expand(
            &self.exporter_secret,
            HPKE_SUITE,
            b"sec",
            info,
        ))
    }

    pub fn seal(self, aad: &[u8], plaintext: &[u8]) -> Option<Vec<u8>> {
        let cipher = ChaCha20Poly1305::new((&*self.key).into());
        cipher
            .encrypt(
                Nonce::from_slice(&self.nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .ok()
    }

    pub fn open(self, aad: &[u8], ciphertext: &[u8]) -> Option<Vec<u8>> {
        let cipher = ChaCha20Poly1305::new((&*self.key).into());
        cipher
            .decrypt(
                Nonce::from_slice(&self.nonce),
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .ok()
    }
}

pub fn setup_sender(recipient: &[u8; 32], info: &[u8]) -> Option<([u8; 32], BaseContext)> {
    let mut ephemeral = Zeroizing::new([0_u8; 32]);
    super::gen_random(ephemeral.as_mut());
    setup_sender_with_key(recipient, &ephemeral, info)
}

fn setup_sender_with_key(
    recipient: &[u8; 32],
    ephemeral: &[u8; 32],
    info: &[u8],
) -> Option<([u8; 32], BaseContext)> {
    let enc = public_key(ephemeral);
    let secret = kem_secret(ephemeral, recipient, &enc, recipient)?;
    Some((enc, BaseContext::new(&secret, info)))
}

pub fn setup_recipient(secret: &[u8; 32], enc: &[u8; 32], info: &[u8]) -> Option<BaseContext> {
    let recipient = public_key(secret);
    let secret = kem_secret(secret, enc, enc, &recipient)?;
    Some(BaseContext::new(&secret, info))
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/crypto/hpke.rs"]
mod tests;
