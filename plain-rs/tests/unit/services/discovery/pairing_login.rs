use super::*;
use serde_json::json;

#[test]
fn management_proof_binds_identity_access_key_and_freshness() {
    let (private, public) = crypto::ed25519_generate();
    let public = base64_encode(&public);
    let mut response = json!({"clientId":"peer","status":"COMPLETED","ecdhPublicKey":"ephemeral","timestamp":1000,"chatPaired":true});
    response["signature"] = json!(crypto::ed25519_sign(
        &private,
        b"peer|COMPLETED|ephemeral|1000|true"
    ));
    assert_eq!(
        verify_response(&response, "peer", &public, 1000).unwrap(),
        "peer"
    );
    assert!(verify_response(&response, "other", &public, 1000).is_err());
    assert!(verify_response(&response, "peer", &public, 301001).is_err());
    assert!(verify_response(&response, "peer", &base64_encode(&[3; 32]), 1000).is_err());
    response["chatPaired"] = json!(false);
    assert!(verify_response(&response, "peer", &public, 1000).is_err());
    assert!(check_identity("peer", "known", "peer", "replacement").is_err());
}
