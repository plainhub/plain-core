use super::*;

#[test]
fn encrypts_graphql_json_with_fresh_uuid_v7_request_ids() {
    let token = plain_libs::crypto::gen_token();
    let key = plain_libs::utils::base64::base64_decode(&token);
    let first = encrypt(&key, "/graphql", r#"{"query":"{ app { deviceName } }"}"#).unwrap();
    let second = encrypt(&key, "/graphql", r#"{"query":"{ app { deviceName } }"}"#).unwrap();
    assert_ne!(first, second);
    let plain = String::from_utf8(decrypt_raw(&key, "POST", "/graphql", &first).unwrap()).unwrap();
    let request: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&plain).unwrap();
    let id =
        plain_libs::utils::uuid::Uuid::parse_str(request["requestId"].as_str().unwrap()).unwrap();
    assert_eq!(id.as_bytes()[6] >> 4, 7);
    assert_eq!(id.as_bytes()[8] & 0xc0, 0x80);
    assert_eq!(request["query"], "{ app { deviceName } }");
    let other: plain_libs::serde_json::Value = plain_libs::serde_json::from_slice(
        &decrypt_raw(&key, "POST", "/graphql", &second).unwrap(),
    )
    .unwrap();
    assert_ne!(request["requestId"], other["requestId"]);
}

#[test]
fn response_authentication_binds_key_and_request() {
    let token = plain_libs::crypto::gen_token();
    let key = plain_libs::utils::base64::base64_decode(&token);
    let first = encrypt(&key, "/graphql", r#"{"query":"{ first }"}"#).unwrap();
    let second = encrypt(&key, "/graphql", r#"{"query":"{ second }"}"#).unwrap();
    let response = super::super::http_response::Context::new(&first)
        .encrypt(&key, b"response")
        .unwrap();
    assert_eq!(
        decrypt_response(&key, &first, &response).unwrap(),
        b"response"
    );
    assert!(decrypt_response(&key, &second, &response).is_err());
    assert!(decrypt_response(&[8; 32], &first, &response).is_err());
    assert!(encrypt(b"invalid", "/graphql", r#"{"query":"{ payload }"}"#).is_err());
}

#[test]
fn graphql_requests_use_empty_aad_and_only_post() {
    let body = encrypt(&[7; 32], "/graphql", r#"{"query":"{ payload }"}"#).unwrap();
    assert!(decrypt_raw(&[7; 32], "POST", "/graphql", &body).is_some());
    assert!(decrypt_raw(&[7; 32], "GET", "/graphql", &body).is_none());
    assert!(plain_libs::crypto::xchacha_decrypt_raw(&[7; 32], &body).is_some());
    let old = plain_libs::crypto::xchacha_encrypt_raw_with_aad(
        &[7; 32],
        b"payload",
        &aad("POST", "/graphql"),
    )
    .unwrap();
    assert!(decrypt_raw(&[7; 32], "POST", "/graphql", &old).is_none());
}

#[test]
fn main_and_guest_share_the_uuid_json_envelope() {
    let body = encrypt(&[7; 32], "/init", "init").unwrap();
    let plain = String::from_utf8(decrypt_raw(&[7; 32], "POST", "/init", &body).unwrap()).unwrap();
    assert_eq!(plain.splitn(3, '|').nth(2), Some("init"));
    for endpoint in ["/graphql", "/guest_graphql"] {
        for invalid in ["not json", "[]", "null"] {
            assert!(encrypt(&[7; 32], endpoint, invalid).is_err());
        }
        let body = encrypt(&[7; 32], endpoint, r#"{"requestId":"caller-value","query":"query Q($text: String!) { q(text:$text) }","variables":{"text":"a|b"},"operationName":"Q"}"#).unwrap();
        let request: plain_libs::serde_json::Value = plain_libs::serde_json::from_slice(
            &decrypt_raw(&[7; 32], "POST", endpoint, &body).unwrap(),
        )
        .unwrap();
        assert_ne!(request["requestId"], "caller-value");
        assert_eq!(request["variables"]["text"], "a|b");
        assert_eq!(request["operationName"], "Q");
    }
}

#[test]
fn graphql_crypto_carries_only_the_payload_without_route_metadata() {
    let key = [7; 32];
    for endpoint in ["/graphql", "/guest_graphql", "/peer_graphql"] {
        let body = encrypt_raw(&key, "POST", endpoint, b"payload").unwrap();
        assert_eq!(
            plain_libs::crypto::xchacha_decrypt_raw(&key, &body).unwrap(),
            b"payload"
        );
        for other in ["/graphql", "/guest_graphql", "/peer_graphql"] {
            assert_eq!(decrypt_raw(&key, "POST", other, &body).unwrap(), b"payload");
        }
        assert!(encrypt_raw(&key, "GET", endpoint, b"payload").is_none());
        assert!(decrypt_raw(&key, "GET", endpoint, &body).is_none());
    }
}
