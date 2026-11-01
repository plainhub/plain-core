use super::*;

#[test]
fn short_uuid_is_non_empty_and_base36() {
    let id = short_uuid();
    assert!(!id.is_empty());
    assert!(
        id.chars()
            .all(|c| c.is_ascii_digit() || (c.is_ascii_lowercase()))
    );
}

#[test]
fn short_uuid_is_unique() {
    let a = short_uuid();
    let b = short_uuid();
    assert_ne!(a, b);
}

#[test]
fn encode_base36_zero() {
    assert_eq!(encode_base36(0), "0");
}

#[test]
fn encode_base36_matches_java() {
    // Sanity check: 36 -> "10" (matches Long.toString(36, 36) -> "10").
    assert_eq!(encode_base36(36), "10");
    assert_eq!(encode_base36(35), "z");
}
