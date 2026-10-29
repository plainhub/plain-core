use crate::{
    db::Db,
    error::{DomainError, DomainResult},
};
use std::collections::{BTreeMap, HashMap, HashSet};

const RECENT_LIMIT: usize = 8;

pub struct History {
    pub id: String,
    pub play_count: i64,
}

pub fn history(db: &Db) -> DomainResult<Vec<History>> {
    db.with_conn(|conn| history_from_connection(conn))
}

fn history_from_connection(conn: &rusqlite::Connection) -> DomainResult<Vec<History>> {
    Ok(conn
        .prepare(
            "SELECT file_id,play_count FROM media_play_history WHERE media_type=1 ORDER BY played_at DESC,rowid DESC",
        )?
        .query_map([], |row| {
            Ok(History {
                id: row.get(0)?,
                play_count: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn native(db: &Db) -> DomainResult<Overview<crate::db::MediaFileRow>> {
    db.with_conn(|connection| {
        let transaction = connection.unchecked_transaction()?;
        let mut summary = Accumulator::new(history_from_connection(&transaction)?)?;
        #[cfg(test)]
        AFTER_NATIVE_HISTORY.with(|hook| {
            let callback = hook.borrow_mut().take();
            if let Some(callback) = callback { callback(); }
        });
        {
            let mut statement = transaction.prepare(&format!(
                "SELECT {} FROM file_catalog WHERE data_type=1 AND trashed=0 AND missing=0 AND is_available=1 ORDER BY title COLLATE NOCASE ASC,id",
                crate::db::media_file::COLUMNS,
            ))?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                let row = crate::db::media_file::from_row(row)?;
                let created_ms = chrono::DateTime::parse_from_rfc3339(&row.created_at)
                    .map_or(0, |time| time.timestamp_millis());
                let id = row.id.clone();
                let artist = row.artist.clone();
                let path = row.path.clone();
                summary.push(&id, &artist, &path, created_ms, row)?;
            }
        }
        transaction.commit()?;
        Ok(summary.finish())
    })
}

#[cfg(test)]
std::thread_local! {
    static AFTER_NATIVE_HISTORY: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = Default::default();
}

#[derive(Clone, Debug, PartialEq)]
pub struct Artist {
    pub name: String,
    pub item_count: i32,
    pub play_count: i64,
    pub sample_path: String,
}

pub struct Overview<T> {
    pub artists: Vec<Artist>,
    pub recent_items: Vec<T>,
}

pub struct Accumulator<T> {
    history: HashMap<String, (usize, i64)>,
    seen: HashSet<String>,
    artists: BTreeMap<String, Artist>,
    recent: Vec<(usize, T)>,
    newest: Vec<(i64, T)>,
}

impl<T: Clone> Accumulator<T> {
    pub fn new(history: Vec<History>) -> DomainResult<Self> {
        if history.iter().any(|item| item.play_count < 0) {
            return Err(DomainError::Other("negative audio play count".into()));
        }
        Ok(Self {
            history: history
                .into_iter()
                .enumerate()
                .map(|(rank, item)| (item.id, (rank, item.play_count)))
                .collect(),
            seen: HashSet::new(),
            artists: BTreeMap::new(),
            recent: Vec::new(),
            newest: Vec::new(),
        })
    }

    pub fn push(
        &mut self,
        id: &str,
        name: &str,
        path: &str,
        created_ms: i64,
        value: T,
    ) -> DomainResult<()> {
        if id.is_empty() || path.is_empty() {
            return Err(DomainError::Other("audio catalog identity required".into()));
        }
        if !self.seen.insert(id.to_owned()) {
            return Err(DomainError::Other(
                "audio catalog changed during summary".into(),
            ));
        }
        let history = self.history.get(id).copied();
        let name = super::metadata::normalize_artist(name);
        if !name.trim().is_empty() {
            let artist = self
                .artists
                .entry(name.to_owned())
                .or_insert_with(|| Artist {
                    name: name.to_owned(),
                    item_count: 0,
                    play_count: 0,
                    sample_path: path.to_owned(),
                });
            artist.item_count = artist
                .item_count
                .checked_add(1)
                .ok_or_else(|| DomainError::Other("audio item count overflow".into()))?;
            artist.play_count = artist
                .play_count
                .checked_add(history.map(|(_, count)| count).unwrap_or(0))
                .ok_or_else(|| DomainError::Other("audio play count overflow".into()))?;
        }
        if let Some((rank, _)) = history {
            if self.recent.len() < RECENT_LIMIT
                || self.recent.last().is_some_and(|last| rank < last.0)
            {
                self.recent.push((rank, value.clone()));
                self.recent.sort_by_key(|item| item.0);
                self.recent.truncate(RECENT_LIMIT);
            }
        }
        if self.newest.len() < RECENT_LIMIT
            || self.newest.last().is_some_and(|last| created_ms > last.0)
        {
            self.newest.push((created_ms, value));
            self.newest.sort_by(|a, b| b.0.cmp(&a.0));
            self.newest.truncate(RECENT_LIMIT);
        }
        Ok(())
    }

    pub fn finish(self) -> Overview<T> {
        let mut artists = self.artists.into_values().collect::<Vec<_>>();
        artists.sort_by(|a, b| {
            b.play_count
                .cmp(&a.play_count)
                .then_with(|| a.name.cmp(&b.name))
        });
        let recent_items = if self.recent.is_empty() {
            self.newest.into_iter().map(|(_, value)| value).collect()
        } else {
            self.recent.into_iter().map(|(_, value)| value).collect()
        };
        Overview {
            artists,
            recent_items,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/media/audio/overview.rs"]
mod tests;
