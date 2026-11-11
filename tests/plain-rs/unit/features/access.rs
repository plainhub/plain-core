use super::AccessFeature;

#[test]
fn access_features_have_exact_wire_names() {
    let expected = [
        "IMAGES",
        "VIDEOS",
        "AUDIO",
        "DOCS",
        "FILES",
        "CONTACTS",
        "SMS",
        "CALLS",
        "CALL_PHONE",
        "PHONE_NUMBER",
        "APPS",
        "NOTIFICATIONS",
        "CLIPBOARD",
        "ADB",
    ];
    for (feature, name) in AccessFeature::ALL.into_iter().zip(expected) {
        assert_eq!(feature.as_str(), name);
        assert_eq!(AccessFeature::parse(name), Some(feature));
        assert_eq!(serde_json::to_value(feature).unwrap(), name);
        assert_eq!(
            serde_json::from_value::<AccessFeature>(serde_json::json!(name)).unwrap(),
            feature
        );
    }
    assert_eq!(AccessFeature::Calls.capability(), Some("TELEPHONY"));
    for name in ["CALL_LOGS", "calls", "UNKNOWN", ""] {
        assert_eq!(AccessFeature::parse(name), None);
        assert!(serde_json::from_value::<AccessFeature>(serde_json::json!(name)).is_err());
    }
}
