use rusqlite::Connection;
pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS replay_receipts (
        receipt BLOB PRIMARY KEY NOT NULL,
        expires_at INTEGER NOT NULL,
        admitted_at INTEGER NOT NULL
    ) STRICT, WITHOUT ROWID;
    CREATE INDEX IF NOT EXISTS replay_receipts_expiry ON replay_receipts(expires_at);
    CREATE INDEX IF NOT EXISTS replay_receipts_clock ON replay_receipts(admitted_at);",
    )
}
