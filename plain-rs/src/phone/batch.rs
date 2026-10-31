use serde::Deserialize;
use std::sync::Arc;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleFacts {
    pub region: String,
    pub locale: String,
    #[serde(default)]
    pub network_region: String,
    #[serde(default)]
    pub sim_region: String,
}

pub async fn lookup_many(
    runtime: Arc<super::Service>,
    numbers: Vec<String>,
    locale: LocaleFacts,
) -> Result<Vec<Option<super::Metadata>>, tokio::task::JoinError> {
    if numbers.is_empty() {
        return Ok(Vec::new());
    }
    tokio::task::spawn_blocking(move || {
        let region = plain_rs::phone::default_region(
            &locale.network_region,
            &locale.sim_region,
            &locale.region,
        );
        numbers
            .iter()
            .map(|number| runtime.lookup(number, region, &locale.locale))
            .collect()
    })
    .await
}

#[cfg(test)]
#[path = "../../tests/unit/phone/batch.rs"]
mod tests;
