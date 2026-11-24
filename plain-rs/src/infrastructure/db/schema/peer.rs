use plain_libs::rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> plain_libs::rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS peers (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    ip TEXT NOT NULL,
    key BLOB CHECK(key IS NULL OR (typeof(key)='blob' AND length(key)=32)),
    public_key BLOB CHECK(public_key IS NULL OR (typeof(public_key)='blob' AND length(public_key)=32)),
    status TEXT NOT NULL CHECK(status IN ('PAIRED','UNPAIRED','CHANNEL')),
    port INTEGER CHECK(port BETWEEN 1 AND 65535),
    device_type TEXT NOT NULL CHECK(device_type IN ('PHONE','TABLET','COMPUTER','TV','NAS','OTHER','UNKNOWN')),
    token BLOB CHECK(token IS NULL OR (typeof(token)='blob' AND length(token)=32)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);",
    )
}
