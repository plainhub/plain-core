use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS shares (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    password TEXT NOT NULL,
    url_token TEXT NOT NULL,
    expires_at TEXT,
    read_only INTEGER NOT NULL CHECK(read_only IN (0,1)),
    data TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);",
    )
}
