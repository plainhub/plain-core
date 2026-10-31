use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS notes (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    deleted_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at DESC);",
    )
}
