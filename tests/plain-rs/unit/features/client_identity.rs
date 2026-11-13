use super::*;

#[test]
fn generated_ids_match_the_discovery_mask_and_keep_random_device_bits() {
    let mask = Uuid::parse_str(DISCOVERY_UUID_MASK).unwrap().as_u128();
    let prefix = Uuid::parse_str(DISCOVERY_UUID).unwrap().as_u128();
    let mut ids = std::collections::HashSet::new();
    for _ in 0..256 {
        let value = generate();
        let id = parse(&value).unwrap();
        assert_eq!(id.as_u128() & mask, prefix);
        assert_eq!(id.to_string(), value);
        assert!(ids.insert(id));
    }
}

#[test]
fn parser_rejects_other_prefixes_versions_variants_and_legacy_ids() {
    for value in [
        "706c6169-2d74-8e53-9a16-f08b62c591d3",
        "706C6169-2D74-8E53-9A16-F08B62C591D3",
    ] {
        assert!(parse(value).is_some());
    }
    for value in [
        "706c6168-2d74-8e53-9a16-f08b62c591d3",
        "706c6169-2d74-4e53-9a16-f08b62c591d3",
        "706c6169-2d74-8e53-7a16-f08b62c591d3",
        "706c6169-2d74-8e53-ca16-f08b62c591d3",
        "f16d05ec6b29248d",
        "",
        "invalid",
    ] {
        assert!(parse(value).is_none(), "{value}");
    }
}

#[test]
fn public_client_ids_accept_uuid_versions_and_reject_labels() {
    assert!(is_valid("706c6169-27a3-827c-b1aa-35dc51fa8c45"));
    assert!(is_valid("00000000-0000-4000-8000-000000000001"));
    for invalid in ["", "wasm-client", "00000000000040008000000000000001", "not-a-uuid--------------------------"] {
        assert!(!is_valid(invalid));
    }
}
