use crate::{db::Db, error::DomainResult};

#[derive(Clone, Debug, PartialEq)]
pub struct AlbumCover {
    pub album_id: String,
    pub track_count: i32,
    pub candidate_paths: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaylistCovers {
    pub id: String,
    pub albums: Vec<AlbumCover>,
}

pub fn playlist_covers(db: &Db, ids: &[String]) -> DomainResult<Vec<PlaylistCovers>> {
    Ok(db.with_conn(|conn| {
        let mut statement = conn.prepare(
            "WITH tracks AS (
                SELECT m.album_id,m.path,p.sort_order
                FROM audio_playlist_items p JOIN audio_media m ON m.file_id=p.file_id
                WHERE p.playlist_id=?1 AND trim(m.album_id)<>''
            ), albums AS (
                SELECT album_id,count(*) AS track_count,min(sort_order) AS first_order
                FROM tracks GROUP BY album_id
                ORDER BY track_count DESC,first_order,album_id LIMIT 8
            ), candidates AS (
                SELECT album_id,path,row_number() OVER (PARTITION BY album_id ORDER BY sort_order,path) AS position
                FROM tracks
            )
            SELECT a.album_id,a.track_count,c.path
            FROM albums a JOIN candidates c ON c.album_id=a.album_id
            WHERE c.position<=3
            ORDER BY a.track_count DESC,a.first_order,a.album_id,c.position",
        )?;
        ids.iter().map(|id| {
            let rows = statement.query_map([id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?, row.get::<_, String>(2)?))
            })?;
            let mut albums: Vec<AlbumCover> = Vec::new();
            for row in rows {
                let (album_id, track_count, path) = row?;
                if albums.last().is_none_or(|album| album.album_id != album_id) {
                    albums.push(AlbumCover { album_id, track_count, candidate_paths: Vec::new() });
                }
                albums.last_mut().unwrap().candidate_paths.push(path);
            }
            Ok(PlaylistCovers { id: id.clone(), albums })
        }).collect::<rusqlite::Result<Vec<_>>>()
    })?)
}

#[cfg(test)]
#[path = "../../../tests/unit/media/audio/playlist_covers.rs"]
mod tests;
