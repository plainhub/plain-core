pub fn encrypt(key: &[u8], payload: &str) -> Result<Vec<u8>, &'static str> {
    let body = format!(
        "{}|{}|{}",
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4(),
        payload
    );
    crate::crypto::xchacha_encrypt_raw(key, body.as_bytes()).ok_or("Invalid management key")
}

pub fn decrypt_response(
    key: &[u8],
    endpoint: &str,
    request: &[u8],
    response: &[u8],
) -> Result<Vec<u8>, &'static str> {
    super::http_response::Context::new(endpoint, request)
        .decrypt(key, response)
        .ok_or("Invalid encrypted HTTP response")
}

#[cfg(test)]
#[path = "../../tests/unit/transport/http_request.rs"]
mod tests;
