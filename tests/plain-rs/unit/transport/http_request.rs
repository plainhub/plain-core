use super::*;

#[test]
fn encrypts_the_existing_replay_envelope_with_fresh_nonces() {
    let token = crate::crypto::gen_token();
    let key = crate::utils::base64::base64_decode(&token);
    let first = encrypt(&key, r#"{"query":"{ app { deviceName } }"}"#).unwrap();
    let second = encrypt(&key, r#"{"query":"{ app { deviceName } }"}"#).unwrap();
    assert_ne!(first, second);
    let plain =
        String::from_utf8(crate::crypto::xchacha_decrypt_raw(&key, &first).unwrap()).unwrap();
    let parts = plain.splitn(3, '|').collect::<Vec<_>>();
    assert!(parts[0].parse::<i64>().unwrap() > 0);
    assert!(uuid::Uuid::parse_str(parts[1]).is_ok());
    assert_eq!(parts[2], r#"{"query":"{ app { deviceName } }"}"#);
}

#[test]
fn response_authentication_binds_key_endpoint_and_request() {
    let token = crate::crypto::gen_token();
    let key = crate::utils::base64::base64_decode(&token);
    let first = encrypt(&key, "first").unwrap();
    let second = encrypt(&key, "second").unwrap();
    let response = super::super::http_response::Context::new("/graphql", &first)
        .encrypt(&key, b"response")
        .unwrap();
    assert_eq!(
        decrypt_response(&key, "/graphql", &first, &response).unwrap(),
        b"response"
    );
    assert!(decrypt_response(&key, "/graphql", &second, &response).is_err());
    assert!(decrypt_response(&key, "/other", &first, &response).is_err());
    assert!(decrypt_response(&[8; 32], "/graphql", &first, &response).is_err());
    assert!(encrypt(b"invalid", "payload").is_err());
}
