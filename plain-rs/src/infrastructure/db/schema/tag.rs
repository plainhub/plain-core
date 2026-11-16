use plain_libs::rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> plain_libs::rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tags (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('DEFAULT','AUDIO','VIDEO','IMAGE','SMS','CONTACT','NOTE','FEED_ENTRY','CALL','PACKAGE','FILE','APP_FILE','DOC')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tag_relations (
    tag_id TEXT NOT NULL CHECK(length(tag_id)>0),
    target_key TEXT NOT NULL CHECK(length(target_key)>0),
    type TEXT NOT NULL CHECK(type IN ('DEFAULT','AUDIO','VIDEO','IMAGE','SMS','CONTACT','NOTE','FEED_ENTRY','CALL','PACKAGE','FILE','APP_FILE','DOC')),
    created_at TEXT NOT NULL,
    PRIMARY KEY (tag_id, target_key, type)
);
CREATE INDEX IF NOT EXISTS idx_tag_relations_target_key ON tag_relations(target_key);",
    )
}
