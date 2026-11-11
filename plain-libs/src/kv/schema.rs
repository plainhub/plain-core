use crate::rusqlite::Connection;

pub fn init(connection: &Connection) -> crate::rusqlite::Result<()> {
    connection.execute_batch("CREATE TABLE IF NOT EXISTS key_values (key TEXT PRIMARY KEY NOT NULL,value TEXT NOT NULL) STRICT, WITHOUT ROWID;")
}
