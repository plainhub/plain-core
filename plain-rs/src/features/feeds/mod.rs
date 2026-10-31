use crate::libs::db::{
    Db,
    notes_feeds::{FeedEntryRow, FeedRow},
};
use crate::libs::error::{DomainError, DomainResult};
use crate::libs::utils::{
    http_url::parse_http_url,
    xml::{self, Event},
};
use chrono::Utc;

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn error(message: impl ToString) -> DomainError {
    DomainError::Other(message.to_string())
}

pub fn require_url(url: &str) -> DomainResult<()> {
    let parsed = parse_http_url(url).ok_or_else(|| error("invalid_feed_url"))?;
    if parsed.host.is_empty() {
        return Err(error("invalid_feed_url"));
    }
    Ok(())
}

pub fn save_error(error_value: rusqlite::Error) -> DomainError {
    if matches!(&error_value, rusqlite::Error::SqliteFailure(code, _) if code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE)
    {
        return error("feed_already_exists");
    }
    error_value.into()
}

pub fn update_fields(
    db: &Db,
    id: &str,
    name: Option<&str>,
    url: Option<&str>,
    fetch_full_content: Option<bool>,
) -> DomainResult<FeedRow> {
    if name.is_none() && url.is_none() && fetch_full_content.is_none() {
        return Err(error("feed_update_requires_changes"));
    }
    if let Some(url) = url {
        require_url(url)?;
    }
    db.feed_update(id, name, url, fetch_full_content, &now())
        .map_err(save_error)?
        .ok_or_else(|| error("feed_not_found"))
}

pub fn delete(db: &Db, id: &str) -> DomainResult<bool> {
    Ok(db.feed_delete(id)?)
}

pub fn list(db: &Db) -> DomainResult<Vec<FeedRow>> {
    Ok(db.feeds_list()?)
}

pub fn get(db: &Db, id: &str) -> DomainResult<Option<FeedRow>> {
    Ok(db.feed_get(id)?)
}

pub fn entry_get(db: &Db, id: &str) -> DomainResult<Option<FeedEntryRow>> {
    Ok(db.feed_entry_get(id)?)
}

pub fn entry_counts(db: &Db) -> DomainResult<Vec<(String, i32)>> {
    Ok(db.feed_entry_counts()?)
}

pub fn search(db: &Db, query: &str, limit: i32, offset: i32) -> DomainResult<Vec<FeedEntryRow>> {
    Ok(db.feed_entries_list(query, i64::from(limit.max(0)), i64::from(offset.max(0)))?)
}

pub fn count(db: &Db, query: &str) -> DomainResult<i32> {
    Ok(db.feed_entry_count(query)?)
}

pub fn delete_entries(db: &Db, query: &str) -> DomainResult<usize> {
    if query.trim().is_empty() {
        return Err(error(
            "query is required for bulk mutations — pass 'all:true' to explicitly target everything",
        ));
    }
    Ok(db.feed_entries_delete(query)?)
}

pub fn mark_read(db: &Db, query: &str, read: bool) -> DomainResult<usize> {
    if query.trim().is_empty() {
        return Err(error("query is required"));
    }
    let ids = db
        .feed_entries_list(query, i64::MAX, 0)?
        .into_iter()
        .map(|e| e.id)
        .collect::<Vec<_>>();
    Ok(db.feed_entries_mark_read(&ids, read)?)
}

pub fn import_opml(db: &Db, content: &str) -> DomainResult<()> {
    let mut reader = xml::Reader::new(content);
    let mut feeds = Vec::new();
    while let Some(event) = reader.next() {
        let Event::Start(tag) = event else {
            continue;
        };
        if tag.name != "outline" {
            continue;
        }
        // Outlines without xmlUrl are the folders that group subscriptions.
        let Some(url) = tag.attr("xmlUrl") else {
            continue;
        };
        let name = tag
            .attr("title")
            .or_else(|| tag.attr("text"))
            .unwrap_or_default()
            .to_string();
        let fetch_full_content = tag.attr("fetchContent") == Some("true");
        feeds.push((name, url.to_string(), fetch_full_content));
    }
    for (name, url, fetch_full_content) in feeds {
        // One malformed outline must not cost the user the rest of the file:
        // OPML is hand-edited and exported by plenty of other readers.
        if require_url(&url).is_err() || db.feed_get_by_url(&url)?.is_some() {
            continue;
        }
        db.feed_save(
            &uuid::Uuid::new_v4().to_string(),
            &name,
            &url,
            fetch_full_content,
            &now(),
        )?;
    }
    Ok(())
}

pub fn export_opml(db: &Db) -> DomainResult<String> {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><opml version=\"2.0\"><head><title>PlainApp</title><dateCreated>{}</dateCreated></head><body>",
        now()
    );
    for feed in db.feeds_list()? {
        xml.push_str(&format!(
            "<outline text=\"{}\" title=\"{}\" xmlUrl=\"{}\" fetchContent=\"{}\"/>",
            xml::escape(&feed.name),
            xml::escape(&feed.name),
            xml::escape(&feed.url),
            feed.fetch_full_content
        ));
    }
    xml.push_str("</body></opml>");
    Ok(xml)
}

pub mod date;

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/feeds/mod.rs"]
mod tests;
