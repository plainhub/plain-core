use plain_libs::rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> plain_libs::rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
    client_id TEXT NOT NULL PRIMARY KEY CHECK(length(client_id)>0),
    name TEXT NOT NULL DEFAULT '',
    type TEXT NOT NULL DEFAULT 'LOGIN' CHECK(type IN ('LOGIN','API_TOKEN')),
    client_ip TEXT NOT NULL,
    os_name TEXT NOT NULL,
    os_version TEXT NOT NULL,
    app_name TEXT NOT NULL,
    app_version TEXT NOT NULL,
    token BLOB NOT NULL CHECK(typeof(token)='blob' AND length(token)=32),
    url_token BLOB NOT NULL CHECK(typeof(url_token)='blob' AND length(url_token)=32),
    last_seen_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);",
    )
}
