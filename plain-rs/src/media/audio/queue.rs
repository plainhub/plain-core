use crate::db::audio_queue::io;
use crate::error::DomainError;
use rusqlite::OptionalExtension;
use std::collections::HashSet;

use crate::db::Db;
use crate::db::Playlist;
use crate::db::PlaylistItem;
use crate::db::QueueItem;
use crate::error::DomainResult;
use crate::utils::dbtime::now_iso_millis;
use crate::utils::shortid;
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrack {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "mediaId")]
    pub file_id: Option<String>,
    pub title: String,
    pub artist: String,
    pub path: String,
    pub duration_ms: i64,
    pub album_id: String,
}

impl AudioTrack {
    pub fn from_path_stem(path: &str) -> Self {
        let path = path.replace('\\', "/");
        let title = std::path::Path::new(&path)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or(&path)
            .to_string();
        Self {
            file_id: None,
            title,
            artist: String::new(),
            path,
            duration_ms: 0,
            album_id: String::new(),
        }
    }
}

impl From<QueueItem> for AudioTrack {
    fn from(item: QueueItem) -> Self {
        Self {
            file_id: Some(item.file_id),
            title: item.title,
            artist: item.artist,
            path: item.path,
            duration_ms: item.duration_ms,
            album_id: String::new(),
        }
    }
}

impl From<&QueueItem> for AudioTrack {
    fn from(item: &QueueItem) -> Self {
        item.clone().into()
    }
}

impl From<&PlaylistItem> for AudioTrack {
    fn from(i: &PlaylistItem) -> Self {
        Self {
            file_id: Some(i.file_id.clone()),
            title: i.title.clone(),
            artist: i.artist.clone(),
            path: i.audio_path.clone(),
            duration_ms: i.duration_ms,
            album_id: i.album_id.clone(),
        }
    }
}
const PREF_AUDIO_MODE: &str = "audio_play_mode";
pub fn get_audio_current(db: &Db) -> DomainResult<String> {
    db.restore_audio_current()?;
    Ok(db.audio().read(|state| state.playback.path.clone()))
}

pub fn save_audio_current(db: &Db, path: &str) -> DomainResult<()> {
    db.audio_transaction(|tx, state| {
        io::save_current(tx, path, state)?;
        state.select(path)
    })
}
pub fn play_mode(
    prefs: &crate::prefs::Prefs,
) -> DomainResult<crate::media::audio::play_mode::MediaPlayMode> {
    Ok(prefs
        .get(PREF_AUDIO_MODE)
        .map_err(|e| DomainError::Other(e.to_string()))?
        .unwrap_or(crate::media::audio::play_mode::MediaPlayMode::Repeat))
}
pub fn get_audio_mode(prefs: &crate::prefs::Prefs) -> DomainResult<String> {
    Ok(play_mode(prefs)?.name().into())
}
pub fn save_audio_mode(prefs: &crate::prefs::Prefs, mode: &str) -> DomainResult<bool> {
    let mode = crate::media::audio::play_mode::MediaPlayMode::from_name(mode.trim())
        .ok_or_else(|| DomainError::Other("invalid audio play mode".into()))?;
    prefs
        .set(PREF_AUDIO_MODE, mode)
        .map_err(|e| DomainError::Other(e.to_string()))
}

pub fn on_playing(db: &Db, path: &str) -> DomainResult<()> {
    if path.is_empty() {
        return Ok(());
    }
    transaction(db, |c, state| {
        io::save_current(c, path, state)?;
        state.select(path)?;
        record_history_conn(c, path)?;
        Ok(())
    })
}
pub fn enqueue(db: &Db, items: &[AudioTrack], play_next: bool) -> DomainResult<()> {
    enqueue_with_selection(db, items, play_next, false)
}

pub fn enqueue_selecting_if_idle(db: &Db, items: &[AudioTrack]) -> DomainResult<()> {
    enqueue_with_selection(db, items, false, true)
}

fn enqueue_with_selection(
    db: &Db,
    items: &[AudioTrack],
    play_next: bool,
    select_if_idle: bool,
) -> DomainResult<()> {
    if items.is_empty() {
        return Ok(());
    }
    for item in items {
        validate_duration(item.duration_ms)?;
        if item.path.is_empty() {
            return Err(DomainError::Other("empty audio path".into()));
        }
    }
    transaction(db, |c, state| {
        enqueue_conn(c, items, play_next)?;
        if select_if_idle && state.playback.path.is_empty() {
            for item in items {
                if let Some(track) = io::audio_by_path(c, &item.path)? {
                    io::save_current(c, &track.path, state)?;
                    state.select(&track.path)?;
                    break;
                }
            }
        }
        Ok(())
    })
}

