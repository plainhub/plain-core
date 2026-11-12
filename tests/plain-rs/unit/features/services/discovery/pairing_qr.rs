use super::*;
use plain_libs::serde_json::{Value, from_slice, from_value, json};

#[test]
fn response_signature_binds_qr_request_sender_and_response() {
    let request: PairingRequest = from_value(json!({
        "toId":"", "targetPublicKey":"", "fromId":"display", "fromName":"Fixture",
        "port":8080, "deviceType":"COMPUTER", "ecdhPublicKey":"ecdh", "signaturePublicKey":"identity",
        "timestamp":1, "signature":"request-signature"
    })).unwrap();
    let response: PairingAccept = from_value(json!({
        "requestId":"request", "port":8080, "deviceType":"COMPUTER", "ecdhPublicKey":"response-ecdh"
    })).unwrap();
    let (key, public) = plain_libs::crypto::ed25519_generate();
    let public = base64_encode(&public);
    let bytes = response_input(&request, &response, "scanner");
    let payload: Value = from_slice(&bytes).unwrap();
    assert_eq!(payload[0], "QR_PAIR_ACCEPT");
    assert_eq!(base64_decode(payload[1].as_str().unwrap()).len(), 32);
    let signature = ed25519_sign(&key, &bytes);
    assert!(ed25519_verify(&public, &bytes, &signature));
    let mut another_request = request.clone();
    another_request.payload.signature = "another-request-signature".into();
    assert!(!ed25519_verify(
        &public,
        &response_input(&another_request, &response, "scanner"),
        &signature
    ));
    assert!(!ed25519_verify(
        &public,
        &response_input(&request, &response, "another-scanner"),
        &signature
    ));
    let mut altered_response = response.clone();
    altered_response.ecdh_public_key.clear();
    assert!(!ed25519_verify(
        &public,
        &response_input(&request, &altered_response, "scanner"),
        &signature
    ));
    assert!(!ed25519_verify(
        &public,
        &base64_decode(payload[1].as_str().unwrap()),
        &signature
    ));
}
