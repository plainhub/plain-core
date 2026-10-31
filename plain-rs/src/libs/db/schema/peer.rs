use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS peers (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    ip TEXT NOT NULL,
    key TEXT NOT NULL,
    public_key TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('PAIRED','UNPAIRED','CHANNEL')),
    port INTEGER CHECK(port BETWEEN 1 AND 65535),
    device_type TEXT NOT NULL CHECK(device_type IN ('PHONE','TABLET','COMPUTER','TV','NAS','OTHER','UNKNOWN')),
    token TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);",
    )
}
