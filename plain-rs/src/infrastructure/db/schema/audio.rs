use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS audio_queue_items (
    file_id TEXT NOT NULL PRIMARY KEY CHECK(length(file_id)>0),
    sort_order INTEGER NOT NULL CHECK(sort_order>=0)
);
CREATE INDEX IF NOT EXISTS idx_audio_queue_items_sort_order ON audio_queue_items(sort_order);
CREATE TABLE IF NOT EXISTS audio_playlists (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS audio_playlist_items (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    playlist_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    sort_order INTEGER NOT NULL CHECK(sort_order>=0),
    added_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS uidx_audio_playlist_items_playlist_id_file_id ON audio_playlist_items(playlist_id, file_id);
CREATE INDEX IF NOT EXISTS idx_audio_playlist_items_playlist_id_sort_order ON audio_playlist_items(playlist_id, sort_order);",
    )
}
