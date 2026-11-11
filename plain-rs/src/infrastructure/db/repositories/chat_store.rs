pub mod channels;
pub mod messages;
pub mod nearby;
pub mod peers;
use plain_libs::anyhow::{Result, bail};
use serde::Deserialize;
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SaveMode {
    Insert,
    Update,
    Upsert,
}
fn validate(id: &str, created: &str, updated: &str) -> Result<()> {
    if id.trim().is_empty() {
        bail!("record id required");
    }
    plain_libs::chrono::DateTime::parse_from_rfc3339(created)?;
    plain_libs::chrono::DateTime::parse_from_rfc3339(updated)?;
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/db/repositories/chat_store.rs"]
mod tests;
