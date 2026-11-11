use crate::features::access::AccessFeature;
use crate::features::prefs::{Prefs, pref_keys::API_PERMISSIONS};
use crate::infrastructure::host::bridge::Host;
use serde_json::json;

pub fn is_enabled(prefs: &Prefs, features: &[AccessFeature]) -> bool {
    let configured: Vec<AccessFeature> = prefs.get_or(API_PERMISSIONS, Vec::new());
    features
        .iter()
        .all(|name| configured.iter().any(|value| value == name))
}

pub async fn sample(host: &Host, features: &[AccessFeature]) -> Result<Vec<AccessFeature>, String> {
    let sample = host
        .call("systemAccessFacts", json!({"features": features}))
        .await?;
    #[derive(serde::Deserialize)]
    struct Facts {
        granted: std::collections::BTreeMap<AccessFeature, bool>,
    }
    let facts: Facts = serde_json::from_value(sample).map_err(|error| error.to_string())?;
    let mut granted = Vec::new();
    for feature in features {
        match facts.granted.get(feature) {
            Some(true) => granted.push(*feature),
            Some(false) => {}
            None => return Err(format!("Missing access sample: {}", feature.as_str())),
        }
    }
    Ok(granted)
}

pub async fn granted(host: &Host, features: &[AccessFeature]) -> plain_libs::anyhow::Result<bool> {
    Ok(sample(host, features)
        .await
        .map_err(plain_libs::anyhow::Error::msg)?
        .len()
        == features.len())
}

pub async fn snapshot(host: &Host) -> Result<Vec<AccessFeature>, String> {
    let capabilities = crate::features::services::capabilities::snapshot(host)
        .await
        .map_err(|error| error.to_string())?;
    snapshot_with_capabilities(host, &capabilities).await
}

pub async fn snapshot_with_capabilities(
    host: &Host,
    capabilities: &[String],
) -> Result<Vec<AccessFeature>, String> {
    let features: Vec<_> = AccessFeature::ALL
        .into_iter()
        .filter(|feature| {
            feature
                .capability()
                .is_none_or(|capability| capabilities.iter().any(|value| value == capability))
        })
        .collect();
    sample(host, &features).await
}

pub fn enabled_features(prefs: &Prefs, grants: &[AccessFeature]) -> Result<Vec<AccessFeature>, String> {
    let configured: Vec<AccessFeature> = prefs
        .get(API_PERMISSIONS)
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    Ok(grants
        .iter()
        .filter(|name| configured.contains(name))
        .copied()
        .collect())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/device/access.rs"]
mod tests;
