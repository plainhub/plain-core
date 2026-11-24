use crate::infrastructure::db::key_blob;
use super::{assets::Assets, preferences::Preferences, short_id};
use crate::features::files::shares::SharePath;
use crate::features::services::pomodoro::PomodoroSettings;
use crate::infrastructure::types::Instant;
use plain_libs::anyhow::{Context, Result, bail, ensure};
use plain_libs::rusqlite::{Connection, Row, params, types::Value as SqlValue};
use plain_libs::serde_json::{self, Value, json};
use plain_libs::utils::dbtime::normalize;
use serde::Deserialize;
use std::collections::HashMap;

pub(super) fn for_each(
    source: &Connection,
    sql: &str,
    mut convert: impl FnMut(&Row<'_>) -> Result<()>,
) -> Result<()> {
    let mut select = source.prepare(sql)?;
    let mut rows = select.query([])?;
    while let Some(row) = rows.next()? {
        convert(row).with_context(|| format!("Legacy import failed for {sql}"))?;
    }
    Ok(())
}

pub(super) fn dates(row: &Row<'_>) -> Result<(String, String)> {
    Ok((
        normalize(&row.get::<_, String>("created_at")?)?,
        normalize(&row.get::<_, String>("updated_at")?)?,
    ))
}

fn date(row: &Row<'_>, name: &str) -> Result<String> {
    Ok(normalize(&row.get::<_, String>(name)?)?)
}

fn nullable_date(row: &Row<'_>, name: &str) -> Result<Option<String>> {
    Ok(row
        .get::<_, Option<String>>(name)?
        .as_deref()
        .map(normalize)
        .transpose()?)
}

pub(super) fn import(
    source: &Connection,
    target: &Connection,
    assets: &mut Assets<'_>,
) -> Result<()> {
    for_each(source, "SELECT * FROM files", |row| {
        assets.app_file(
            row.get("id")?,
            row.get("size")?,
            row.get("mime_type")?,
            row.get("real_path")?,
            row.get("ref_count")?,
            row.get("created_at")?,
            row.get("updated_at")?,
        )
    })?;
    let merged_entries = import_feeds(source, target)?;
    for_each(source, "SELECT * FROM notes", |row| {
        let id = short_id::convert(&row.get::<_, String>("id")?)?;
        let id = merged_entries.get(&id).unwrap_or(&id);
        let (created, updated) = dates(row)?;
        let content = assets.note_images(&row.get::<_, String>("content")?, &created, &updated)?;
        target.execute(
            "INSERT INTO notes(id,title,content,deleted_at,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO NOTHING",
            params![id, row.get::<_, SqlValue>("title")?, content, nullable_date(row, "deleted_at")?, created, updated],
        )?;
        Ok(())
    })?;
    for_each(source, "SELECT * FROM pomodoro_items", |row| {
        let (created, updated) = dates(row)?;
        target.execute(
            "INSERT INTO pomodoro_items(id,date,completed_count,total_work_secs,total_break_secs,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(date) DO UPDATE SET \
             completed_count=completed_count+excluded.completed_count,total_work_secs=total_work_secs+excluded.total_work_secs, \
             total_break_secs=total_break_secs+excluded.total_break_secs,created_at=min(created_at,excluded.created_at), \
             updated_at=max(updated_at,excluded.updated_at)",
            params![short_id::convert(&row.get::<_, String>("id")?)?, row.get::<_, SqlValue>("date")?, row.get::<_, SqlValue>("completed_count")?,
                row.get::<_, SqlValue>("total_work_seconds")?, row.get::<_, SqlValue>("total_break_seconds")?, created, updated],
        )?;
        Ok(())
    })?;
    import_tags(source, target, &merged_entries)?;
    import_bookmarks(source, target, assets)?;
    import_other_tables(source, target)
}

fn import_feeds(source: &Connection, target: &Connection) -> Result<HashMap<String, String>> {
    let mut merged_feeds = HashMap::new();
    for_each(source, "SELECT * FROM feeds", |row| {
        let id = short_id::convert(&row.get::<_, String>("id")?)?;
        let url: String = row.get("url")?;
        let (created, updated) = dates(row)?;
        let raw: String = row.get("last_error")?;
        let error = feed_error(&raw)?;
        let inserted = target.execute(
            "INSERT INTO feeds(id,name,url,fetch_full_content,last_sync_at,last_error,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(url) DO NOTHING",
            params![id, row.get::<_, SqlValue>("name")?, url, row.get::<_, SqlValue>("fetch_content")?,
                nullable_date(row, "last_sync_at")?, error, created, updated],
        )?;
        if inserted == 0 {
            let current =
                target.query_row("SELECT id FROM feeds WHERE url=?1", [url], |row| row.get(0))?;
            merged_feeds.insert(id, current);
        }
        Ok(())
    })?;
    let mut merged_entries = HashMap::new();
    for_each(source, "SELECT * FROM feed_entries", |row| {
        let id = short_id::convert(&row.get::<_, String>("id")?)?;
        let source_id: String = row.get("raw_id")?;
        let feed = short_id::convert(&row.get::<_, String>("feed_id")?)?;
        let feed = merged_feeds.get(&feed).unwrap_or(&feed);
        let (created, updated) = dates(row)?;
        let inserted = target.execute(
            "INSERT INTO feed_entries(id,title,url,image,description,author,content,feed_id,source_entry_id,published_at,is_read,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(feed_id,source_entry_id) DO NOTHING",
            params![id, row.get::<_, SqlValue>("title")?, row.get::<_, SqlValue>("url")?, row.get::<_, SqlValue>("image")?,
                row.get::<_, SqlValue>("description")?, row.get::<_, SqlValue>("author")?, row.get::<_, SqlValue>("content")?, feed,
                source_id, date(row, "published_at")?, row.get::<_, SqlValue>("read")?, created, updated],
        )?;
        if inserted == 0 {
            let current = target.query_row(
                "SELECT id FROM feed_entries WHERE feed_id=?1 AND source_entry_id=?2",
                params![feed, source_id],
                |row| row.get(0),
            )?;
            merged_entries.insert(id, current);
        }
        Ok(())
    })?;
    Ok(merged_entries)
}

fn feed_error(raw: &str) -> Result<String> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct LegacyError {
        code: String,
        detail: String,
    }

    let error: LegacyError = if raw.is_empty() {
        LegacyError::default()
    } else {
        serde_json::from_str(raw)?
    };
    let code = match error.code.to_ascii_uppercase().as_str() {
        "" | "NONE" => "NONE",
        "TIMEOUT" => "TIMEOUT",
        "SERVER" => "SERVER",
        "DNS" => "DNS",
        "PARSE" => "PARSE",
        _ => "UNKNOWN",
    };
    Ok(json!({"code": code, "detail": error.detail}).to_string())
}

