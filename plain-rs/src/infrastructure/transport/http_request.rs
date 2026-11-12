use plain_libs::aad::AadPurpose;

/// Request binding uses the logical destination across HTTP, proxy, BLE and Aware.
pub fn aad(method: &str, endpoint: &str) -> Vec<u8> {
    let mut aad = AadPurpose::HttpRequest.as_str().as_bytes().to_vec();
    aad.push(0);
    aad.extend_from_slice(method.as_bytes());
    aad.push(0);
    aad.extend_from_slice(endpoint.as_bytes());
    aad
}
pub fn encrypt_raw(key: &[u8], method: &str, endpoint: &str, payload: &[u8]) -> Option<Vec<u8>> {
    if matches!(endpoint, "/graphql" | "/guest_graphql" | "/peer_graphql") && method != "POST" {
        return None;
    }
    plain_libs::crypto::xchacha_encrypt_raw_with_aad(key, payload, &aad(method, endpoint))
}
pub fn decrypt_raw(key: &[u8], method: &str, endpoint: &str, payload: &[u8]) -> Option<Vec<u8>> {
    if matches!(endpoint, "/graphql" | "/guest_graphql" | "/peer_graphql") && method != "POST" {
        return None;
    }
    plain_libs::crypto::xchacha_decrypt_raw_with_aad(key, payload, &aad(method, endpoint))
}
pub fn encrypt(key: &[u8], endpoint: &str, payload: &str) -> Result<Vec<u8>, &'static str> {
    let body = if matches!(endpoint, "/graphql" | "/guest_graphql") {
        super::graphql_request::Envelope::new(payload)?.json()?
    } else {
        format!(
            "{}|{}|{}",
            plain_libs::chrono::Utc::now().timestamp_millis(),
            plain_libs::utils::uuid::Uuid::new_v4(),
            payload
        )
    };
    encrypt_raw(key, "POST", endpoint, body.as_bytes()).ok_or("Invalid management key")
}

pub fn decrypt_response(
    key: &[u8],
    request: &[u8],
    response: &[u8],
) -> Result<Vec<u8>, &'static str> {
    super::http_response::Context::new(request)
        .decrypt(key, response)
        .ok_or("Invalid encrypted HTTP response")
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/transport/http_request.rs"]
mod tests;
