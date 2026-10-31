use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS image_editor_projects (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    state_b64 TEXT NOT NULL,
    thumbnail TEXT,
    canvas_width INTEGER NOT NULL CHECK(canvas_width>0),
    canvas_height INTEGER NOT NULL CHECK(canvas_height>0),
    layer_count INTEGER NOT NULL CHECK(layer_count>=0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_image_editor_projects_updated_at ON image_editor_projects(updated_at DESC);",
    )
}
