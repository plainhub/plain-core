use super::*;

fn sample_request() -> PairingRequest {
    PairingRequest {
        to_id: "receiver".into(),
        target_public_key: plain_libs::utils::base64::base64_encode(&[1u8; 32]),
        from_id: "client-a".to_string(),
        from_ip: String::new(),
        payload: PairingRequestPayload {
            from_name: "Mac".to_string(),
            port: 8443,
            device_type: crate::features::chat::enums::DeviceType::Computer,
            ecdh_public_key: "ECDH".to_string(),
            signature_public_key: "SIGPK".to_string(),
            timestamp: 1_700_000_000_000,
            ips: vec!["203.0.113.10".to_string(), "203.0.113.11".to_string()],
            signature: String::new(),
            aware_supported: false,
        },
    }
}

#[test]
fn request_signature_data_format_is_locked() {
    let req = sample_request();
    assert_eq!(
        req.signature_data(),
        r#"["pair-request","client-a","SIGPK","receiver","AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=","Mac",8443,"COMPUTER","ECDH",1700000000000,["203.0.113.10","203.0.113.11"],false]"#
    );
    // `from_ip` is stamped by the receiver and must not affect the signature.
    let mut stamped = req.clone();
    stamped.from_ip = "203.0.113.99".to_string();
    assert_eq!(stamped.signature_data(), req.signature_data());
    // Sender-owned capability claims are authenticated.
    let mut aware = req.clone();
    aware.payload.aware_supported = true;
    assert_ne!(aware.signature_data(), req.signature_data());
}

#[test]
fn response_signature_data_format_is_locked() {
    let resp = PairingResponse {
        request_signature: "REQUEST_SIGNATURE".into(),
        port: 2443,
        device_type: crate::features::chat::enums::DeviceType::Phone,
        ecdh_public_key: "ECDH2".to_string(),
        accepted: true,
        ips: vec![],
        signature: String::new(),
        aware_supported: false,
    };
    assert_eq!(
        resp.signature_data(),
        r#"["pair-response","REQUEST_SIGNATURE",2443,"PHONE","ECDH2",true,[],false]"#
    );
}

/// camelCase JSON wire names must match the Kotlin @Serializable classes.
#[test]
fn wire_json_is_camelcase() {
    let json = serde_json::to_value(sample_request()).unwrap();
    assert_eq!(json["fromId"], "client-a");
    assert_eq!(json["fromName"], "Mac");
    assert_eq!(json["deviceType"], "COMPUTER");
    assert_eq!(json["ecdhPublicKey"], "ECDH");
    assert_eq!(json["signaturePublicKey"], "SIGPK");
    assert_eq!(json["awareSupported"], false);
    assert_eq!(json["fromIp"], "");
    let back: PairingRequest = serde_json::from_value(json).unwrap();
    assert_eq!(back.from_id, "client-a");
}

#[test]
fn legacy_unsigned_cancellation_and_unbound_response_are_rejected() {
    assert!(
        serde_json::from_value::<PairingCancel>(serde_json::json!({"fromId":"a","toId":"b"}))
            .is_err()
    );
    assert!(serde_json::from_value::<PairingResponse>(serde_json::json!({"fromId":"b","toId":"a","port":2443,"deviceType":"PHONE","ecdhPublicKey":"key","signaturePublicKey":"signer","accepted":true,"timestamp":1,"signature":"old"})).is_err());
}

#[test]
fn request_device_enums_preserve_wire_and_signature_values() {
    use crate::features::chat::enums::DeviceType;
    for kind in [
        DeviceType::Computer,
        DeviceType::Phone,
        DeviceType::Tablet,
        DeviceType::Tv,
        DeviceType::Nas,
        DeviceType::Other,
        DeviceType::Unknown,
    ] {
        let mut request = sample_request();
        request.payload.device_type = kind;
        let wire = serde_json::to_value(&request).unwrap();
        assert_eq!(wire["deviceType"], kind.as_str());
        let decoded: PairingRequest = serde_json::from_value(wire).unwrap();
        assert_eq!(decoded.payload.device_type, kind);
        assert_eq!(decoded.signature_data(), request.signature_data());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&request.signature_data()).unwrap()[7],
            kind.as_str()
        );
    }
    let mut wire = serde_json::to_value(sample_request()).unwrap();
    wire["deviceType"] = serde_json::json!("FUTURE_DEVICE");
    assert!(serde_json::from_value::<PairingRequest>(wire).is_err());
}

#[test]
fn delimiters_cannot_move_between_signed_fields() {
    let mut a = sample_request();
    a.from_id = "a|b".into();
    a.payload.from_name = "c".into();
    let mut b = a.clone();
    b.from_id = "a".into();
    b.payload.from_name = "b|c".into();
    assert_ne!(a.signature_data(), b.signature_data());
}
