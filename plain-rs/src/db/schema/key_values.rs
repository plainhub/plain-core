use rusqlite::Connection;

pub(crate) fn init(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch("CREATE TABLE IF NOT EXISTS key_values (key TEXT PRIMARY KEY NOT NULL,value TEXT NOT NULL) STRICT, WITHOUT ROWID;")
}
