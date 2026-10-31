use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
    client_id TEXT NOT NULL PRIMARY KEY CHECK(length(client_id)>0),
    name TEXT NOT NULL DEFAULT '',
    type TEXT NOT NULL DEFAULT 'WEB' CHECK(type IN ('WEB','CUSTOM')),
    client_ip TEXT NOT NULL,
    os_name TEXT NOT NULL,
    os_version TEXT NOT NULL,
    browser_name TEXT NOT NULL,
    browser_version TEXT NOT NULL,
    token TEXT NOT NULL,
    url_token TEXT NOT NULL,
    last_seen_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);",
    )
}
