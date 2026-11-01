use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS trashed_sms (
    message_id TEXT NOT NULL CHECK(length(message_id)>0),
    is_mms INTEGER NOT NULL CHECK(is_mms IN (0,1)),
    trashed_at TEXT NOT NULL,
    PRIMARY KEY(message_id, is_mms)
);",
    )
}
