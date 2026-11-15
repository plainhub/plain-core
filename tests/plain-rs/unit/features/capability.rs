use super::*;

#[test]
fn every_capability_has_one_stable_wire_value() {
    let mut names = std::collections::HashSet::new();
    for capability in Capability::ALL {
        let name = capability.as_str();
        assert!(names.insert(name));
        assert_eq!(Capability::parse(name), Some(*capability));
        assert_eq!(plain_libs::serde_json::to_value(capability).unwrap(), name);
        assert_eq!(
            plain_libs::serde_json::from_value::<Capability>(plain_libs::serde_json::json!(name))
                .unwrap(),
            *capability
        );
    }
    assert_eq!(Capability::parse("UNKNOWN"), None);
}

#[test]
fn wire_lists_ignore_unknown_capabilities_without_changing_known_values() {
    let capabilities = deserialize_list(plain_libs::serde_json::json!([
        "HTTP_SERVER",
        "FUTURE",
        "MEDIA_SCAN"
    ]))
    .unwrap();
    assert_eq!(
        capabilities,
        vec![Capability::HttpServer, Capability::MediaScan]
    );
    assert!(deserialize_list(plain_libs::serde_json::json!({})).is_err());
}
