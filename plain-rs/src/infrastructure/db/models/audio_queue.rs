pub const HISTORY_KEEP: usize = 200;
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QueueItem {
    #[serde(rename = "media_id")]
    pub file_id: String,
    pub path: String,
    pub sort_order: i64,
    pub title: String,
    pub artist: String,
    pub duration_ms: i64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlaylistItem {
    #[serde(rename = "media_id")]
    pub file_id: String,
    pub id: String,
    pub playlist_id: String,
    pub audio_path: String,
    pub title: String,
    pub artist: String,
    pub duration_ms: i64,
    pub album_id: String,
    pub sort_order: i64,
    pub added_at: String,
}
