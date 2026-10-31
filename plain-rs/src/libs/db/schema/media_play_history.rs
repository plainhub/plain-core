use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS media_play_history (
    file_id TEXT NOT NULL PRIMARY KEY CHECK(length(file_id)>0),
    media_type INTEGER NOT NULL CHECK(media_type IN (1,2)),
    position_ms INTEGER NOT NULL CHECK(position_ms>=0),
    play_count INTEGER NOT NULL CHECK(play_count>=0),
    played_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_media_play_history_media_type_played_at ON media_play_history(media_type,played_at);",
    )
}