fn import_tags(
    source: &Connection,
    target: &Connection,
    merged_entries: &HashMap<String, String>,
) -> Result<()> {
    for_each(source, "SELECT * FROM tags WHERE type<>9", |row| {
        let (created, updated) = dates(row)?;
        target.execute(
            "INSERT INTO tags(id,name,type,created_at,updated_at) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO NOTHING",
            params![short_id::convert(&row.get::<_, String>("id")?)?, row.get::<_, SqlValue>("name")?, tag_type(row.get("type")?)?, created, updated],
        )?;
        Ok(())
    })?;
    for_each(source, "SELECT * FROM tag_relations WHERE type<>9", |row| {
        let kind = tag_type(row.get("type")?)?;
        let key: String = row.get("key")?;
        let key = if matches!(kind, "NOTE" | "FEED_ENTRY") {
            short_id::convert(&key)?
        } else {
            key
        };
        let key = match kind {
            "NOTE" | "FEED_ENTRY" => merged_entries.get(&key).unwrap_or(&key),
            _ => &key,
        };
        target.execute(
            "INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES (?1,?2,?3,?4) ON CONFLICT DO NOTHING",
            params![short_id::convert(&row.get::<_, String>("tag_id")?)?, key, kind, date(row, "created_at")?],
        )?;
        Ok(())
    })
}

