use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS scan_history (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id)>0),
    text TEXT NOT NULL
);",
    )
}
