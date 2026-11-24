use crate::features::capability::Capability;
use crate::infrastructure::host::bridge::Host;
use plain_libs::anyhow::{Result, anyhow, bail};

pub async fn snapshot(host: &Host) -> Result<Vec<Capability>> {
    let mut capabilities = crate::features::capability::deserialize_list(
        host.call("systemCapabilities", plain_libs::serde_json::json!({}))
            .await
            .map_err(plain_libs::anyhow::Error::msg)?,
    )
    .map_err(|error| anyhow!("invalid platform capabilities: {error}"))?;
    include_filesystem_media_trash(host, &mut capabilities);
    Ok(capabilities)
}

pub(super) fn include_filesystem_media_trash(host: &Host, capabilities: &mut Vec<Capability>) {
    if host.startup().filesystem_media_trash && !capabilities.contains(&Capability::MediaTrash) {
        capabilities.push(Capability::MediaTrash);
    }
}

pub fn require_list(capabilities: &[Capability], capability: Capability) -> Result<()> {
    if capabilities.contains(&capability) {
        return Ok(());
    }
    bail!("unsupported capability {capability}")
}

pub async fn require(host: &Host, capability: Capability) -> Result<()> {
    require_list(&snapshot(host).await?, capability)
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/services/capabilities.rs"]
mod tests;
