#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaPlayHistoryRow {
    pub file_id: String,
    pub media_type: i32,
    pub position_ms: i64,
    pub play_count: i64,
    pub played_at: String,
}