pub(crate) fn enqueue_conn(
    c: &rusqlite::Connection,
    items: &[AudioTrack],
    play_next: bool,
) -> DomainResult<()> {
    if !play_next {
        let mut order: i64 = c.query_row(
            "SELECT COALESCE(MAX(sort_order),-1)+1 FROM audio_queue_items",
            [],
            |row| row.get(0),
        )?;
        for track in items {
            let resolved = if let Some(id) = &track.file_id {
                let path: Option<String> = c
                    .query_row(
                        "SELECT path FROM audio_media WHERE file_id=?1",
                        [id],
                        |row| row.get(0),
                    )
                    .optional()?;
                path.map(|path| io::audio_by_path(c, &path))
                    .transpose()?
                    .flatten()
            } else {
                io::audio_by_path(c, &track.path)?
            };
            if let Some(id) = resolved.and_then(|track| track.file_id) {
                if c.execute(
                    "INSERT OR IGNORE INTO audio_queue_items(file_id,sort_order) VALUES(?1,?2)",
                    rusqlite::params![id, order],
                )? > 0
                {
                    order += 1;
                }
            }
        }
        return Ok(());
    }
    let mut queued = io::all_queue_items(c)?;
    let mut incoming = Vec::new();
    let mut seen = HashSet::new();
    for item in items.iter().rev() {
        let Some(item) = io::audio_by_path(c, &item.path)? else {
            continue;
        };
        if !seen.insert(item.path.clone()) {
            continue;
        }
        incoming.push(QueueItem {
            file_id: item.file_id.clone().unwrap(),
            path: item.path.clone(),
            sort_order: 0,
            title: item.title.clone(),
            artist: item.artist.clone(),
            duration_ms: item.duration_ms,
        });
    }
    incoming.reverse();
    queued.retain(|q| !seen.contains(&q.path));
    if play_next {
        incoming.extend(queued);
        queued = incoming;
    } else {
        queued.extend(incoming);
    }
    io::replace_queue_items(c, &queued)?;
    Ok(())
}

pub fn remove_queued(db: &Db, file_id: &str) -> DomainResult<bool> {
    transaction(db, |c, state| {
        let removed = c.execute("DELETE FROM audio_queue_items WHERE file_id=?1", [file_id])? > 0;
        if removed && state.current_file_id.as_deref() == Some(file_id) {
            state.select("")?;
        }
        Ok(removed)
    })
}

pub fn reorder_queued(db: &Db, file_ids: &[String]) -> DomainResult<()> {
    transaction(db, |c, _| {
        let mut seen = HashSet::new();
        let exists = c
            .prepare("SELECT file_id FROM audio_queue_items ORDER BY sort_order,file_id")?
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if file_ids
            .iter()
            .any(|id| !seen.insert(id.clone()) || !exists.contains(id))
        {
            return Err(DomainError::Other("invalid queue media IDs".into()));
        }
        let ordered = file_ids
            .iter()
            .cloned()
            .chain(exists.into_iter().filter(|id| !seen.contains(id)));
        for (order, id) in ordered.enumerate() {
            c.execute(
                "UPDATE audio_queue_items SET sort_order=?1 WHERE file_id=?2",
                rusqlite::params![order as i64, id],
            )?;
        }
        Ok(())
    })
}
pub fn remove_paths(db: &Db, paths: &[String]) -> DomainResult<()> {
    if paths.is_empty() {
        return Ok(());
    }
    transaction(db, |c, state| remove_paths_conn(c, paths, state))
}

pub(crate) fn remove_root_paths_conn(
    c: &rusqlite::Connection,
    root: &str,
    state: &mut super::state::State,
) -> DomainResult<()> {
    remove_root_paths_except_conn(c, root, state, &[])
}

