use super::*;
use ed25519_dalek::SigningKey;
use serde_json::Value;

fn vectors() -> Value {
    serde_json::from_str(include_str!("../../fixtures/crypto-vectors.json")).unwrap()
}

fn bytes(value: &Value) -> Vec<u8> {
    base64_decode(value.as_str().unwrap())
}

#[test]
fn p256_keys_and_shared_secret_match_android_vectors() {
    let vectors = vectors();
    let vector = &vectors["ecdh"];
    let a = EcdhSession::from_private_key(&bytes(&vector["privateKeyA"])).unwrap();
    let b = EcdhSession::from_private_key(&bytes(&vector["privateKeyB"])).unwrap();
    let public_a = bytes(&vector["publicKeyA"]);
    let public_b = bytes(&vector["publicKeyB"]);
    assert_eq!(a.public_key_bytes, public_a);
    assert_eq!(b.public_key_bytes, public_b);
    assert_eq!(public_a.len(), 65);
    assert_eq!(public_a[0], 4);
    assert_eq!(
        a.compute_shared_key(&public_b).unwrap().as_slice(),
        bytes(&vector["sharedKey"])
    );
    assert_eq!(
        b.compute_shared_key(&public_a).unwrap().as_slice(),
        bytes(&vector["sharedKey"])
    );
}

#[test]
fn ed25519_seed_public_key_and_signature_match_android_vectors() {
    let vectors = vectors();
    let vector = &vectors["ed25519"];
    let seed: [u8; 32] = bytes(&vector["privateKey"]).try_into().unwrap();
    let keypair = SigningKey::from_bytes(&seed).to_keypair_bytes();
    let public = bytes(&vector["publicKey"]);
    assert_eq!(&keypair[..32], seed);
    assert_eq!(&keypair[32..], public);
    assert_eq!(
        ed25519_public_from_keypair(&keypair).unwrap().as_slice(),
        public
    );
    let message = vector["message"].as_str().unwrap().as_bytes();
    assert_eq!(
        ed25519_sign(&keypair, message),
        vector["signature"].as_str().unwrap()
    );
    assert!(ed25519_verify(
        vector["publicKey"].as_str().unwrap(),
        message,
        vector["signature"].as_str().unwrap()
    ));
}

#[test]
fn xchacha_wire_frame_matches_tink_raw_vectors() {
    let vectors = vectors();
    let vector = &vectors["xchacha20"];
    let key = bytes(&vector["key"]);
    let nonce = bytes(&vector["nonce"]);
    let ciphertext = bytes(&vector["ciphertext"]);
    let plaintext = vector["plaintext"].as_str().unwrap().as_bytes();
    assert_eq!(key.len(), 32);
    assert_eq!(nonce.len(), 24);
    assert_eq!(ciphertext.len(), plaintext.len() + 16);
    let frame = [nonce.as_slice(), ciphertext.as_slice()].concat();
    assert_eq!(xchacha_decrypt_raw(&key, &frame).unwrap(), plaintext);
    use chacha20poly1305::{
        XChaCha20Poly1305, XNonce,
        aead::{Aead, KeyInit},
    };
    let cipher = XChaCha20Poly1305::new_from_slice(&key).unwrap();
    assert_eq!(
        cipher
            .encrypt(XNonce::from_slice(&nonce), plaintext)
            .unwrap(),
        ciphertext
    );
}

#[test]
fn generated_ed25519_keypair_contains_portable_seed_and_public_key() {
    let (keypair, public) = ed25519_generate();
    let seed: [u8; 32] = keypair[..32].try_into().unwrap();
    assert_eq!(
        SigningKey::from_bytes(&seed).verifying_key().to_bytes(),
        public
    );
    assert_eq!(&keypair[32..], public);
}
