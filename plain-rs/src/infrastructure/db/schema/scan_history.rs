use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS scan_history (
    id INTEGER PRIMARY KEY,
    text TEXT NOT NULL
);",
    )
}
