use plain_libs::rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> plain_libs::rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS shares (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    password TEXT NOT NULL,
    url_token BLOB CHECK(url_token IS NULL OR (typeof(url_token)='blob' AND length(url_token)=32)),
    expires_at TEXT,
    read_only INTEGER NOT NULL CHECK(read_only IN (0,1)),
    data TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);",
    )
}
