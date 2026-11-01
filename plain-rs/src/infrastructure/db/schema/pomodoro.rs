use rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS pomodoro_items (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    date TEXT NOT NULL UNIQUE,
    completed_count INTEGER NOT NULL CHECK(completed_count>=0),
    total_work_secs INTEGER NOT NULL CHECK(total_work_secs>=0),
    total_break_secs INTEGER NOT NULL CHECK(total_break_secs>=0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);",
    )
}
