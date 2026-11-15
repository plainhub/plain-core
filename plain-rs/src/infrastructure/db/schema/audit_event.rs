use plain_libs::rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> plain_libs::rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS events (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    type TEXT NOT NULL,
    data TEXT NOT NULL CHECK(json_valid(data)),
    client_id TEXT,
    client_name TEXT,
    client_ip TEXT,
    os_name TEXT,
    os_version TEXT,
    app_name TEXT,
    app_version TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_created_at_id ON events(created_at DESC,id DESC);",
    )
}
