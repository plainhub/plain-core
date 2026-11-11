use crate::features::prefs::settings::FileSortBy;
use crate::features::services::files::catalog::MediaCatalog;
use crate::features::services::images::models::Runtime;
use crate::features::services::images::status::ImageSearchStatusType;

use crate::features::enums::DataType;
use plain_libs::serde_json::Value;
use std::collections::{HashMap, HashSet};

fn enabled(models: &Runtime, text: &str) -> bool {
    !text.trim().is_empty() && models.snapshot().status.status == ImageSearchStatusType::Ready
}

fn sort_by(value: &str) -> Result<FileSortBy, String> {
    plain_libs::serde_json::from_value(Value::String(value.to_owned())).map_err(|error| error.to_string())
}

async fn filename_rows(
    catalog: &MediaCatalog,
    query: &str,
    offset: i32,
    limit: i32,
    sort: FileSortBy,
) -> Result<Vec<Value>, String> {
    catalog
        .rows(
            DataType::Image,
            query,
            sort,
            limit.max(0) as u32,
            offset.max(0) as u32,
        )
        .await
}

async fn semantic_extras(
    catalog: &MediaCatalog,
    models: &Runtime,
    text: &str,
    query: &str,
) -> Result<Vec<String>, String> {
    let ids = models
        .search(text)
        .await?
        .into_iter()
        .map(|row| row.image_id)
        .collect::<Vec<_>>();
    let mut extras = Vec::new();
    for candidates in ids.chunks(256) {
        let semantic = catalog.image_matches(query, candidates, false).await?;
        let matches = catalog.image_matches(query, candidates, true).await?;
        let filename_ids = matches
            .iter()
            .filter_map(|row| row["id"].as_str())
            .collect::<HashSet<_>>();
        let semantic_ids = semantic
            .iter()
            .filter_map(|row| row["id"].as_str())
            .collect::<HashSet<_>>();
        extras.extend(
            candidates
                .iter()
                .filter(|id| {
                    semantic_ids.contains(id.as_str()) && !filename_ids.contains(id.as_str())
                })
                .cloned(),
        );
    }
    Ok(extras)
}

pub async fn rows(
    catalog: &MediaCatalog,
    models: &Runtime,
    text: &str,
    query: &str,
    offset: i32,
    limit: i32,
    sort: &str,
) -> Result<Vec<Value>, String> {
    if offset < 0 || limit < 0 {
        return Err("Invalid image pagination".into());
    }
    let sort = sort_by(sort)?;
    if !enabled(models, text) {
        return filename_rows(catalog, query, offset, limit, sort).await;
    }
    let mut items = filename_rows(catalog, query, offset, limit, sort).await?;
    if items.len() == limit as usize {
        return Ok(items);
    }
    let filename_count = catalog.count(DataType::Image, query).await?;
    let extra = semantic_extras(catalog, models, text, query).await?;
    let extra_offset = (i64::from(offset) - filename_count).max(0) as usize;
    let remaining = limit as usize - items.len();
    let selected = extra
        .into_iter()
        .skip(extra_offset)
        .take(remaining)
        .collect::<Vec<_>>();
    let positions = selected
        .iter()
        .enumerate()
        .map(|(position, id)| (id.as_str(), position))
        .collect::<HashMap<_, _>>();
    let mut semantic = Vec::new();
    for ids in selected.chunks(256) {
        semantic.extend(catalog.image_matches(query, ids, false).await?);
    }
    semantic.sort_by_key(|row| {
        positions
            .get(row["id"].as_str().unwrap_or_default())
            .copied()
            .unwrap_or(usize::MAX)
    });
    items.extend(semantic);
    Ok(items)
}

pub async fn count(
    catalog: &MediaCatalog,
    models: &Runtime,
    text: &str,
    query: &str,
) -> Result<i32, String> {
    if !enabled(models, text) {
        return catalog
            .count(DataType::Image, query)
            .await
            .and_then(|count| i32::try_from(count).map_err(|error| error.to_string()));
    }
    let filename_count = catalog.count(DataType::Image, query).await?;
    let extra = semantic_extras(catalog, models, text, query).await?;
    i32::try_from(filename_count + extra.len() as i64).map_err(|error| error.to_string())
}
