use crate::db::Db;
use crate::media::audio::queue::AudioTrack;
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioMedia {
    #[serde(rename = "mediaId")]
    pub file_id: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(flatten)]
    pub track: AudioTrack,
}

pub(crate) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TEMP TABLE platform_audio_media (
            file_id TEXT NOT NULL PRIMARY KEY,path TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,artist TEXT NOT NULL,duration_ms INTEGER NOT NULL,album_id TEXT NOT NULL
        );
        CREATE TEMP TABLE platform_audio_aliases(path TEXT NOT NULL PRIMARY KEY,file_id TEXT NOT NULL);
        CREATE TEMP TABLE platform_audio_catalog (enabled INTEGER NOT NULL);
        CREATE TEMP VIEW audio_media AS
            SELECT file_id,path,title,artist,duration_ms,album_id FROM platform_audio_media
            UNION ALL
            SELECT id,path,title,artist,duration_ms,album_id FROM file_catalog
            WHERE data_type=1 AND trashed=0 AND missing=0 AND is_available=1 AND NOT EXISTS(SELECT 1 FROM platform_audio_catalog);",
    )
}

impl Db {
    pub fn audio_restore_needed(&self) -> bool {
        self.audio().restore_needed()
    }

    pub fn use_platform_audio_catalog(&self) -> rusqlite::Result<()> {
        self.with_conn(|conn| conn.execute("INSERT INTO platform_audio_catalog(enabled) SELECT 1 WHERE NOT EXISTS(SELECT 1 FROM platform_audio_catalog)", []).map(|_| ()))
    }

    pub fn audio_current_file_id(&self) -> Option<String> {
        self.audio().read(|state| state.current_file_id.clone())
    }

    pub fn audio_reference_ids(&self) -> rusqlite::Result<Vec<String>> {
        let mut ids = self.with_conn(|conn| {
            conn.prepare("SELECT file_id FROM audio_queue_items UNION SELECT file_id FROM audio_playlist_items UNION SELECT file_id FROM media_play_history WHERE media_type=1")?
                .query_map([], |row| row.get(0))?.collect::<rusqlite::Result<Vec<String>>>()
        })?;
        if let Some(id) = self.audio_current_file_id().filter(|id| !ids.contains(id)) {
            ids.push(id);
        }
        Ok(ids)
    }

    pub fn forget_platform_audio_paths(&self, paths: &[String]) -> rusqlite::Result<()> {
        self.with_conn(|conn| {
            let paths=serde_json::to_string(paths).map_err(|error|rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            let tx=conn.unchecked_transaction()?;
            tx.execute("DELETE FROM platform_audio_aliases WHERE path IN (SELECT value FROM json_each(?1))",[&paths])?;
            tx.execute("DELETE FROM platform_audio_media WHERE path IN (SELECT value FROM json_each(?1))",[&paths])?;
            tx.commit()
        })
    }

    pub fn cache_platform_audio(&self, rows: &[AudioMedia], replace: bool) -> rusqlite::Result<()> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            if replace { tx.execute("DELETE FROM platform_audio_media", [])?; }
            for row in rows {
                let Some(id) = row.file_id.as_deref().filter(|id| !id.is_empty()) else { continue; };
                tx.execute("INSERT INTO platform_audio_media(file_id,path,title,artist,duration_ms,album_id) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(file_id) DO UPDATE SET path=excluded.path,title=excluded.title,artist=excluded.artist,duration_ms=excluded.duration_ms,album_id=excluded.album_id",
                    params![id,row.track.path,row.track.title,row.track.artist,row.track.duration_ms,row.track.album_id])?;
                for path in &row.aliases { tx.execute("INSERT INTO platform_audio_aliases(path,file_id) VALUES(?1,?2) ON CONFLICT(path) DO UPDATE SET file_id=excluded.file_id",params![path,id])?; }
            }
            tx.commit()
        })
    }

    pub fn update_platform_audio_metadata(&self, track: &AudioTrack) -> rusqlite::Result<usize> {
        self.with_conn(|conn| {
            conn.execute(
            "UPDATE platform_audio_media SET title=?1,artist=?2,duration_ms=?3 WHERE file_id=?4",
            params![track.title, track.artist, track.duration_ms, track.file_id],
        )
        })
    }

    pub fn restore_audio_current(&self) -> crate::error::DomainResult<()> {
        if !self.audio().restore_needed() {
            return Ok(());
        }
        let Some(id) = self.audio_current_file_id() else {
            return self.audio().restore(String::new());
        };
        let path = self.with_conn(|conn| {
            conn.query_row(
                "SELECT path FROM audio_media WHERE file_id=?1",
                [&id],
                |row| row.get::<_, String>(0),
            )
            .optional()
        })?;
        if let Some(path) = path {
            self.audio().restore(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/db/repositories/audio_catalog.rs"]
mod tests;
