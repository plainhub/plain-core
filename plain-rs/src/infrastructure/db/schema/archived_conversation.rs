use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS archived_sms_conversations (
    conversation_id TEXT NOT NULL PRIMARY KEY CHECK(length(conversation_id)>0),
    archived_at TEXT NOT NULL
);",
    )
}
