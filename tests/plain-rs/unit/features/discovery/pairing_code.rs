use super::*;
use plain_libs::{
    crypto::{EcdhSession, ed25519_generate, ed25519_sign},
    utils::base64::base64_encode,
};

#[test]
fn qr_carries_a_signed_request_and_rejects_legacy_or_modified_codes() {
    let (key, public) = ed25519_generate();
    let mut request = PairingRequest {
        from_id: "id+&value".into(),
        from_ip: String::new(),
        to_id: String::new(),
        target_public_key: String::new(),
        payload: PairingRequestPayload {
            from_name: "Device α + &? /".into(),
            device_type: crate::features::chat::enums::DeviceType::Phone,
            ips: vec!["192.0.2.4".into(), "2001:db8::1".into()],
            port: 8443,
            signature_public_key: base64_encode(&public),
            ecdh_public_key: base64_encode(&EcdhSession::generate().public_key_bytes),
            timestamp: crate::features::peers::pairing::now_ms(),
            signature: String::new(),
            aware_supported: false,
        },
    };
    request.payload.signature = ed25519_sign(&key, request.signature_data().as_bytes());
    let code = format(&request).unwrap();
    let parsed = parse(&code).unwrap().into_request();
    assert_eq!(request.signature_data(), parsed.signature_data());
    assert_eq!(request.payload.signature, parsed.payload.signature);
    assert_eq!(
        parse(&code).unwrap().into_device().device_type,
        request.payload.device_type
    );
    assert!(parse("plainapp://pair?v=2&id=fixture&name=Device&ips=192.0.2.1&port=8443").is_err());
    assert!(parse(&code.replace("8443", "9443")).is_err());
    request.payload.timestamp -= 300_001;
    request.payload.signature = ed25519_sign(&key, request.signature_data().as_bytes());
    assert!(parse(&format(&request).unwrap()).is_err());
}
