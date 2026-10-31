use async_graphql::{ID, Result, SimpleObject};
use plain_rs::db::ClipboardRow;
use plain_rs::types::Instant;

#[derive(SimpleObject)]
pub struct ClipboardItem {
    id: ID,
    text: String,
    /// Client id of the sync origin: empty = captured locally on this device, otherwise received from that peer.
    source: String,
    label: String,
    sensitive: bool,
    created_at: Instant,
}
impl TryFrom<ClipboardRow> for ClipboardItem {
    type Error = async_graphql::Error;
    fn try_from(row: ClipboardRow) -> Result<Self> {
        Ok(Self {
            id: ID(row.id),
            text: row.text,
            source: row.source,
            label: row.label,
            sensitive: row.sensitive,
            created_at: Instant(
                chrono::DateTime::parse_from_rfc3339(&row.created_at)?.with_timezone(&chrono::Utc),
            ),
        })
    }
}
