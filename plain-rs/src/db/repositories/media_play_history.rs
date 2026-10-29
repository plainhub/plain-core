use crate::db::{Db, HISTORY_KEEP, MediaPlayHistoryRow};
use crate::enums::DataType;
use rusqlite::{Connection, OptionalExtension, params};

pub(crate) const COLUMNS: &str = "file_id,position_ms,play_count,played_at,media_type";

fn type_mismatch() -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
        "Media play history type mismatch",
    )))
}

pub(crate) fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaPlayHistoryRow> {
    Ok(MediaPlayHistoryRow {
        file_id: row.get(0)?,
        position_ms: row.get(1)?,
        play_count: row.get(2)?,
        played_at: row.get(3)?,
        media_type: row.get(4)?,
    })
}

pub(crate) fn record_play(
    conn: &Connection,
    media_type: DataType,
    file_id: &str,
    position_ms: i64,
) -> rusqlite::Result<()> {
    let current = conn
        .query_row(
            "SELECT media_type,play_count FROM media_play_history WHERE file_id=?1",
            [file_id],
            |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
        .map(|(kind, count)| {
            if kind != media_type.kind() {
                Err(type_mismatch())
            } else {
                Ok(count)
            }
        })
        .transpose()?
        .unwrap_or(0);
    let count = current.checked_add(1).ok_or_else(|| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
            "play count overflow",
        )))
    })?;
    let changed = conn.execute("INSERT INTO media_play_history(file_id,position_ms,play_count,played_at,media_type) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(file_id) DO UPDATE SET position_ms=excluded.position_ms,play_count=excluded.play_count,played_at=excluded.played_at WHERE media_play_history.media_type=excluded.media_type", params![file_id,position_ms,count,crate::utils::dbtime::now_iso_millis(),media_type.kind()])?;
    if changed == 0 {
        return Err(type_mismatch());
    }
    if media_type == DataType::Audio {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM media_play_history WHERE media_type=1",
            [],
            |row| row.get(0),
        )?;
        if count > (HISTORY_KEEP * 5 / 4) as i64 {
            conn.execute(
                "DELETE FROM media_play_history WHERE media_type=1 AND file_id NOT IN (SELECT file_id FROM media_play_history WHERE media_type=1 ORDER BY played_at DESC,rowid DESC LIMIT ?1)",
                [HISTORY_KEEP as i64],
            )?;
        }
    }
    Ok(())
}

pub(crate) fn update_progress(
    conn: &Connection,
    media_type: DataType,
    file_id: &str,
    position_ms: i64,
) -> rusqlite::Result<()> {
    let current = conn
        .query_row(
            "SELECT media_type,position_ms FROM media_play_history WHERE file_id=?1",
            [file_id],
            |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    let Some((kind, previous)) = current else {
        return Ok(());
    };
    if kind != media_type.kind() {
        return Err(type_mismatch());
    }
    if position_ms != previous {
        conn.execute(
            "UPDATE media_play_history SET position_ms=?1,played_at=?2 WHERE file_id=?3 AND media_type=?4",
            params![position_ms, crate::utils::dbtime::now_iso_millis(), file_id, kind],
        )?;
    }
    Ok(())
}

pub(crate) fn save_progress(
    conn: &Connection,
    media_type: DataType,
    file_id: &str,
    position_ms: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO media_play_history(file_id,position_ms,play_count,played_at,media_type) VALUES(?1,?2,0,?3,?4) ON CONFLICT(file_id) DO NOTHING",
        params![file_id, position_ms, crate::utils::dbtime::now_iso_millis(), media_type.kind()],
    )?;
    update_progress(conn, media_type, file_id, position_ms)
}

impl Db {
    pub fn media_play_history_record(
        &self,
        media_type: DataType,
        file_id: &str,
        position_ms: i64,
    ) -> rusqlite::Result<()> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            record_play(&tx, media_type, file_id, position_ms)?;
            tx.commit()
        })
    }

    pub fn media_play_history_save(
        &self,
        media_type: DataType,
        file_id: &str,
        position_ms: i64,
    ) -> rusqlite::Result<()> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            save_progress(&tx, media_type, file_id, position_ms)?;
            tx.commit()
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/db/repositories/media_play_history.rs"]
mod tests;
