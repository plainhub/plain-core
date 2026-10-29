use crate::db::{
    Db, MediaFileMetadata, MediaFileMetadataAvailability, MediaFileMetadataRead, MediaFileRow,
};
use crate::enums::DataType;
use rusqlite::{OptionalExtension, params};

pub const COLUMNS: &str = "id,data_type,path,title,artist,bucket_id,size_bytes,duration_ms,cover_image_id,width,height,rotation,taken_at,created_at,updated_at,favorited,trashed,original_path,album,album_artist,album_id";

pub fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaFileRow> {
    Ok(MediaFileRow {
        id: row.get(0)?,
        data_type: row.get(1)?,
        path: row.get(2)?,
        title: row.get(3)?,
        artist: row.get(4)?,
        bucket_id: row.get(5)?,
        size_bytes: row.get(6)?,
        duration_ms: row.get(7)?,
        cover_image_id: row.get(8)?,
        width: row.get(9)?,
        height: row.get(10)?,
        rotation: row.get(11)?,
        taken_at: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        favorited: row.get(15)?,
        trashed: row.get(16)?,
        original_path: row.get(17)?,
        album: row.get(18)?,
        album_artist: row.get(19)?,
        album_id: row.get(20)?,
    })
}

pub const SOURCE: &str = "file_catalog";

pub(crate) fn init(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TEMP VIEW file_catalog AS SELECT CAST(f.id AS TEXT) AS id,f.id AS file_id,
CASE f.kind WHEN 'AUDIO' THEN 1 WHEN 'VIDEO' THEN 2 WHEN 'IMAGE' THEN 3 WHEN 'DOC' THEN 24 ELSE 22 END AS data_type,
CASE WHEN substr(v.path,-1)='/' THEN v.path||f.path ELSE v.path||CASE WHEN f.path='' THEN '' ELSE '/'||f.path END END AS path,
f.file_name,f.kind,f.volume_id,f.file_identity,f.missing,v.is_available,
COALESCE(f.title,'') AS title,COALESCE(f.artist,'') AS artist,COALESCE(f.album,'') AS album,COALESCE(f.album_artist,'') AS album_artist,COALESCE(f.album_id,'') AS album_id,
CASE WHEN instr(f.path,'/')=0 THEN CASE WHEN v.path='/' THEN '/' ELSE rtrim(v.path,'/') END ELSE rtrim(v.path,'/')||'/'||substr(f.path,1,length(f.path)-length(f.file_name)-1) END AS bucket_id,
COALESCE(f.mime_type,'') AS mime_type,CAST(COALESCE(f.size_bytes,0) AS INTEGER) AS size_bytes,CAST(COALESCE(f.duration_ms,0) AS INTEGER) AS duration_ms,f.cover_image_id,
CAST(COALESCE(f.width,0) AS INTEGER) AS width,CAST(COALESCE(f.height,0) AS INTEGER) AS height,CAST(COALESCE(f.rotation,0) AS INTEGER) AS rotation,f.taken_at,COALESCE(f.created_at,f.updated_at) AS created_at,f.updated_at,
f.favorited,f.trashed_at,f.trashed_at IS NOT NULL AS trashed,
CASE WHEN f.original_path IS NULL THEN NULL ELSE rtrim(v.path,'/')||'/'||f.original_path END AS original_path
FROM files f JOIN volumes v ON v.id=f.volume_id")
}

fn kind(data_type: DataType) -> rusqlite::Result<crate::db::FileKind> {
    crate::db::FileKind::try_from(data_type).map_err(super::files::sql_error)
}

impl Db {
    pub fn record_media_files(
        &self,
        files: &[MediaFileMetadata],
    ) -> rusqlite::Result<Vec<MediaFileRow>> {
        self.record_media_file_inputs(
            files
                .iter()
                .map(|file| (file, MediaFileMetadataAvailability::ALL, None)),
        )
    }

    pub fn record_media_file_reads(
        &self,
        files: &[MediaFileMetadataRead],
    ) -> rusqlite::Result<Vec<MediaFileRow>> {
        self.record_media_file_inputs(files.iter().map(|file| {
            (
                &file.metadata,
                file.available,
                file.file_identity.as_deref(),
            )
        }))
    }

