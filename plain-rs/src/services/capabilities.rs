use anyhow::{Result, anyhow, bail};
use plain_rs::host::bridge::Host;

pub async fn snapshot(host: &Host) -> Result<Vec<String>> {
    serde_json::from_value(
        host.call("systemCapabilities", serde_json::json!({}))
            .await
            .map_err(anyhow::Error::msg)?,
    )
    .map_err(|error| anyhow!("invalid platform capabilities: {error}"))
}

pub fn require_list(capabilities: &[String], capability: &str) -> Result<()> {
    if capabilities.iter().any(|value| value == capability) {
        return Ok(());
    }
    bail!("unsupported capability {capability}")
}

pub async fn require(host: &Host, capability: &str) -> Result<()> {
    require_list(&snapshot(host).await?, capability)
}

#[cfg(test)]
#[path = "../../tests/unit/services/capabilities.rs"]
mod tests;
