use super::*;
use p256::pkcs8::EncodePublicKey;

#[test]
fn generated_keys_round_trip_and_agree_in_sec1_and_spki_formats() {
    let a = EcdhSession::generate();
    let b = EcdhSession::generate();
    let a_private = a.private_key_bytes();
    let b_private = b.private_key_bytes();
    assert_eq!(a_private.len(), 32);
    assert_ne!(a_private, b_private);
    assert_eq!(a.public_key_bytes.len(), 65);
    assert_eq!(a.public_key_bytes[0], 4);
    let restored = EcdhSession::from_private_key(&a_private).unwrap();
    assert_eq!(restored.public_key_bytes, a.public_key_bytes);
    let public = P256PublicKey::from_sec1_bytes(&b.public_key_bytes).unwrap();
    let der = public.to_public_key_der().unwrap();
    let expected = a.compute_shared_key(&b.public_key_bytes).unwrap();
    assert_eq!(restored.compute_shared_key(der.as_bytes()), Some(expected));
    let a_public = EcdhSession::from_private_key(&a_private)
        .unwrap()
        .public_key_bytes;
    assert_eq!(b.compute_shared_key(&a_public), Some(expected));
}

#[test]
fn invalid_private_scalars_and_malformed_public_keys_are_rejected() {
    for key in [vec![], vec![1; 31], vec![1; 33], vec![0; 32], vec![255; 32]] {
        assert!(EcdhSession::from_private_key(&key).is_none());
    }
    let mut private = [0; 32];
    private[31] = 1;
    for public in [
        vec![],
        vec![0],
        vec![4; 64],
        vec![4; 66],
        vec![0; 65],
        vec![4; 65],
    ] {
        assert!(
            EcdhSession::from_private_key(&private)
                .unwrap()
                .compute_shared_key(&public)
                .is_none()
        );
    }
}
