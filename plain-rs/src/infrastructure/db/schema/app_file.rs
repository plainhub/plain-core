use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_files (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    size_bytes INTEGER NOT NULL CHECK(size_bytes>=0),
    mime_type TEXT NOT NULL,
    real_path TEXT NOT NULL,
    ref_count INTEGER NOT NULL CHECK(ref_count>=0),
    weak_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_app_files_size_bytes_weak_hash ON app_files(size_bytes, weak_hash);",
    )
}
