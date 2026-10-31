use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS bookmarks (
    id TEXT NOT NULL CHECK(length(id)>0),
    url TEXT NOT NULL,
    title TEXT NOT NULL,
    favicon_path TEXT NOT NULL,
    group_id TEXT,
    pinned INTEGER NOT NULL CHECK(pinned IN (0,1)),
    click_count INTEGER NOT NULL CHECK(click_count>=0),
    last_clicked_at TEXT,
    sort_order INTEGER NOT NULL CHECK(sort_order>=0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(id)
);
CREATE INDEX IF NOT EXISTS idx_bookmarks_group_id ON bookmarks(group_id);
CREATE TABLE IF NOT EXISTS bookmark_groups (
    id TEXT NOT NULL CHECK(length(id)>0),
    name TEXT NOT NULL,
    collapsed INTEGER NOT NULL CHECK(collapsed IN (0,1)),
    sort_order INTEGER NOT NULL CHECK(sort_order>=0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(id)
);",
    )
}
