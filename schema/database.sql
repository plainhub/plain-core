-- plain-core SQLite DDL (SQLite, WAL mode)
-- Generated; do not edit manually. Source: plain-rs/src/db/schema/*.rs
-- Generate: node scripts/gen-db-schema-sql.mjs --write
-- Check: node scripts/gen-db-schema-sql.mjs

PRAGMA user_version = 1;

CREATE TABLE IF NOT EXISTS app_files (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    size_bytes INTEGER NOT NULL CHECK(size_bytes>=0),
    mime_type TEXT NOT NULL,
    real_path TEXT NOT NULL,
    ref_count INTEGER NOT NULL CHECK(ref_count>=0),
    weak_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_app_files_size_bytes_weak_hash ON app_files(size_bytes, weak_hash);

CREATE TABLE IF NOT EXISTS archived_sms_conversations (
    conversation_id TEXT NOT NULL PRIMARY KEY CHECK(length(conversation_id)>0),
    archived_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audio_queue_items (
    file_id TEXT NOT NULL PRIMARY KEY CHECK(length(file_id)>0),
    sort_order INTEGER NOT NULL CHECK(sort_order>=0)
);

CREATE INDEX IF NOT EXISTS idx_audio_queue_items_sort_order ON audio_queue_items(sort_order);

CREATE TABLE IF NOT EXISTS audio_playlists (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audio_playlist_items (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    playlist_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    sort_order INTEGER NOT NULL CHECK(sort_order>=0),
    added_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS uidx_audio_playlist_items_playlist_id_file_id ON audio_playlist_items(playlist_id, file_id);

CREATE INDEX IF NOT EXISTS idx_audio_playlist_items_playlist_id_sort_order ON audio_playlist_items(playlist_id, sort_order);

CREATE TABLE IF NOT EXISTS audit_events (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    type TEXT NOT NULL,
    message TEXT NOT NULL,
    client_id TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_events_created_at_id ON audit_events(created_at DESC,id DESC);

CREATE TABLE IF NOT EXISTS bookmarks (
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
);

CREATE TABLE IF NOT EXISTS chats (
    id TEXT NOT NULL CHECK(length(id)>0),
    from_id TEXT NOT NULL,
    to_id TEXT NOT NULL,
    channel_id TEXT,
    status TEXT NOT NULL DEFAULT 'PENDING' CHECK(status IN ('PENDING','SENT','PARTIAL','FAILED')),
    status_data TEXT,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(id)
);

CREATE INDEX IF NOT EXISTS idx_chats_from_id ON chats(from_id);

CREATE INDEX IF NOT EXISTS idx_chats_to_id ON chats(to_id);

CREATE INDEX IF NOT EXISTS idx_chats_channel_id ON chats(channel_id);

CREATE TABLE IF NOT EXISTS chat_receipts (
    receipt_id TEXT NOT NULL PRIMARY KEY CHECK(length(receipt_id)>0),
    peer_id TEXT NOT NULL,
    sent_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chat_receipts_sent_at ON chat_receipts(sent_at);

CREATE TABLE IF NOT EXISTS chat_channels (
    id TEXT NOT NULL CHECK(length(id)>0),
    name TEXT NOT NULL,
    key TEXT NOT NULL,
    owner_id TEXT,
    members TEXT NOT NULL,
    version INTEGER NOT NULL CHECK(version>=0) DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'JOINED' CHECK(status IN ('JOINED','LEFT','KICKED')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(id)
);

CREATE TABLE IF NOT EXISTS clipboards (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    text TEXT NOT NULL,
    hash TEXT NOT NULL,
    source TEXT NOT NULL,
    label TEXT NOT NULL,
    sensitive INTEGER NOT NULL CHECK(sensitive IN (0,1)),
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_clipboards_hash ON clipboards(hash);

CREATE TABLE IF NOT EXISTS favorite_folders (
    root_path     TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    alias         TEXT,
    PRIMARY KEY (root_path, relative_path)
);

CREATE TABLE IF NOT EXISTS feeds (
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

CREATE UNIQUE INDEX IF NOT EXISTS uidx_feed_entries_feed_id_source_entry_id ON feed_entries(feed_id, source_entry_id);

CREATE TABLE IF NOT EXISTS volumes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    identity TEXT NOT NULL UNIQUE CHECK(length(identity)>0),
    path TEXT NOT NULL CHECK(length(path)>0),
    is_available INTEGER NOT NULL CHECK(is_available IN (0,1))
) STRICT;

CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    volume_id INTEGER NOT NULL REFERENCES volumes(id) ON DELETE RESTRICT,
    path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('FILE','DIRECTORY','IMAGE','VIDEO','AUDIO','DOC')),
    file_identity TEXT,
    mime_type TEXT,
    size_bytes INTEGER CHECK(size_bytes>=0),
    created_at TEXT,
    updated_at TEXT NOT NULL,
    missing INTEGER NOT NULL DEFAULT 0 CHECK(missing IN (0,1)),
    favorited INTEGER NOT NULL DEFAULT 0 CHECK(favorited IN (0,1)),
    trashed_at TEXT,
    original_path TEXT,
    duration_ms INTEGER CHECK(duration_ms>=0),
    width INTEGER CHECK(width>=0),
    height INTEGER CHECK(height>=0),
    rotation INTEGER CHECK(rotation IN (0,90,180,270)),
    taken_at TEXT,
    title TEXT,
    artist TEXT,
    album TEXT,
    album_artist TEXT,
    album_id TEXT,
    cover_image_id TEXT,
    CHECK(path<>'' OR kind='DIRECTORY'),
    CHECK(substr(path,1,1)<>'/'),
    CHECK(instr(path,char(92))=0 AND instr(path,char(0))=0),
    CHECK(instr('/'||path||'/','/../')=0 AND instr('/'||path||'/','/./')=0),
    CHECK(instr(path,'//')=0 AND (path='' OR substr(path,-1)<>'/')),
    CHECK((trashed_at IS NULL)=(original_path IS NULL)),
    CHECK(original_path IS NULL OR (
    substr(original_path,1,1)<>'/' AND instr(original_path,char(92))=0
    AND instr(original_path,char(0))=0
    AND instr('/'||original_path||'/','/../')=0
    AND instr('/'||original_path||'/','/./')=0
    AND instr(original_path,'//')=0
    AND (original_path='' OR substr(original_path,-1)<>'/')
    ))
) STRICT;

CREATE UNIQUE INDEX IF NOT EXISTS uidx_files_volume_id_path ON files(volume_id,path)
WHERE missing=0;

CREATE INDEX IF NOT EXISTS idx_files_volume_id_file_identity ON files(volume_id,file_identity)
WHERE file_identity IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_files_kind_updated_at_id ON files(kind,updated_at,id)
WHERE missing=0 AND trashed_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_files_kind_id ON files(kind,id) WHERE missing=0 AND trashed_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_files_trashed_at_id ON files(trashed_at,id) WHERE trashed_at IS NOT NULL;

CREATE VIRTUAL TABLE IF NOT EXISTS file_names_fts USING fts5(
    file_name, content='files', content_rowid='id', tokenize='trigram'
);

CREATE TRIGGER IF NOT EXISTS files_after_insert AFTER INSERT ON files BEGIN
    INSERT INTO file_names_fts(rowid,file_name) VALUES(new.id,new.file_name);
END;

CREATE TRIGGER IF NOT EXISTS files_after_delete AFTER DELETE ON files BEGIN
    INSERT INTO file_names_fts(file_names_fts,rowid,file_name) VALUES('delete',old.id,old.file_name);
END;

CREATE TRIGGER IF NOT EXISTS files_after_file_name_update AFTER UPDATE OF file_name ON files
    WHEN old.file_name IS NOT new.file_name BEGIN
    INSERT INTO file_names_fts(file_names_fts,rowid,file_name) VALUES('delete',old.id,old.file_name);
    INSERT INTO file_names_fts(rowid,file_name) VALUES(new.id,new.file_name);
END;

CREATE TABLE IF NOT EXISTS image_editor_projects (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    state_b64 TEXT NOT NULL,
    thumbnail TEXT,
    canvas_width INTEGER NOT NULL CHECK(canvas_width>0),
    canvas_height INTEGER NOT NULL CHECK(canvas_height>0),
    layer_count INTEGER NOT NULL CHECK(layer_count>=0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_image_editor_projects_updated_at ON image_editor_projects(updated_at DESC);

CREATE TABLE IF NOT EXISTS image_embeddings (
    file_id TEXT NOT NULL PRIMARY KEY CHECK(length(file_id)>0),
    path TEXT NOT NULL,
    embedding BLOB NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS key_values (key TEXT PRIMARY KEY NOT NULL,value TEXT NOT NULL) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS media_play_history (
    file_id TEXT NOT NULL PRIMARY KEY CHECK(length(file_id)>0),
    media_type INTEGER NOT NULL CHECK(media_type IN (1,2)),
    position_ms INTEGER NOT NULL CHECK(position_ms>=0),
    play_count INTEGER NOT NULL CHECK(play_count>=0),
    played_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_media_play_history_media_type_played_at ON media_play_history(media_type,played_at);

CREATE TABLE IF NOT EXISTS nearby_devices (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    ips TEXT NOT NULL,
    port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
    device_type TEXT NOT NULL CHECK(device_type IN ('PHONE','TABLET','COMPUTER','TV','NAS','OTHER')),
    version TEXT NOT NULL,
    platform TEXT NOT NULL,
    last_seen_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    deleted_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at DESC);

CREATE TABLE IF NOT EXISTS peers (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    ip TEXT NOT NULL,
    key TEXT NOT NULL,
    public_key TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('PAIRED','UNPAIRED','CHANNEL')),
    port INTEGER CHECK(port BETWEEN 1 AND 65535),
    device_type TEXT NOT NULL CHECK(device_type IN ('PHONE','TABLET','COMPUTER','TV','NAS','OTHER','UNKNOWN')),
    token TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS pomodoro_items (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    date TEXT NOT NULL UNIQUE,
    completed_count INTEGER NOT NULL CHECK(completed_count>=0),
    total_work_secs INTEGER NOT NULL CHECK(total_work_secs>=0),
    total_break_secs INTEGER NOT NULL CHECK(total_break_secs>=0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS scan_history (
    id INTEGER PRIMARY KEY,
    text TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    client_id TEXT NOT NULL PRIMARY KEY CHECK(length(client_id)>0),
    name TEXT NOT NULL DEFAULT '',
    type TEXT NOT NULL DEFAULT 'WEB' CHECK(type IN ('WEB','CUSTOM')),
    client_ip TEXT NOT NULL,
    os_name TEXT NOT NULL,
    os_version TEXT NOT NULL,
    browser_name TEXT NOT NULL,
    browser_version TEXT NOT NULL,
    token TEXT NOT NULL,
    last_seen_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS shares (
    id TEXT NOT NULL PRIMARY KEY CHECK(length(id)>0),
    name TEXT NOT NULL,
    password TEXT NOT NULL,
    url_token TEXT NOT NULL,
    expires_at TEXT,
    read_only INTEGER NOT NULL CHECK(read_only IN (0,1)),
    data TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tags (
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

CREATE INDEX IF NOT EXISTS idx_tag_relations_target_key ON tag_relations(target_key);

CREATE TABLE IF NOT EXISTS trashed_sms (
    message_id TEXT NOT NULL CHECK(length(message_id)>0),
    is_mms INTEGER NOT NULL CHECK(is_mms IN (0,1)),
    trashed_at TEXT NOT NULL,
    PRIMARY KEY(message_id, is_mms)
);
