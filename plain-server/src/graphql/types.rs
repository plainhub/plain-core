use async_graphql::SimpleObject;
use plain_rs::types::Instant;

#[derive(SimpleObject, Clone, Debug)]
pub struct ActionResult {
    #[graphql(name = "affectedCount")]
    pub affected_count: i32,
}

pub(crate) fn parse_instant(value: &str) -> async_graphql::Result<Instant> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|dt| Instant(dt.with_timezone(&chrono::Utc)))
        .map_err(|e| async_graphql::Error::new(format!("invalid stored timestamp: {e}")))
}

/// The `app` root echoes a write back rather than reading the store again,
/// so a caller can confirm what it asked for without a round trip.
#[derive(SimpleObject, Clone, Debug)]
pub struct KeyValuePair {
    pub key: String,
    pub value: String,
}

#[cfg(feature = "core")]
pub mod facts;
