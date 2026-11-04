use super::*;

#[test]
fn uuid_prefix_preserves_all_byte_values_and_positions() {
    for position in 0..4 {
        for value in 0..=255 {
            let mut bytes = [0; 4];
            bytes[position] = value;
            let expected = format!(
                "{:02x}{:02x}{:02x}{:02x}",
                bytes[0], bytes[1], bytes[2], bytes[3]
            );
            assert_eq!(format_prefix(&bytes).as_slice(), expected.as_bytes());
        }
    }
}

#[test]
fn short_uuid_preserves_existing_text_prefix_encoding() {
    for (bytes, expected) in [
        ([0, 0, 0, 0], "qdpxnhru4ieo"),
        ([0x12, 0x34, 0x56, 0x78], "qxl0nudotqgo"),
        ([0xff; 4], "1k25m0f9nil46"),
    ] {
        assert_eq!(
            encode_base36(u64::from_be_bytes(format_prefix(&bytes))),
            expected
        );
    }
}

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