    fn record_media_file_inputs<'a>(
        &self,
        files: impl IntoIterator<
            Item = (
                &'a MediaFileMetadata,
                MediaFileMetadataAvailability,
                Option<&'a str>,
            ),
        >,
    ) -> rusqlite::Result<Vec<MediaFileRow>> {
        let inputs = files
            .into_iter()
            .map(|(file, available, identity)| {
                Ok((file, available, identity, super::files::locate(&file.path)?))
            })
            .collect::<rusqlite::Result<Vec<_>>>()?;
        self.with_conn(|conn| {
            let tx=conn.unchecked_transaction()?;
            let mut rows=Vec::with_capacity(inputs.len());
            for (file,available,identity,location) in inputs {
                let id=super::files::upsert(&tx,&location,Some(kind(file.data_type)?),identity,Some(file.size_bytes),Some(&crate::utils::dbtime::normalize(&file.created_at)?),&crate::utils::dbtime::normalize(&file.updated_at)?)?;
                tx.execute("UPDATE files SET title=CASE WHEN ?1 THEN ?2 ELSE title END,artist=CASE WHEN ?1 THEN ?3 ELSE artist END,album=CASE WHEN ?1 THEN ?4 ELSE album END,album_artist=CASE WHEN ?1 THEN ?5 ELSE album_artist END,album_id=CASE WHEN ?1 THEN ?6 ELSE album_id END,duration_ms=CASE WHEN ?7 THEN ?8 ELSE duration_ms END,cover_image_id=CASE WHEN ?9 THEN ?10 ELSE cover_image_id END,width=CASE WHEN ?11 THEN ?12 ELSE width END,height=CASE WHEN ?11 THEN ?13 ELSE height END,rotation=CASE WHEN ?14 THEN ?15 ELSE rotation END,taken_at=CASE WHEN ?16 THEN ?17 ELSE taken_at END WHERE id=?18",
                    params![available.tags,file.title,file.artist,file.album,file.album_artist,crate::media::audio::metadata::album_id(&file.album,&file.album_artist,&file.artist),available.duration,file.duration_ms,available.cover_image_id,file.cover_image_id,available.dimensions,file.width,file.height,available.rotation,file.rotation,available.taken_at,file.taken_at.as_deref().map(crate::utils::dbtime::normalize).transpose()?,id])?;
                rows.push(tx.query_row(&format!("SELECT {COLUMNS} FROM {SOURCE} WHERE file_id=?1"),[id],from_row)?);
            }
            tx.commit()?;
            Ok(rows)
        })
    }

    pub fn media_file_by_path(
        &self,
        data_type: DataType,
        path: &str,
    ) -> rusqlite::Result<Option<MediaFileRow>> {
        kind(data_type)?;
        let location = super::files::locate(path)?;
        self.with_conn(|conn| {
            let volume:Option<(i64,String)>=conn.query_row("SELECT id,path FROM volumes WHERE identity=?1",[&location.volume.identity],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
            let Some((volume_id,root))=volume else { return Ok(None); };
            let relative=location.path.strip_prefix(std::path::Path::new(&root)).or_else(|_|location.path.strip_prefix(&location.volume.path)).map_err(super::files::sql_error)?;
            let relative=super::files::relative_path(relative)?;
            conn.query_row(&format!("SELECT {COLUMNS} FROM {SOURCE} WHERE data_type=?1 AND missing=0 AND is_available=1 AND file_id IN (SELECT id FROM files WHERE volume_id=?2 AND path=?3 AND missing=0)"),params![data_type.kind(),volume_id,relative],from_row).optional()
        })
    }

    pub fn forget_media_file(&self, path: &str) -> rusqlite::Result<usize> {
        self.forget_media_file_types(path).map(|types| types.len())
    }
    pub fn forget_media_file_types(&self, path: &str) -> rusqlite::Result<Vec<i32>> {
        self.forget_media_path_rows(path, false)
    }
    pub fn forget_media_path_types(&self, path: &str) -> rusqlite::Result<Vec<i32>> {
        self.forget_media_path_rows(path, true)
    }

    fn forget_media_path_rows(&self, path: &str, recursive: bool) -> rusqlite::Result<Vec<i32>> {
        if path.is_empty() {
            return Ok(Vec::new());
        }
        let location = super::files::locate(path)?;
        let path = location
            .path
            .to_str()
            .ok_or(rusqlite::Error::InvalidQuery)?;
        let prefix = format!("{}/", path.trim_end_matches('/'));
        self.audio_transaction(|tx,state| {
            let rows=tx.prepare(&format!("SELECT file_id,data_type FROM {SOURCE} WHERE missing=0 AND (path=?1 OR (?2 AND substr(path,1,length(?3))=?3))"))?
                .query_map(params![path,recursive,prefix],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,i32>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            for (id,_) in &rows {
                tx.execute("UPDATE files SET missing=1 WHERE id=?1",[id])?;
                if state.current_file_id.as_deref()==Some(id.to_string().as_str()) { state.select("")?; }
            }
            Ok(rows.into_iter().map(|(_,kind)|kind).collect())
        }).map_err(|error|match error {
            crate::error::DomainError::Sqlite(error)=>error,
            error=>super::files::sql_error(error),
        })
    }

    pub fn media_file_list(
        &self,
        data_type: DataType,
        limit: u32,
        offset: u32,
    ) -> rusqlite::Result<Vec<MediaFileRow>> {
        kind(data_type)?;
        self.with_conn(|conn| conn.prepare(&format!("SELECT {COLUMNS} FROM {SOURCE} WHERE data_type=?1 AND trashed=0 AND missing=0 AND is_available=1 ORDER BY updated_at DESC,file_id LIMIT ?2 OFFSET ?3"))?
            .query_map(params![data_type.kind(),limit,offset],from_row)?.collect())
    }
    pub fn media_file_count(&self, data_type: DataType) -> rusqlite::Result<i64> {
        kind(data_type)?;
        self.with_conn(|conn|conn.query_row(&format!("SELECT count(*) FROM {SOURCE} WHERE data_type=?1 AND trashed=0 AND missing=0 AND is_available=1"),[data_type.kind()],|row|row.get(0)))
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/db/repositories/media_file.rs"]
mod tests;
