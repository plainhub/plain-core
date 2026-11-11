use plain_libs::rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> plain_libs::rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS scan_history (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id)>0),
    text TEXT NOT NULL
);",
    )
}
