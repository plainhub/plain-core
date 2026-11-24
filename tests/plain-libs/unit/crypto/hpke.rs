use super::*;
use crate::utils::hex::bytes_to_hex;

fn private_key(hex: &str) -> [u8; 32] {
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}

#[test]
fn hpke_base_matches_rfc9180_a_2_1() {
    let ephemeral = private_key("f4ec9b33b792c372c1d2c2063507b684ef925b8c75a42dbcbf57d63ccd381600");
    let recipient = private_key("8057991eef8f1f1af18f4a9491d16a1ce333f695d4db8e38da75975c4478e0fb");
    let recipient_public = public_key(&recipient);
    assert_eq!(
        bytes_to_hex(&recipient_public),
        "4310ee97d88cc1f088a5576c77ab0cf5c3ac797f3d95139c6c84b5429c59662a"
    );
    let info = b"Ode on a Grecian Urn";
    let (enc, encryption) = setup_sender_with_key(&recipient_public, &ephemeral, info).unwrap();
    assert_eq!(
        bytes_to_hex(&enc),
        "1afa08d3dec047a643885163f1180476fa7ddb54c6a8029ea33f95796bf2ac4a"
    );
    let decryption = setup_recipient(&recipient, &enc, info).unwrap();
    assert_eq!(
        bytes_to_hex(encryption.export(b"").as_ref()),
        "4bbd6243b8bb54cec311fac9df81841b6fd61f56538a775e7c80a9f40160606e"
    );
    assert_eq!(
        encryption.export(b"response").as_ref(),
        decryption.export(b"response").as_ref()
    );
    let plaintext = b"Beauty is truth, truth beauty";
    let ciphertext = encryption.seal(b"Count-0", plaintext).unwrap();
    assert_eq!(
        bytes_to_hex(&ciphertext),
        "1c5250d8034ec2b784ba2cfd69dbdb8af406cfe3ff938e131f0def8c8b60b4db21993c62ce81883d2dd1b51a28"
    );
    assert_eq!(decryption.open(b"Count-0", &ciphertext).unwrap(), plaintext);
}

#[test]
fn hpke_base_rejects_wrong_recipient_context_and_tampering() {
    let key = [53; 32];
    let peer = public_key(&key);
    let (enc, ctx) = setup_sender(&peer, b"channel message").unwrap();
    let ciphertext = ctx.seal(b"context", b"synthetic text").unwrap();
    let (other_enc, _) = setup_sender(&peer, b"channel message").unwrap();
    assert_ne!(enc, other_enc);
    assert!(
        setup_recipient(&[75; 32], &enc, b"channel message")
            .unwrap()
            .open(b"context", &ciphertext)
            .is_none()
    );
    assert!(
        setup_recipient(&key, &enc, b"notification")
            .unwrap()
            .open(b"context", &ciphertext)
            .is_none()
    );
    assert!(
        setup_recipient(&key, &enc, b"channel message")
            .unwrap()
            .open(b"other", &ciphertext)
            .is_none()
    );
    let mut modified = ciphertext;
    modified[0] ^= 1;
    assert!(
        setup_recipient(&key, &enc, b"channel message")
            .unwrap()
            .open(b"context", &modified)
            .is_none()
    );
    assert!(setup_sender(&[0; 32], b"").is_none());
    assert!(setup_recipient(&key, &[0; 32], b"").is_none());
}
