use crate::infrastructure::error_code::ErrorCode;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use plain_libs::protocols::http_client;
use plain_libs::serde_json::Value;
use std::time::Duration;

pub async fn query(
    state: &CoreRuntime,
    base: &http_client::Url,
    client_id: &str,
    key: &[u8],
    request: Value,
    limit: usize,
) -> Result<Value> {
    ensure!(
        matches!(base.scheme(), "https" | "http") && base.host_str().is_some(),
        "Invalid management address"
    );
    let outgoing = state
        .lan
        .client
        .post(base.join("/graphql")?)
        .header("c-id", client_id)
        .timeout(Duration::from_secs(10));
    let response = exchange(outgoing, key, &request.to_string(), limit).await?;
    let response: Value = plain_libs::serde_json::from_slice(&response)?;
    if let Some(errors) = response["errors"].as_array() {
        if !errors.is_empty()
            && errors
                .iter()
                .all(|error| error["extensions"]["code"] == ErrorCode::NoPermission.as_str())
        {
            return Err(ErrorCode::RemoteAccessDisabled.into());
        }
    }
    ensure!(
        response["errors"].is_null() && response["data"].is_object(),
        "Invalid management response"
    );
    Ok(response["data"].clone())
}

pub async fn exchange(
    outgoing: http_client::RequestBuilder,
    key: &[u8],
    request: &str,
    limit: usize,
) -> Result<Vec<u8>> {
    let encrypted =
        crate::infrastructure::transport::http_request::encrypt(key, "/graphql", request)
            .map_err(|_| plain_libs::anyhow::anyhow!(ErrorCode::InvalidSession))?;
    let context = crate::infrastructure::transport::http_response::Context::new(&encrypted);
    let response = outgoing
        .header("content-type", "application/octet-stream")
        .body(encrypted)
        .send()
        .await?;
    match response.status().as_u16() {
        401 => return Err(ErrorCode::Unauthorized.into()),
        403 => return Err(ErrorCode::RemoteAccessDisabled.into()),
        _ => {}
    }
    ensure!(response.status().is_success(), ErrorCode::InvalidSession);
    let bytes = crate::features::services::peers::peer_lan::limited(response, limit).await?;
    context
        .decrypt(key, &bytes)
        .ok_or_else(|| plain_libs::anyhow::anyhow!(ErrorCode::InvalidSession))
}
