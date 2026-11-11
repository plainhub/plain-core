use crate::features::access::AccessFeature;
use serde::Deserialize;
use plain_libs::serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccessRequest {
    features: Vec<crate::features::access::AccessFeature>,
}

pub fn facts(granted: &[AccessFeature], params: Value) -> Result<Value, String> {
    let request: AccessRequest =
        plain_libs::serde_json::from_value(params).map_err(|error| error.to_string())?;
    let values: plain_libs::serde_json::Map<String, Value> = request
        .features
        .into_iter()
        .map(|feature| {
            let name = feature.as_str();
            (name.to_owned(), Value::Bool(granted.contains(&feature)))
        })
        .collect();
    Ok(json!({"granted": values}))
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/host/access_facts.rs"]
mod tests;
