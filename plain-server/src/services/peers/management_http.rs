use crate::assembly::server::ServerState;
use anyhow::{Result, ensure};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug)]
pub(super) enum AuthorizationError {
    Permission,
    Session,
}

impl std::fmt::Display for AuthorizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Permission => "no_permission",
            Self::Session => "invalid_session",
        })
    }
}

impl std::error::Error for AuthorizationError {}

pub(crate) async fn query(
    state: &ServerState,
    base: &reqwest::Url,
    client_id: &str,
    key: &[u8],
    request: Value,
    limit: usize,
) -> Result<Value> {
    ensure!(
        matches!(base.scheme(), "https" | "http") && base.host_str().is_some(),
        "Invalid management address"
    );
    let encrypted = plain_rs::transport::http_request::encrypt(key, &request.to_string())
        .map_err(|_| anyhow::anyhow!("invalid_session"))?;
    let context = plain_rs::transport::http_response::Context::new("/graphql", &encrypted);
    let response = state
        .lan
        .client
        .post(base.join("/graphql")?)
        .header("c-id", client_id)
        .header("content-type", "application/octet-stream")
        .body(encrypted)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;
    if matches!(response.status().as_u16(), 401 | 403) {
        return Err(AuthorizationError::Session.into());
    }
    ensure!(response.status().is_success(), "invalid_session");
    let bytes = super::peer_lan::limited(response, limit).await?;
    let plain = context
        .decrypt(key, &bytes)
        .ok_or_else(|| anyhow::anyhow!("invalid_session"))?;
    let response: Value = serde_json::from_slice(&plain)?;
    if let Some(errors) = response["errors"].as_array() {
        if !errors.is_empty()
            && errors
                .iter()
                .all(|error| error["message"] == "no_permission")
        {
            return Err(AuthorizationError::Permission.into());
        }
    }
    ensure!(
        response["errors"].is_null() && response["data"].is_object(),
        "Invalid management response"
    );
    Ok(response["data"].clone())
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../tests/unit/services/peers/management_http.rs"]
mod tests;
