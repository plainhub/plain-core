use crate::features::enums::DataType;
use crate::features::tags;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::notes_feeds::NoteRow;
use crate::infrastructure::error::DomainError;
use crate::infrastructure::error::DomainResult;

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn create(db: &Db, title: &str, content: &str) -> DomainResult<NoteRow> {
    let id = crate::features::record_id::generate();
    Ok(db.note_save(&id, &resolved_title(title, content), content, &now())?)
}

pub fn update(db: &Db, id: &str, title: &str, content: &str) -> DomainResult<NoteRow> {
    if db.note_get(id)?.is_none() {
        return Err(DomainError::Other(format!("Note {id} not found")));
    }
    Ok(db.note_save(id, &resolved_title(title, content), content, &now())?)
}

pub fn search(db: &Db, query: &str, limit: i32, offset: i32) -> DomainResult<Vec<NoteRow>> {
    Ok(db.notes_list(query, i64::from(limit.max(0)), i64::from(offset.max(0)))?)
}

pub fn count(db: &Db, query: &str) -> DomainResult<i32> {
    Ok(db.notes_count(query)?)
}

pub fn get(db: &Db, id: &str) -> DomainResult<Option<NoteRow>> {
    Ok(db.note_get(id)?)
}

fn explicit(query: &str) -> DomainResult<()> {
    if query.trim().is_empty() {
        return Err(DomainError::Other("query is required for bulk mutations — pass 'all:true' to explicitly target everything".to_string()));
    }
    Ok(())
}

pub fn trash(db: &Db, query: &str) -> DomainResult<usize> {
    explicit(query)?;
    let ids = db.note_ids(query, Some(false))?;
    Ok(db.notes_set_deleted(&ids, true, &now())?)
}

pub fn restore(db: &Db, query: &str) -> DomainResult<usize> {
    explicit(query)?;
    let ids = db.note_ids(query, Some(true))?;
    Ok(db.notes_set_deleted(&ids, false, &now())?)
}

pub fn delete(db: &Db, query: &str) -> DomainResult<usize> {
    explicit(query)?;
    let ids = db.note_ids(query, Some(true))?;
    Ok(db.notes_delete(&ids)?)
}

pub fn save_feed_entries(db: &Db, query: &str) -> DomainResult<Vec<String>> {
    explicit(query)?;
    let entries = db.feed_entries_list(query, i64::MAX, 0)?;
    let mut ids = Vec::with_capacity(entries.len());
    for entry in entries {
        let content = format!(
            "# {}\n\n{}",
            entry.title,
            if entry.content.is_empty() {
                &entry.description
            } else {
                &entry.content
            }
        );
        db.note_save(&entry.id, &entry.title, &content, &now())?;
        ids.push(entry.id);
    }
    Ok(ids)
}

pub fn export(db: &Db, query: &str) -> DomainResult<String> {
    let notes = db.notes_list(query, i64::MAX, 0)?;
    let values: Vec<_> = notes
        .into_iter()
        .map(|note| -> DomainResult<serde_json::Value> {
            let tag_values: Vec<_> = tags::tags_for_key_of_kind(
                db,
                &note.id,
                DataType::Note.kind(),
            )?
            .into_iter()
            .map(|tag| serde_json::json!({ "id": tag.id, "name": tag.name, "count": tag.count }))
            .collect();
            Ok(
                serde_json::json!({ "id": note.id, "title": note.title, "content": note.content,
            "createdAt": note.created_at, "updatedAt": note.updated_at, "tags": tag_values }),
            )
        })
        .collect::<DomainResult<Vec<_>>>()?;
    Ok(serde_json::to_string(&values).map_err(|e| DomainError::Other(e.to_string()))?)
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/notes/mod.rs"]
mod tests;

pub fn markdown_title(content: &str) -> String {
    static IMAGES: std::sync::OnceLock<plain_libs::regex::Regex> = std::sync::OnceLock::new();
    let images = IMAGES.get_or_init(|| {
        plain_libs::regex::compile(r"(?i)!\[.*?\]\(.*?\)|!\[.*?\]\[.*?\]|<img.*?>").unwrap()
    });
    for line in content.lines() {
        if let Some(title) = line.trim().strip_prefix("# ") {
            return replace_images(images, title.trim()).into_owned();
        }
    }
    replace_images(images, content)
        .replace('\n', "")
        .trim()
        .chars()
        .take(50)
        .collect()
}
fn replace_images<'a>(images: &plain_libs::regex::Regex, text: &'a str) -> std::borrow::Cow<'a, str> {
    let mut matches = images.find_iter(text);
    let Some(first) = matches.next() else {
        return std::borrow::Cow::Borrowed(text);
    };
    let mut result = String::with_capacity(text.len());
    let mut start = 0;
    for found in std::iter::once(first).chain(matches) {
        result.push_str(&text[start..found.start()]);
        result.push('🖼');
        start = found.end();
    }
    result.push_str(&text[start..]);
    std::borrow::Cow::Owned(result)
}
fn resolved_title<'a>(title: &'a str, content: &str) -> std::borrow::Cow<'a, str> {
    if title.is_empty() {
        markdown_title(content).into()
    } else {
        title.into()
    }
}