fn tag_type(value: i32) -> Result<&'static str> {
    Ok(match value {
        0 => "DEFAULT",
        1 => "AUDIO",
        2 => "VIDEO",
        3 => "IMAGE",
        4 => "SMS",
        5 => "CONTACT",
        6 => "NOTE",
        7 => "FEED_ENTRY",
        8 => "CALL",
        21 => "PACKAGE",
        22 => "FILE",
        23 => "APP_FILE",
        24 => "DOC",
        _ => bail!("Invalid legacy tag type {value}"),
    })
}

fn import_bookmarks(source: &Connection, target: &Connection, assets: &Assets<'_>) -> Result<()> {
    for_each(source, "SELECT * FROM bookmark_groups", |row| {
        let (created, updated) = dates(row)?;
        target.execute(
            "INSERT INTO bookmark_groups(id,name,collapsed,sort_order,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO NOTHING",
            params![
                short_id::convert(&row.get::<_, String>("id")?)?,
                row.get::<_, SqlValue>("name")?,
                row.get::<_, SqlValue>("collapsed")?,
                row.get::<_, SqlValue>("sort_order")?,
                created,
                updated
            ],
        )?;
        Ok(())
    })?;
    for_each(source, "SELECT * FROM bookmarks", |row| {
        let (created, updated) = dates(row)?;
        let favicon: String = row.get("favicon_path")?;
        let favicon = if favicon.starts_with("fid:") {
            assets.fid(&favicon)?
        } else {
            favicon
        };
        target.execute(
            "INSERT INTO bookmarks(id,url,title,favicon_path,group_id,pinned,click_count,last_clicked_at,sort_order,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(id) DO NOTHING",
            params![short_id::convert(&row.get::<_, String>("id")?)?, row.get::<_, SqlValue>("url")?, row.get::<_, SqlValue>("title")?, favicon, short_id::optional(&row.get::<_, String>("group_id")?)?,
                row.get::<_, SqlValue>("pinned")?, row.get::<_, SqlValue>("click_count")?, nullable_date(row, "last_clicked_at")?,
                row.get::<_, SqlValue>("sort_order")?, created, updated],
        )?;
        Ok(())
    })
}

fn import_other_tables(source: &Connection, target: &Connection) -> Result<()> {
    for_each(source, "SELECT * FROM clipboard", |row| {
        let peer = short_id::reference(&row.get::<_, String>("source")?)?;
        target.execute(
            "INSERT INTO clipboards(id,text,hash,source,label,sensitive,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO NOTHING",
            params![short_id::convert(&row.get::<_, String>("id")?)?, row.get::<_, SqlValue>("text")?, row.get::<_, SqlValue>("hash")?, peer, row.get::<_, SqlValue>("label")?,
                row.get::<_, SqlValue>("sensitive")?, date(row, "created_at")?],
        )?;
        Ok(())
    })?;
    for_each(source, "SELECT * FROM archived_conversations", |row| {
        let millis: i64 = row.get("conversation_date")?;
        let archived = plain_libs::chrono::DateTime::from_timestamp_millis(millis)
            .context("Invalid legacy SMS archive date")?;
        target.execute(
            "INSERT INTO archived_sms_conversations(conversation_id,archived_at) VALUES (?1,?2) \
             ON CONFLICT(conversation_id) DO UPDATE SET archived_at=max(archived_at,excluded.archived_at)",
            params![row.get::<_, SqlValue>("conversation_id")?, archived.to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true)],
        )?;
        Ok(())
    })?;
    for_each(source, "SELECT * FROM shares", |row| {
        let (created, updated) = dates(row)?;
        let data = share_paths(&row.get::<_, String>("data")?)?;
        target.execute(
            "INSERT INTO shares(id,name,password,url_token,expires_at,read_only,data,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO NOTHING",
            params![short_id::convert(&row.get::<_, String>("id")?)?, row.get::<_, SqlValue>("name")?, row.get::<_, SqlValue>("password")?, key_blob::optional(&row.get::<_, String>("url_token")?)?,
                nullable_date(row, "expires_at")?, row.get::<_, SqlValue>("read_only")?, data, created, updated],
        )?;
        Ok(())
    })
}

