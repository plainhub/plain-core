use super::*;

#[test]
fn pbkdf2_sha256_matches_the_fixed_reference() {
    let salt: [u8; 16] = std::array::from_fn(|index| index as u8);
    let hash = pbkdf2::pbkdf2_hmac_array::<Sha256, 32>(b"fixture-digest", &salt, ITERATIONS);
    assert_eq!(
        crate::utils::hex::bytes_to_hex(&hash),
        "f8d62b15649f08dc593114e12040486bbd38cab782367a85ff6e93d3570d6912"
    );
}

#[test]
fn salted_records_verify_only_the_original_secret_and_roundtrip() {
    let first = PasswordHash::new(b"fixture-digest");
    let second = PasswordHash::new(b"fixture-digest");
    assert_ne!(first.salt, second.salt);
    assert_ne!(first.hash, second.hash);
    assert!(first.verify(b"fixture-digest"));
    assert!(!first.verify(b"wrong-digest"));
    let encoded = crate::serde_json::to_string(&first).unwrap();
    assert!(!encoded.contains("fixture-digest"));
    assert_eq!(
        crate::serde_json::from_str::<PasswordHash>(&encoded).unwrap(),
        first
    );
    let mut altered = first;
    altered.iterations = u32::MAX;
    assert!(!altered.verify(b"fixture-digest"));
}
