use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS feeds (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    logo TEXT NOT NULL DEFAULT '',
    fetch_full_content INTEGER NOT NULL CHECK(fetch_full_content IN (0,1)),
    last_sync_at TEXT,
    last_error TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS uidx_feeds_url ON feeds(url);
CREATE TABLE IF NOT EXISTS feed_entries (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    image TEXT NOT NULL,
    description TEXT NOT NULL,
    author TEXT NOT NULL,
    content TEXT NOT NULL,
    feed_id TEXT NOT NULL,
    source_entry_id TEXT NOT NULL,
    published_at TEXT NOT NULL,
    is_read INTEGER NOT NULL CHECK(is_read IN (0,1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_feed_entries_feed_id ON feed_entries(feed_id);
CREATE UNIQUE INDEX IF NOT EXISTS uidx_feed_entries_feed_id_source_entry_id ON feed_entries(feed_id, source_entry_id);",
    )
}