pub(crate) fn remove_root_paths_except_conn(
    c: &rusqlite::Connection,
    root: &str,
    state: &mut super::state::State,
    retained_ids: &[String],
) -> DomainResult<()> {
    let prefix = format!("{}/", root.trim_end_matches('/'));
    let mut statement = c.prepare("SELECT path FROM audio_media WHERE file_id IN (SELECT file_id FROM audio_queue_items UNION SELECT file_id FROM audio_playlist_items UNION SELECT file_id FROM media_play_history WHERE media_type=1) AND file_id NOT IN (SELECT value FROM json_each(?3)) AND (path=?1 OR substr(path,1,length(?2))=?2)")?;
    let mut paths = statement
        .query_map(
            rusqlite::params![
                root,
                prefix,
                serde_json::to_string(retained_ids)
                    .map_err(|e| DomainError::Other(e.to_string()))?
            ],
            |row| row.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    let current = io::audio_by_path(c, &state.playback.path)?
        .map(|track| track.path)
        .unwrap_or_else(|| state.playback.path.clone());
    if (current == root || current.starts_with(&prefix))
        && !state
            .current_file_id
            .as_ref()
            .is_some_and(|id| retained_ids.contains(id))
    {
        paths.push(current);
    }
    remove_paths_conn(c, &paths, state)
}

pub(crate) fn remove_paths_conn(
    c: &rusqlite::Connection,
    paths: &[String],
    state: &mut super::state::State,
) -> DomainResult<()> {
    let current = io::audio_by_path(c, &state.playback.path)?
        .map(|track| track.path)
        .unwrap_or_else(|| state.playback.path.clone());
    io::remove_queue_paths(c, paths)?;
    io::remove_history(c, paths)?;
    io::remove_playlist_items_by_paths(c, paths)?;
    if paths.contains(&current) || paths.contains(&state.playback.path) {
        state.select("")?;
    }
    Ok(())
}

pub fn clear_queue(db: &Db) -> DomainResult<()> {
    transaction(db, |c, state| {
        c.execute("DELETE FROM audio_queue_items", [])?;
        state.select("")
    })
}

pub fn select_next(db: &Db, is_next: bool, shuffle: bool) -> DomainResult<Option<AudioTrack>> {
    transaction(db, |c, state| {
        let items = io::all_queue_items(c)?;
        if items.is_empty() {
            return Ok(None);
        }
        let current = items
            .iter()
            .position(|item| Some(item.file_id.as_str()) == state.current_file_id.as_deref());
        let index = if shuffle {
            use rand::Rng;
            if items.len() > 1 {
                let selected =
                    rand::thread_rng().gen_range(0..items.len() - usize::from(current.is_some()));
                if current.is_some_and(|index| selected >= index) {
                    selected + 1
                } else {
                    selected
                }
            } else {
                0
            }
        } else if is_next {
            current.map(|index| (index + 1) % items.len()).unwrap_or(0)
        } else {
            current
                .map(|index| (index + items.len() - 1) % items.len())
                .unwrap_or(items.len() - 1)
        };
        let track = AudioTrack::from(&items[index]);
        state.current_file_id = track.file_id.clone();
        state.select(&track.path)?;
        Ok(Some(track))
    })
}

pub fn queue_total(db: &Db) -> DomainResult<usize> {
    Ok(db.with_conn(|c| {
        c.query_row(
            "SELECT COUNT(*) FROM audio_queue_items q JOIN audio_media m USING(file_id)",
            [],
            |row| crate::db::read_count(row, 0),
        )
    })?)
}

pub fn queue_page(db: &Db, offset: i64, limit: i64, query: &str) -> DomainResult<Vec<AudioTrack>> {
    validate_page(offset, limit)?;
    Ok(crate::db::audio_queue::all_queue_items(db)?
        .into_iter()
        .map(AudioTrack::from)
        .filter(|item| matches_query(item, query))
        .skip(offset as usize)
        .take(limit as usize)
        .collect())
}

fn validate_page(offset: i64, limit: i64) -> DomainResult<()> {
    if offset < 0 || limit < 0 {
        return Err(DomainError::Other("invalid pagination".into()));
    }
    Ok(())
}

pub fn matches_query(track: &AudioTrack, query: &str) -> bool {
    crate::utils::search_dsl::parse(query).iter().all(|field| {
        let matched = match field.name.as_str() {
            "ids" => track
                .file_id
                .as_ref()
                .is_some_and(|id| field.value.split(',').any(|value| value == id)),
            "text" => {
                let needle = field.value.to_lowercase();
                [&track.title, &track.artist, &track.path]
                    .iter()
                    .any(|value| value.to_lowercase().contains(&needle))
            }
            _ => return true,
        };
        if matches!(field.op.as_str(), "!=" | "nin" | "NOT") {
            !matched
        } else {
            matched
        }
    })
}

pub fn track_by_id(db: &Db, file_id: &str) -> DomainResult<AudioTrack> {
    db.with_conn(|c| {
        c.query_row(
            "SELECT path FROM audio_media WHERE file_id=?1",
            [file_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
    })?
    .map(|path| db.with_conn(|c| io::audio_by_path(c, &path)))
    .transpose()?
    .flatten()
    .ok_or_else(|| DomainError::Other("audio not found".into()))
}

pub fn playlists(db: &Db) -> DomainResult<Vec<(Playlist, usize)>> {
    let all = crate::db::audio_queue::all_playlists(db)?;
    let counts = crate::db::audio_queue::playlist_item_counts(db)?;
    Ok(all
        .into_iter()
        .map(|pl| {
            let count = counts.get(&pl.id).copied().unwrap_or(0);
            (pl, count)
        })
        .collect())
}

pub fn playlist_by_id(db: &Db, id: &str) -> DomainResult<Option<Playlist>> {
    Ok(crate::db::audio_queue::playlist_by_id(db, id)?)
}

pub fn create_playlist(db: &Db, name: &str) -> DomainResult<Playlist> {
    let now = now_iso_millis();
    let pl = Playlist {
        id: shortid::new_id(),
        name: name.to_string(),
        created_at: now.clone(),
        updated_at: now,
    };
    crate::db::audio_queue::insert_playlist(db, &pl)?;
    Ok(pl)
}

pub fn rename_playlist(db: &Db, id: &str, name: &str) -> DomainResult<()> {
    transaction(db, |c, _| {
        let mut pl = io::playlist_by_id(c, id)?
            .ok_or_else(|| DomainError::Other("playlist not found".into()))?;
        pl.name = name.into();
        pl.updated_at = now_iso_millis();
        io::update_playlist(c, &pl)?;
        Ok(())
    })
}

pub fn delete_playlist(db: &Db, id: &str) -> DomainResult<()> {
    transaction(db, |c, _| {
        io::delete_playlist(c, id)?;
        io::delete_playlist_items(c, id)?;
        Ok(())
    })
}
pub fn add_playlist_items(db: &Db, playlist_id: &str, items: &[AudioTrack]) -> DomainResult<usize> {
    for item in items {
        validate_duration(item.duration_ms)?;
        if item.path.is_empty() {
            return Err(DomainError::Other("empty audio path".into()));
        }
    }
    transaction(db, |c, _| {
        let mut pl = io::playlist_by_id(c, playlist_id)?
            .ok_or_else(|| DomainError::Other("playlist not found".into()))?;
        let existing = io::playlist_items(c, playlist_id)?;
        let mut next = existing.last().map(|i| i.sort_order + 1).unwrap_or(0);
        let mut paths: HashSet<String> = existing.into_iter().map(|i| i.audio_path).collect();
        let mut added = 0;
        let now = now_iso_millis();
        for a in items {
            let Some(a) = io::audio_by_path(c, &a.path)? else {
                continue;
            };
            if !paths.insert(a.path.clone()) {
                continue;
            }
            let row = PlaylistItem {
                file_id: a.file_id.clone().unwrap(),
                id: shortid::new_id(),
                playlist_id: playlist_id.into(),
                audio_path: a.path.clone(),
                title: a.title.clone(),
                artist: a.artist.clone(),
                duration_ms: a.duration_ms,
                album_id: a.album_id.clone(),
                sort_order: next,
                added_at: now.clone(),
            };
            io::insert_playlist_item(c, &row)?;
            next += 1;
            added += 1;
        }
        pl.updated_at = now;
        io::update_playlist(c, &pl)?;
        Ok(added)
    })
}

pub fn remove_playlist_item(db: &Db, playlist_id: &str, path: &str) -> DomainResult<()> {
    transaction(db, |c, _| {
        io::remove_playlist_item(c, playlist_id, path)?;
        if let Some(mut pl) = io::playlist_by_id(c, playlist_id)? {
            pl.updated_at = now_iso_millis();
            io::update_playlist(c, &pl)?;
        }
        Ok(())
    })
}

pub fn playlist_items_page(
    db: &Db,
    playlist_id: &str,
    offset: i64,
    limit: i64,
    query: &str,
) -> DomainResult<Vec<AudioTrack>> {
    Ok(
        playlist_item_records_page(db, playlist_id, offset, limit, query)?
            .iter()
            .map(AudioTrack::from)
            .collect(),
    )
}

pub fn playlist_item_count(db: &Db, playlist_id: &str) -> DomainResult<usize> {
    Ok(crate::db::audio_queue::playlist_count(db, playlist_id)?)
}

pub fn playlist_filtered_count(db: &Db, playlist_id: &str, query: &str) -> DomainResult<usize> {
    Ok(crate::db::audio_queue::playlist_items(db, playlist_id)?
        .iter()
        .filter(|item| matches_query(&AudioTrack::from(*item), query))
        .count())
}

pub fn remove_playlist_query(db: &Db, playlist_id: &str, query: &str) -> DomainResult<usize> {
    if query.trim().is_empty() {
        return Err(DomainError::Other("bulk_query_required".into()));
    }
    transaction(db, |c, _| {
        let mut count = 0;
        for row in io::playlist_items(c, playlist_id)? {
            if matches_query(&AudioTrack::from(&row), query) {
                count += c.execute(
                    "DELETE FROM audio_playlist_items WHERE playlist_id=?1 AND file_id=?2",
                    rusqlite::params![playlist_id, row.file_id],
                )?;
            }
        }
        if count > 0 {
            c.execute(
                "UPDATE audio_playlists SET updated_at=?1 WHERE id=?2",
                rusqlite::params![now_iso_millis(), playlist_id],
            )?;
        }
        Ok(count)
    })
}
fn transaction<T>(
    db: &Db,
    operation: impl FnOnce(&rusqlite::Connection, &mut super::state::State) -> DomainResult<T>,
) -> DomainResult<T> {
    db.audio_transaction(|tx, state| operation(tx, state))
}
fn validate_duration(value: i64) -> DomainResult<()> {
    if value < 0 {
        return Err(DomainError::Other("negative duration".into()));
    }
    Ok(())
}
pub(crate) fn record_history_conn(c: &rusqlite::Connection, path: &str) -> DomainResult<()> {
    let file_id = c.query_row("SELECT file_id FROM audio_media WHERE path=?1 UNION SELECT a.file_id FROM platform_audio_aliases a JOIN audio_media m USING(file_id) WHERE a.path=?1",[path],|row|row.get::<_,String>(0)).optional()?;
    let Some(file_id) = file_id else {
        return Ok(());
    };
    crate::db::media_play_history::record_play(c, crate::enums::DataType::Audio, &file_id, 0)?;
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/unit/media/audio/queue.rs"]
mod tests;

pub fn playlist_item_records_page(
    db: &Db,
    playlist_id: &str,
    offset: i64,
    limit: i64,
    text: &str,
) -> DomainResult<Vec<PlaylistItem>> {
    if offset < 0 || limit < 0 {
        return Err(DomainError::Other("invalid pagination".into()));
    }
    Ok(crate::db::audio_queue::playlist_items(db, playlist_id)?
        .into_iter()
        .filter(|item| matches_query(&AudioTrack::from(item), text))
        .skip(offset as usize)
        .take(limit as usize)
        .collect())
}

pub fn remove_playlist_items(db: &Db, playlist_id: &str, paths: &[String]) -> DomainResult<()> {
    transaction(db, |c, _| {
        for path in paths {
            io::remove_playlist_item(c, playlist_id, path)?;
        }
        if let Some(mut pl) = io::playlist_by_id(c, playlist_id)? {
            pl.updated_at = now_iso_millis();
            io::update_playlist(c, &pl)?;
        }
        Ok(())
    })
}
pub fn move_queued(db: &Db, from: i32, to: i32) -> DomainResult<()> {
    transaction(db, |c, _| {
        let mut items = io::all_queue_items(c)?;
        if from < 0 || to < 0 || from as usize >= items.len() || to as usize >= items.len() {
            return Err(DomainError::Other("invalid queue index".into()));
        }
        let item = items.remove(from as usize);
        items.insert(to as usize, item);
        for (i, item) in items.iter_mut().enumerate() {
            item.sort_order = i as i64;
        }
        io::replace_queue_items(c, &items)?;
        Ok(())
    })
}
pub fn track_by_path_conn(
    conn: &rusqlite::Connection,
    path: &str,
) -> rusqlite::Result<Option<AudioTrack>> {
    io::audio_by_path(conn, path)
}

pub fn selection_ids(query: &str) -> DomainResult<Option<Vec<String>>> {
    let fields = crate::utils::search_dsl::parse(query);
    if fields.len() != 1 || fields[0].name != "ids" || fields[0].op != "=" {
        return Ok(None);
    }
    let ids = fields[0]
        .value
        .split(',')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if ids.iter().any(|id| id.trim().is_empty()) {
        return Err(crate::error::DomainError::Other(
            "media IDs required".into(),
        ));
    }
    Ok(Some(ids))
}
