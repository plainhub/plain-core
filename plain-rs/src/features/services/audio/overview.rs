use crate::features::enums::DataType;
use crate::features::media::audio::overview;
use crate::features::media::audio::overview::Accumulator;
use crate::features::media::audio::overview::Artist;
use crate::features::prefs::settings::MediaSortBy;
use crate::features::services::files::catalog::MediaCatalog;
use crate::infrastructure::db::Db;
use std::sync::Arc;

const PAGE_SIZE: u32 = 512;

pub async fn read(db: Arc<Db>, catalog: MediaCatalog) -> Result<Vec<Artist>, String> {
    if let MediaCatalog::Native(source) = &catalog {
        let source = source.clone();
        return tokio::task::spawn_blocking(move || overview::native(&source))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string());
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
) -> Result<Option<Vec<Artist>>, String> {
    let history = tokio::task::spawn_blocking(move || overview::history(&db))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let mut summary = Accumulator::new(history).map_err(|error| error.to_string())?;
    let mut revision = None;
    let mut offset = 0u32;
    loop {
        let page = catalog
            .sampled_rows(DataType::Audio, "", MediaSortBy::TitleAsc, PAGE_SIZE, offset)
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
            summary
                .push(
                    row["id"].as_str().unwrap_or_default(),
                    row["artist"].as_str().unwrap_or_default(),
                    row["path"].as_str().unwrap_or_default(),
                )
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
#[path = "../../../../../tests/plain-rs/unit/features/services/audio/overview.rs"]
mod tests;
