use super::*;

#[test]
fn noble_vectors_match_the_shared_response_context_and_wire_bytes() {
    use chacha20poly1305::{
        XChaCha20Poly1305, XNonce,
        aead::{Aead, KeyInit, Payload},
    };
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../testdata/crypto/http-response-vectors.json"
    ))
    .unwrap();
    for vector in vectors.as_array().unwrap() {
        let bytes = |field: &str| crate::base64_decode(vector[field].as_str().unwrap());
        let key = bytes("key");
        let request = bytes("request");
        let response = bytes("response");
        let plaintext = vector["plaintext"].as_str().unwrap().as_bytes();
        let context = Context::new(vector["endpoint"].as_str().unwrap(), &request);
        assert_eq!(
            crate::utils::hex::bytes_to_hex(&context.0),
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
fn responses_require_the_exact_request_endpoint_key_and_response_purpose() {
    let key = [7; 32];
    let context = Context::new("/graphql", b"first encrypted request");
    let response = context.encrypt(&key, br#"{"data":{"ok":true}}"#).unwrap();
    assert_eq!(
        context.decrypt(&key, &response).unwrap(),
        br#"{"data":{"ok":true}}"#
    );
    assert!(
        Context::new("/graphql", b"second encrypted request")
            .decrypt(&key, &response)
            .is_none()
    );
    assert!(
        Context::new("/guest_graphql", b"first encrypted request")
            .decrypt(&key, &response)
            .is_none()
    );
    assert!(
        Context::new("/peer_graphql", b"first encrypted request")
            .decrypt(&key, &response)
            .is_none()
    );
    assert!(context.decrypt(&[8; 32], &response).is_none());
    assert!(crate::crypto::xchacha_decrypt_raw(&key, &response).is_none());
    let legacy = crate::crypto::xchacha_encrypt_raw(&key, br#"{"data":{}}"#).unwrap();
    assert!(context.decrypt(&key, &legacy).is_none());
    let mut tampered = response.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert!(context.decrypt(&key, &tampered).is_none());
    for length in 0..response.len() {
        assert!(context.decrypt(&key, &response[..length]).is_none());
    }
}
