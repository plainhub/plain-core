use crate::infrastructure::db::Db;
use crate::infrastructure::error::DomainResult;

pub fn playlist_cover_candidates(db: &Db, ids: &[String]) -> DomainResult<Vec<Vec<String>>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(db.with_conn(|conn| {
        let mut statement = conn.prepare(
            "SELECT m.path FROM audio_playlist_items p JOIN audio_media m USING(file_id)
             WHERE p.playlist_id=?1 ORDER BY p.sort_order,p.id",
        )?;
        ids.iter()
            .map(|id| statement.query_map([id], |row| row.get(0))?.collect())
            .collect::<plain_libs::rusqlite::Result<_>>()
    })?)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/media/audio/playlist_covers.rs"]
mod tests;
