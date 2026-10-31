use plain_rs::prefs::settings::FileSortBy;
use plain_rs::services::files::catalog::MediaCatalog;
use plain_rs::{
    db::Db,
    enums::DataType,
    media::audio::overview::{self, Accumulator, Overview},
};
use serde_json::Value;
use std::sync::Arc;

const PAGE_SIZE: u32 = 512;

pub async fn read(db: Arc<Db>, catalog: MediaCatalog) -> Result<Overview<Value>, String> {
    if let MediaCatalog::Native(source) = &catalog {
        let source = source.clone();
        return tokio::task::spawn_blocking(move || {
            let summary = overview::native(&source).map_err(|error| error.to_string())?;
            Ok(Overview {
                artists: summary.artists,
                recent_items: summary
                    .recent_items
                    .iter()
                    .map(plain_rs::services::files::media_catalog_facts::row_facts)
                    .collect::<Result<_, _>>()?,
            })
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    for _ in 0..3 {
        if let Some(summary) = platform_read(db.clone(), &catalog).await? {
            return Ok(summary);
        }
    }
    Err("audio catalog kept changing during summary".into())
}

async fn platform_read(
    db: Arc<Db>,
    catalog: &MediaCatalog,
) -> Result<Option<Overview<Value>>, String> {
    let history = tokio::task::spawn_blocking(move || overview::history(&db))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let mut summary = Accumulator::new(history).map_err(|error| error.to_string())?;
    let mut revision = None;
    let mut offset = 0u32;
    loop {
        let page = catalog
            .sampled_rows(DataType::Audio, "", FileSortBy::NameAsc, PAGE_SIZE, offset)
            .await?;
        if page.revision_before != page.revision_after
            || revision
                .as_ref()
                .is_some_and(|expected| expected != &page.revision_before)
        {
            return Ok(None);
        }
        revision = Some(page.revision_before);
        let count = u32::try_from(page.rows.len()).map_err(|error| error.to_string())?;
        if count > PAGE_SIZE {
            return Err("audio catalog page exceeds requested limit".into());
        }
        for row in page.rows {
            let id = row["id"].as_str().unwrap_or_default().to_owned();
            let path = row["path"].as_str().unwrap_or_default().to_owned();
            let artist = row["artist"].as_str().unwrap_or_default().to_owned();
            let created = row["createdAt"]
                .as_str()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.timestamp_millis())
                .unwrap_or_default();
            summary
                .push(&id, &artist, &path, created, row)
                .map_err(|error| error.to_string())?;
        }
        if count < PAGE_SIZE {
            break;
        }
        offset = offset
            .checked_add(count)
            .ok_or("audio catalog offset overflow")?;
    }
    Ok(Some(summary.finish()))
}

#[cfg(test)]
#[path = "../../../tests/unit/services/audio/overview.rs"]
mod tests;
