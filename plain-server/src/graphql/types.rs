use plain_graphql::SimpleObject;
use plain_rs::infrastructure::types::Instant;

#[derive(SimpleObject, Clone, Debug)]
pub struct ActionResult {
    #[graphql(name = "affectedCount")]
    pub affected_count: i32,
}

pub(crate) fn parse_instant(value: &str) -> plain_graphql::Result<Instant> {
    plain_libs::chrono::DateTime::parse_from_rfc3339(value)
        .map(|dt| Instant(dt.with_timezone(&plain_libs::chrono::Utc)))
        .map_err(|e| plain_graphql::Error::new(format!("invalid stored timestamp: {e}")))
}

pub(crate) fn instant_from_millis(value: i64) -> plain_graphql::Result<Instant> {
    plain_libs::chrono::DateTime::from_timestamp_millis(value)
        .map(Instant)
        .ok_or_else(|| plain_graphql::Error::new("timestamp out of range"))
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
