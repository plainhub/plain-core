use super::*;

#[test]
fn legacy_ids_round_trip_through_the_reserved_ble_identity_range() {
    for (legacy, expected) in [
        ("0", "706c6169-0001-8000-8000-000000000000"),
        ("9ix", "706c6169-0001-8000-8000-000000003039"),
        ("1y2p0ij32e8e7", "706c6169-0001-8001-bfff-ffffffffffff"),
    ] {
        let id = convert(legacy).unwrap();
        assert_eq!(id, expected);
        assert!(crate::features::client_identity::parse(&id).is_some());
        assert_eq!(reverse(&id).as_deref(), Some(legacy));
    }
    assert!(reverse("706c6169-0001-8002-8000-000000000000").is_none());
}

#[test]
fn legacy_reference_labels_are_distinct_from_numeric_ids() {
    for label in ["", "me", "local"] {
        assert_eq!(reference(label).unwrap(), label);
        assert!(convert(label).is_err());
    }
    for invalid in ["09ix", "9IX", "-1", "+1", "1y2p0ij32e8e8", "9 ix"] {
        assert!(convert(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn existing_uuids_keep_their_identity() {
    let id = "706c6169-0001-8001-bfff-ffffffffffff";
    assert_eq!(convert(id).unwrap(), id);
    assert_eq!(convert(&id.to_uppercase()).unwrap(), id);
}
