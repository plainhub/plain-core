use super::*;

#[test]
fn reports_only_requested_features_and_keeps_grants_independent() {
    assert_eq!(
        facts(
            &[AccessFeature::Files, AccessFeature::Images],
            json!({"features":["IMAGES","AUDIO","FILES"]})
        )
        .unwrap(),
        json!({"granted":{"IMAGES":true,"AUDIO":false,"FILES":true}})
    );
}

#[test]
fn rejects_platform_permissions_and_unknown_features() {
    assert!(facts(&[], json!({"features":["WRITE_EXTERNAL_STORAGE"]})).is_err());
    assert!(facts(&[], json!({"features":["UNKNOWN"]})).is_err());
    assert!(facts(&[], json!({"features":["DOCUMENTS"]})).is_err());
    assert!(facts(&[], json!({"permissions":["FILES"]})).is_err());
}
