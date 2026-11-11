use super::*;

#[test]
fn encrypts_the_existing_replay_envelope_with_fresh_nonces() {
    let token = plain_libs::crypto::gen_token();
    let key = plain_libs::utils::base64::base64_decode(&token);
    let first = encrypt(&key, "/graphql", r#"{"query":"{ app { deviceName } }"}"#).unwrap();
    let second = encrypt(&key, "/graphql", r#"{"query":"{ app { deviceName } }"}"#).unwrap();
    assert_ne!(first, second);
    let plain = String::from_utf8(decrypt_raw(&key, "POST", "/graphql", &first).unwrap()).unwrap();
    let parts = plain.splitn(3, '|').collect::<Vec<_>>();
    assert!(parts[0].parse::<i64>().unwrap() > 0);
    assert!(plain_libs::utils::uuid::Uuid::parse_str(parts[1]).is_ok());
    assert_eq!(parts[2], r#"{"query":"{ app { deviceName } }"}"#);
}

#[test]
fn response_authentication_binds_key_and_request() {
    let token = plain_libs::crypto::gen_token();
    let key = plain_libs::utils::base64::base64_decode(&token);
    let first = encrypt(&key, "/graphql", "first").unwrap();
    let second = encrypt(&key, "/graphql", "second").unwrap();
    let response = super::super::http_response::Context::new(&first)
        .encrypt(&key, b"response")
        .unwrap();
    assert_eq!(
        decrypt_response(&key, &first, &response).unwrap(),
        b"response"
    );
    assert!(decrypt_response(&key, &second, &response).is_err());
    assert!(decrypt_response(&[8; 32], &first, &response).is_err());
    assert!(encrypt(b"invalid", "/graphql", "payload").is_err());
}

#[test]
fn request_binding_rejects_other_methods_routes_and_unbound_ciphertexts() {
    let body = encrypt(&[7; 32], "/graphql", "payload").unwrap();
    assert!(decrypt_raw(&[7; 32], "POST", "/graphql", &body).is_some());
    assert!(decrypt_raw(&[7; 32], "GET", "/graphql", &body).is_none());
    assert!(decrypt_raw(&[7; 32], "POST", "/guest_graphql", &body).is_none());
    let old = plain_libs::crypto::xchacha_encrypt_raw(&[7; 32], b"payload").unwrap();
    assert!(decrypt_raw(&[7; 32], "POST", "/graphql", &old).is_none());
    assert_eq!(aad("POST", "/graphql"), b"http-request\0POST\0/graphql");
}
