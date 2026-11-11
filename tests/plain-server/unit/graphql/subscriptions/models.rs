use super::*;
use plain_libs::serde_json::json;

#[test]
fn image_update_events_preserve_all_statuses_and_reject_unknown_statuses() {
    use plain_rs::features::services::images::status::ImageSearchStatusType;
    let source = |status| json!({"version":7,"status":status,"downloadProgress":50,"errorMessage":"","modelSize":12,"modelDir":"models","isIndexing":false,"totalImages":3,"indexedImages":1});
    for (wire, expected) in [
        ("UNAVAILABLE", ImageSearchStatusType::Unavailable),
        ("DOWNLOADING", ImageSearchStatusType::Downloading),
        ("LOADING", ImageSearchStatusType::Loading),
        ("READY", ImageSearchStatusType::Ready),
        ("ERROR", ImageSearchStatusType::Error),
    ] {
        let value: ImageModelsUpdate = plain_libs::serde_json::from_value(source(wire)).unwrap();
        assert_eq!(value.status, expected);
        assert_eq!(value.version, Long(7));
        assert_eq!(value.download_progress, 50);
    }
    assert!(plain_libs::serde_json::from_value::<ImageModelsUpdate>(source("FUTURE_STATUS")).is_err());
}

#[test]
fn pairing_notices_preserve_device_enums_and_reject_unknown_values() {
    use plain_rs::features::services::device::types::DeviceType;
    let source = |kind| json!({"toId":"local","targetPublicKey":"local-signing","fromId":"peer","fromName":"Fixture","port":8443,"deviceType":kind,"ecdhPublicKey":"ecdh","signaturePublicKey":"signing","timestamp":5000000000_i64,"ips":["127.0.0.1"],"signature":"signature","fromIp":"127.0.0.1","awareSupported":false,"verificationCode":"123456789","identityChanged":false});
    for expected in [
        DeviceType::Computer,
        DeviceType::Phone,
        DeviceType::Tablet,
        DeviceType::Tv,
        DeviceType::Nas,
        DeviceType::Other,
    ] {
        let value: PairingRequestNotice =
            plain_libs::serde_json::from_value(source(expected.as_str())).unwrap();
        assert_eq!(value.to_id.as_str(), "local");
        assert_eq!(value.target_public_key, "local-signing");
        assert!(!value.identity_changed);
        assert_eq!(value.device_type, expected);
        assert_eq!(value.timestamp, Long(5000000000));
        assert_eq!(value.signature, "signature");
    }
    assert!(plain_libs::serde_json::from_value::<PairingRequestNotice>(source("FUTURE_DEVICE")).is_err());
}

#[test]
fn internal_unknown_devices_project_to_the_public_other_variant() {
    use plain_rs::features::services::device::types::DeviceType;
    assert_eq!(
        plain_libs::serde_json::from_value::<DeviceType>(json!("UNKNOWN")).unwrap(),
        DeviceType::Other
    );
}
