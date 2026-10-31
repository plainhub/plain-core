use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS nearby_devices (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    ips TEXT NOT NULL,
    port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
    device_type TEXT NOT NULL CHECK(device_type IN ('PHONE','TABLET','COMPUTER','TV','NAS','OTHER')),
    version TEXT NOT NULL,
    platform TEXT NOT NULL,
    last_seen_at TEXT NOT NULL
);",
    )
}
