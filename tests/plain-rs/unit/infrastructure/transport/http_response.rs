use super::*;

#[test]
fn noble_vectors_match_the_shared_response_context_and_wire_bytes() {
    use chacha20poly1305::{
        XChaCha20Poly1305, XNonce,
        aead::{Aead, KeyInit, Payload},
    };
    let vectors: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(include_str!(
        "../../../../../plain-rs/testdata/crypto/http-response-vectors.json"
    ))
    .unwrap();
    for vector in vectors.as_array().unwrap() {
        let bytes =
            |field: &str| plain_libs::utils::base64::base64_decode(vector[field].as_str().unwrap());
        let key = bytes("key");
        let request = bytes("request");
        let response = bytes("response");
        let plaintext = vector["plaintext"].as_str().unwrap().as_bytes();
        let context = Context::new(&request);
        assert_eq!(
            plain_libs::utils::hex::bytes_to_hex(&context.0),
            vector["aadHex"].as_str().unwrap()
        );
        assert_eq!(context.decrypt(&key, &response).unwrap(), plaintext);
        assert!(context.decrypt(&key, &request).is_none());
        let cipher = XChaCha20Poly1305::new_from_slice(&key).unwrap();
        assert_eq!(
            cipher
                .encrypt(
                    XNonce::from_slice(&response[..24]),
                    Payload {
                        msg: plaintext,
                        aad: &context.0
                    }
                )
                .unwrap(),
            response[24..]
        );
    }
}

#[test]
fn responses_require_the_exact_request_key_and_response_purpose() {
    let key = [7; 32];
    let request = super::super::http_request::encrypt(&key, "/graphql", "first").unwrap();
    let context = Context::new(&request);
    assert_eq!(&context.0[..2], b"r\0");
    assert_eq!(&context.0[2..], &Sha256::digest(&request)[..]);
    let response = context.encrypt(&key, br#"{"data":{"ok":true}}"#).unwrap();
    assert_eq!(
        context.decrypt(&key, &response).unwrap(),
        br#"{"data":{"ok":true}}"#
    );
    for position in [0, 24, request.len() - 1] {
        let mut other_request = request.clone();
        other_request[position] ^= 1;
        assert!(
            Context::new(&other_request)
                .decrypt(&key, &response)
                .is_none()
        );
    }
    for endpoint in ["/graphql", "/guest_graphql", "/peer_graphql"] {
        let other_request = super::super::http_request::encrypt(&key, endpoint, "first").unwrap();
        assert!(
            Context::new(&other_request)
                .decrypt(&key, &response)
                .is_none()
        );
    }
    assert!(context.decrypt(&key, &request).is_none());
    assert!(context.decrypt(&[8; 32], &response).is_none());
    assert!(plain_libs::crypto::xchacha_decrypt_raw(&key, &response).is_none());
    let mut old_aad = b"plain-http-response-v1\0/graphql\0".to_vec();
    old_aad.extend_from_slice(&Sha256::digest(&request));
    for aad in [
        &[][..],
        old_aad.as_slice(),
        super::super::http_request::aad("POST", "/graphql").as_slice(),
    ] {
        let other =
            plain_libs::crypto::xchacha_encrypt_raw_with_aad(&key, br#"{"data":{}}"#, aad).unwrap();
        assert!(context.decrypt(&key, &other).is_none());
    }
    let mut tampered = response.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert!(context.decrypt(&key, &tampered).is_none());
    for length in 0..response.len() {
        assert!(context.decrypt(&key, &response[..length]).is_none());
    }
}
