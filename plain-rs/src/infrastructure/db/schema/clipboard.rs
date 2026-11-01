use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS clipboards (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    text TEXT NOT NULL,
    hash TEXT NOT NULL,
    source TEXT NOT NULL,
    label TEXT NOT NULL,
    sensitive INTEGER NOT NULL CHECK(sensitive IN (0,1)),
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_clipboards_hash ON clipboards(hash);",
    )
}
