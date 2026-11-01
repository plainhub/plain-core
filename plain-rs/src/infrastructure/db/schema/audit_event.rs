use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS audit_events (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    type TEXT NOT NULL,
    message TEXT NOT NULL,
    client_id TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_events_created_at_id ON audit_events(created_at DESC,id DESC);",
    )
}
