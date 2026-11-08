use super::*;

#[test]
fn all_previously_accepted_forms_produce_the_same_value_and_json() {
    let canonical = "936da01f-9abd-4d9d-80c7-02af85c822a8";
    for input in [
        canonical,
        "936DA01F9ABD4D9D80C702AF85C822A8",
        "{936da01f-9abd-4d9d-80c7-02af85c822a8}",
        "urn:uuid:936da01f-9abd-4d9d-80c7-02af85c822a8",
    ] {
        let parsed = Uuid::parse_str(input).unwrap();
        assert_eq!(parsed.to_string(), canonical);
        assert_eq!(
            serde_json::to_string(&parsed).unwrap(),
            format!("\"{canonical}\"")
        );
        assert_eq!(
            serde_json::from_str::<Uuid>(&format!("\"{input}\"")).unwrap(),
            parsed
        );
    }
}

#[test]
fn malformed_identifiers_keep_existing_diagnostics() {
    for (input, message) in [
        ("", "invalid length: found 0"),
        ("00-é", "invalid character: found `é` at 3"),
        ("936da01f-9abd", "invalid group count: expected 5, found 2"),
        (
            "{936da01f-9abd-4d9d-80c7-02af85c822aZ}",
            "invalid character: found `Z` at 35",
        ),
        (
            "936da01f-9ab-4d9d-80c7-02af85c822a8",
            "invalid group length in group 1: expected 4, found 3",
        ),
    ] {
        assert_eq!(Uuid::parse_str(input).unwrap_err().to_string(), message);
    }
}

#[test]
fn v4_masks_only_the_version_and_variant_bits() {
    let expected = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x4f, 0xff, 0xbf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff,
    ];
    assert_eq!(Uuid::from_random_bytes([0xff; 16]).as_bytes(), &expected);
    let random = Uuid::new_v4();
    assert_eq!(random.as_bytes()[6] >> 4, 4);
    assert_eq!(random.as_bytes()[8] & 0xc0, 0x80);
}

#[test]
fn display_encodes_every_byte_in_each_position() {
    for position in 0..16 {
        for byte in 0..=255u8 {
            let mut bytes = [0; 16];
            bytes[position] = byte;
            let mut expected = String::new();
            for (index, byte) in bytes.iter().enumerate() {
                use std::fmt::Write;
                if matches!(index, 4 | 6 | 8 | 10) {
                    expected.push('-');
                }
                write!(&mut expected, "{byte:02x}").unwrap();
            }
            assert_eq!(Uuid::from_bytes(bytes).to_string(), expected);
        }
    }
}