fn share_paths(data: &str) -> Result<String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct LegacySharePath {
        virtual_path: String,
        real_path: String,
        is_dir: Option<bool>,
    }

    let legacy: Vec<LegacySharePath> = serde_json::from_str(data)?;
    let mut paths = Vec::with_capacity(legacy.len());
    for path in legacy {
        let virtual_path = path.virtual_path.trim_matches('/');
        ensure!(
            !virtual_path.is_empty()
                && virtual_path
                    .split('/')
                    .all(|part| !matches!(part, "" | "." | ".."))
                && !virtual_path.contains(['\\', '\0']),
            "Invalid legacy share path"
        );
        paths.push(SharePath {
            virtual_path: format!("/{virtual_path}"),
            real_path: path.real_path,
            is_dir: path.is_dir.unwrap_or(true),
        });
    }
    Ok(serde_json::to_string(&paths)?)
}

pub(super) fn import_preferences(
    values: Preferences,
    target: &Connection,
    assets: &mut Assets<'_>,
) -> Result<Option<Value>> {
    if let Some(scan_history) = values.scan_history {
        let mut rowid: i64 = target.query_row(
            "SELECT coalesce(min(rowid),1) FROM scan_history",
            [],
            |row| row.get(0),
        )?;
        for text in serde_json::from_str::<Vec<String>>(&scan_history)? {
            rowid = rowid.checked_sub(1).context("Scan history is too large")?;
            target.execute("INSERT INTO scan_history(rowid,id,text) SELECT ?1,?2,?3 WHERE NOT EXISTS (SELECT 1 FROM scan_history WHERE text=?3)",
                params![rowid, crate::features::record_id::generate(), text])?;
        }
    }
    if let Some(favorites) = values.favorite_folders {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Favorite {
            root_path: String,
            full_path: String,
            alias: Option<String>,
        }
        for favorite in serde_json::from_str::<Vec<Favorite>>(&favorites)? {
            let (root, relative) = crate::features::favorites::split_full_path(
                &favorite.root_path,
                &favorite.full_path,
            )?;
            target.execute("INSERT INTO favorite_folders(root_path,relative_path,alias) VALUES (?1,?2,?3) ON CONFLICT DO NOTHING",
                params![root, relative, favorite.alias])?;
        }
    }
    if let Some(draft) = values.quick_note_draft {
        let now = crate::infrastructure::db::now_iso();
        let value = assets.note_images(&draft, &now, &now)?;
        target.execute(
            "INSERT INTO key_values(key,value) VALUES ('quick_note_draft',?1) ON CONFLICT(key) DO NOTHING",
            [serde_json::to_string(&value)?],
        )?;
    }
    values
        .pomodoro_settings
        .map(|settings| convert_pomodoro(&settings, assets))
        .transpose()
}

fn convert_pomodoro(data: &str, assets: &mut Assets<'_>) -> Result<Value> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct OldSettings {
        work_duration: Option<i32>,
        short_break_duration: Option<i32>,
        long_break_duration: Option<i32>,
        pomodoros_before_long_break: Option<i32>,
        show_notification: Option<bool>,
        play_sound_on_complete: Option<bool>,
        sound_path: Option<String>,
        original_sound_name: Option<String>,
    }
    let old: OldSettings = serde_json::from_str(data)?;
    let defaults = PomodoroSettings::default();
    let sound_path = old.sound_path.unwrap_or_default();
    if !sound_path.is_empty() {
        ensure!(
            sound_path == "app://audio/pomodoro_sound.mp3",
            "Invalid legacy Pomodoro sound path"
        );
        assets.pomodoro_sound()?;
    }
    Ok(serde_json::to_value(PomodoroSettings {
        work_duration_min: old.work_duration.unwrap_or(defaults.work_duration_min),
        short_break_duration_min: old
            .short_break_duration
            .unwrap_or(defaults.short_break_duration_min),
        long_break_duration_min: old
            .long_break_duration
            .unwrap_or(defaults.long_break_duration_min),
        pomodoros_before_long_break: old
            .pomodoros_before_long_break
            .unwrap_or(defaults.pomodoros_before_long_break),
        show_notification: old.show_notification.unwrap_or(defaults.show_notification),
        play_sound_on_complete: old
            .play_sound_on_complete
            .unwrap_or(defaults.play_sound_on_complete),
        sound_path,
        original_sound_name: old.original_sound_name.unwrap_or_default(),
    })?)
}
