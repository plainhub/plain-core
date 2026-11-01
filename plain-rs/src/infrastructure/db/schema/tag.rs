use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tags (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    type INTEGER NOT NULL CHECK(type IN (0,1,2,3,4,5,6,7,8,21,22,23,24)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tag_relations (
    tag_id TEXT NOT NULL CHECK(length(tag_id)>0),
    target_key TEXT NOT NULL CHECK(length(target_key)>0),
    type INTEGER NOT NULL CHECK(type IN (0,1,2,3,4,5,6,7,8,21,22,23,24)),
    created_at TEXT NOT NULL,
    PRIMARY KEY (tag_id, target_key, type)
);
CREATE INDEX IF NOT EXISTS idx_tag_relations_target_key ON tag_relations(target_key);",
    )
}
