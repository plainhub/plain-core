use plain_libs::anyhow::{Context, Result, ensure};
use plain_libs::rusqlite::{Connection, OpenFlags};
use std::path::Path;

const IDENTITIES: [&str; 25] = [
    "fc89d71dd5b850f76f04c7e02ac96146",
    "fc89d71dd5b850f76f04c7e02ac96146",
    "10b13d499e4754e9793a1529e493356f",
    "38b4401ab15a8aa4e0b34a0f15ff9d00",
    "e1b4b330f9592e4af5f492996b8c6ae1",
    "e9c004b176ce3b0c2a886737cafa1f73",
    "6f60aad342331620522ef5ee76f49ed2",
    "2a8e4b5b20d64222360977009096b72d",
    "319fbaac8f5f4ea9517eb76a7eece33c",
    "37f608d1e32a2c9624dc551fa0068e63",
    "b76df23edb3b4dcb0633075f2b5f5647",
    "28698c889e5f8c1cf5edd4b72ff5f4ee",
    "3d20f2246b6fa3682f2ff54afa640c94",
    "30a3bf0b1a15071588dd2c1c26186c51",
    "235a8663d6e5000316b51fa9c2abe04e",
    "75c6ce872a8b16371c6370fe97567248",
    "27353c7282a43a64014e5f0b05f6bce6",
    "d3c14e4986b2c7f3306aaed8a5a9963d",
    "d3c14e4986b2c7f3306aaed8a5a9963d",
    "bb86d50664730fa68e4cc1a414b0e839",
    "bb86d50664730fa68e4cc1a414b0e839",
    "bb86d50664730fa68e4cc1a414b0e839",
    "e87b9a48daf38070aefa607dff55723c",
    "32fc60c5bd5285193bd36e644c9f7f8e",
    "31a1d1104a4c276fe9b33234e1e4d016",
];

const OPTIONAL_TABLES: &[(&str, usize)] = &[
    ("pomodoro_items", 5),
    ("peers", 6),
    ("bookmarks", 7),
    ("bookmark_groups", 7),
    ("files", 8),
    ("archived_conversations", 14),
    ("shares", 23),
    ("clipboard", 25),
];

fn open(path: &Path, flags: OpenFlags) -> Result<(Connection, usize)> {
    let connection = Connection::open_with_flags(path, flags)?;
    let version = usize::try_from(
        connection.pragma_query_value(None, "user_version", |row| row.get::<_, i32>(0))?,
    )?;
    let known_identity = version
        .checked_sub(1)
        .and_then(|index| IDENTITIES.get(index))
        .with_context(|| format!("Legacy database schema {version} cannot be upgraded"))?;
    let identity: String = connection.query_row(
        "SELECT identity_hash FROM room_master_table WHERE id=42",
        [],
        |row| row.get(0),
    )?;
    ensure!(
        identity == *known_identity,
        "Legacy database has an unexpected schema"
    );
    Ok((connection, version))
}

pub(super) fn read(path: &Path) -> Result<Connection> {
    let (db, version) = open(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    for &(table, introduced) in OPTIONAL_TABLES {
        if version < introduced {
            db.execute_batch(&format!("CREATE TEMP VIEW {table} AS SELECT 1 WHERE 0"))?;
        }
    }
    if version < 6 {
        db.execute_batch("CREATE TEMP VIEW chats AS SELECT id, \
            CASE is_me WHEN 1 THEN 'me' ELSE 'local' END AS from_id, \
            CASE is_me WHEN 1 THEN 'local' ELSE 'me' END AS to_id, \
            '' AS channel_id, 'sent' AS status, '' AS status_data, content, created_at, updated_at FROM main.chats")?;
        db.execute_batch("CREATE TEMP VIEW chat_channels AS SELECT 1 WHERE 0")?;
    } else if version < 12 {
        let channel = if version < 11 {
            "group_id"
        } else {
            "channel_id"
        };
        db.execute_batch(&format!(
            "CREATE TEMP VIEW chats AS SELECT id,from_id,to_id,{channel} AS channel_id, \
            status,'' AS status_data,content,created_at,updated_at FROM main.chats"
        ))?;
        let source = if version < 9 {
            "chat_groups"
        } else {
            "chat_channels"
        };
        let owner_and_version = if version < 10 {
            "'' AS owner,0 AS version"
        } else {
            "owner,version"
        };
        let status = if version < 11 {
            "'joined' AS status"
        } else {
            "status"
        };
        db.execute_batch(&format!(
            "CREATE TEMP VIEW chat_channels AS SELECT id,name,key,members, \
            {owner_and_version},{status},created_at,updated_at FROM main.{source}"
        ))?;
    }
    if version < 24 {
        db.execute_batch(
            "CREATE TEMP VIEW feeds AS SELECT id,name,url,fetch_content,NULL AS last_sync_at, \
            '' AS last_error,created_at,updated_at FROM main.feeds",
        )?;
    }
    Ok(db)
}

pub(super) fn clear(path: &Path) -> Result<()> {
    let (mut connection, version) = open(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let tx = connection.transaction()?;
    tx.execute_batch(
        "DELETE FROM chats; DELETE FROM notes; DELETE FROM feed_entries; DELETE FROM feeds; \
        DELETE FROM tag_relations WHERE type<>9; DELETE FROM tags WHERE type<>9;",
    )?;
    if version >= 6 {
        tx.execute_batch(if version < 9 {
            "DELETE FROM chat_groups"
        } else {
            "DELETE FROM chat_channels"
        })?;
    }
    for &(table, introduced) in OPTIONAL_TABLES {
        if version >= introduced {
            tx.execute_batch(&format!("DELETE FROM {table}"))?;
        }
    }
    tx.commit()?;
    connection.execute_batch("VACUUM;")?;
    Ok(())
}
