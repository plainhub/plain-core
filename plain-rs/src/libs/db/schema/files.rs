use rusqlite::Connection;

pub(crate) fn init(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS volumes (
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
END;",
    )
}
