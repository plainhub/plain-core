use plain_libs::rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> plain_libs::rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS events (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    type TEXT NOT NULL,
    message TEXT NOT NULL,
    client_id TEXT,
    client_name TEXT NOT NULL,
    client_ip TEXT NOT NULL,
    os_name TEXT NOT NULL,
    os_version TEXT NOT NULL,
    app_name TEXT NOT NULL,
    app_version TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_created_at_id ON events(created_at DESC,id DESC);",
    )
}
