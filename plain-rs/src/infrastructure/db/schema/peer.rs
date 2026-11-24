use plain_libs::rusqlite::Connection;

pub(crate) fn init(conn: &Connection) -> plain_libs::rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS peers (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    ip TEXT NOT NULL,
    chat_token BLOB CHECK(chat_token IS NULL OR (typeof(chat_token)='blob' AND length(chat_token)=32)),
    signature_public_key BLOB CHECK(signature_public_key IS NULL OR (typeof(signature_public_key)='blob' AND length(signature_public_key)=32)),
    encryption_public_key BLOB CHECK(encryption_public_key IS NULL OR (typeof(encryption_public_key)='blob' AND length(encryption_public_key)=32)),
    status TEXT NOT NULL CHECK(status IN ('PAIRED','UNPAIRED','CHANNEL')),
    port INTEGER CHECK(port BETWEEN 1 AND 65535),
    device_type TEXT NOT NULL CHECK(device_type IN ('PHONE','TABLET','COMPUTER','TV','NAS','OTHER','UNKNOWN')),
    manage_token BLOB CHECK(manage_token IS NULL OR (typeof(manage_token)='blob' AND length(manage_token)=32)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);",
    )
}
