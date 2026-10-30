use crate::graphql::resolvers::files::types::FileSortBy;
use crate::graphql::resolvers::images::public_index::ImageSearchStatusType;
use crate::services::files::catalog::MediaCatalog;
use crate::services::images::models::Runtime;

use plain_rs::enums::DataType;
use serde_json::Value;
use std::collections::HashSet;

fn enabled(models: &Runtime, text: &str) -> bool {
    !text.trim().is_empty() && models.snapshot().status.status == ImageSearchStatusType::Ready
}

fn sort_by(value: &str) -> Result<FileSortBy, String> {
    serde_json::from_value(Value::String(value.to_owned())).map_err(|error| error.to_string())
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

async fn semantic_rows(
    catalog: &MediaCatalog,
    models: &Runtime,
    text: &str,
    query: &str,
) -> Result<Vec<Value>, String> {
    let ids = models
        .search(text, 50)
        .await?
        .into_iter()
        .map(|row| row.image_id)
        .collect::<Vec<_>>();
    let mut rows = catalog.image_matches(query, &ids, false).await?;
    rows.sort_by_key(|row| {
        ids.iter()
            .position(|id| Some(id.as_str()) == row["id"].as_str())
            .unwrap_or(usize::MAX)
    });
    Ok(rows)
}

pub(crate) async fn rows(
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
    items.extend(extra.into_iter().skip(extra_offset).take(remaining));
    Ok(items)
}

async fn semantic_extras(
    catalog: &MediaCatalog,
    models: &Runtime,
    text: &str,
    query: &str,
) -> Result<Vec<Value>, String> {
    let semantic = semantic_rows(catalog, models, text, query).await?;
    let candidates = semantic
        .iter()
        .filter_map(|row| row["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let matches = catalog.image_matches(query, &candidates, true).await?;
    let filename_ids = matches
        .iter()
        .filter_map(|row| row["id"].as_str())
        .collect::<HashSet<_>>();
    Ok(semantic
        .into_iter()
        .filter(|row| {
            row["id"]
                .as_str()
                .is_some_and(|id| !filename_ids.contains(id))
        })
        .collect())
}

pub(crate) async fn count(
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
