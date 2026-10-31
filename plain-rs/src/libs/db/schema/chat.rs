use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chats (
    id TEXT NOT NULL CHECK(length(id)>0),
    from_id TEXT NOT NULL,
    to_id TEXT NOT NULL,
    channel_id TEXT,
    status TEXT NOT NULL DEFAULT 'PENDING' CHECK(status IN ('PENDING','SENT','PARTIAL','FAILED')),
    status_data TEXT,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(id)
);
CREATE INDEX IF NOT EXISTS idx_chats_from_id ON chats(from_id);
CREATE INDEX IF NOT EXISTS idx_chats_to_id ON chats(to_id);
CREATE INDEX IF NOT EXISTS idx_chats_channel_id ON chats(channel_id);
CREATE TABLE IF NOT EXISTS chat_receipts (
    receipt_id TEXT NOT NULL PRIMARY KEY CHECK(length(receipt_id)>0),
    peer_id TEXT NOT NULL,
    sent_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chat_receipts_sent_at ON chat_receipts(sent_at);
CREATE TABLE IF NOT EXISTS chat_channels (
    id TEXT NOT NULL CHECK(length(id)>0),
    name TEXT NOT NULL,
    key TEXT NOT NULL,
    owner_id TEXT,
    members TEXT NOT NULL,
    version INTEGER NOT NULL CHECK(version>=0) DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'JOINED' CHECK(status IN ('JOINED','LEFT','KICKED')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(id)
);",
    )
}
