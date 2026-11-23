pub fn platform_plan(
    kind: crate::features::enums::DataType,
) -> plain_libs::anyhow::Result<crate::features::services::files::provider_query::Plan> {
    plain_libs::anyhow::ensure!(
        matches!(
            kind,
            crate::features::enums::DataType::Image
                | crate::features::enums::DataType::Video
                | crate::features::enums::DataType::Audio
                | crate::features::enums::DataType::Doc
        ),
        "unsupported media bucket type"
    );
    let mut plan = crate::features::services::files::provider_query::plan(
        crate::features::services::files::provider_query::media_provider(kind)?,
        &[],
        &plain_libs::serde_json::Value::Null,
        None,
        true,
    )?;
    if kind == crate::features::enums::DataType::Audio {
        plan.add("duration > 0".into(), vec![]);
    }
    plan.add("bucket_display_name != ''".into(), vec![]);
    Ok(plan)
}

use crate::features::enums::DataType;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemFact {
    pub id: String,
    pub name: String,
    pub size: i64,
    path: String,
    updated_at: String,
}

pub struct CoverItem {
    pub path: String,
    pub updated_at: String,
    pub thumb_hash: Option<Vec<u8>>,
}

pub struct Bucket {
    pub id: String,
    pub name: String,
    pub item_count: u64,
    pub size: i64,
    pub cover_items: Vec<CoverItem>,
    pub(super) sort_name: String,
}

pub fn aggregate(
    data_type: DataType,
    items: Vec<ItemFact>,
) -> plain_libs::anyhow::Result<Vec<Bucket>> {
    aggregate_with_sort_key(data_type, items, plain_libs::utils::sort_name::sort_name)
}

fn aggregate_with_sort_key(
    data_type: DataType,
    items: Vec<ItemFact>,
    mut sort_key: impl FnMut(&str) -> String,
) -> plain_libs::anyhow::Result<Vec<Bucket>> {
    plain_libs::anyhow::ensure!(
        matches!(
            data_type,
            DataType::Image | DataType::Video | DataType::Audio | DataType::Doc
        ),
        "unsupported media bucket type"
    );
    let mut positions = HashMap::<String, usize>::new();
    let mut buckets = Vec::<Bucket>::new();
    for item in items {
        plain_libs::anyhow::ensure!(item.size >= 0, "invalid media bucket item size");
        let cover = CoverItem {
            path: item.path,
            updated_at: item.updated_at,
            thumb_hash: None,
        };
        if let Some(&index) = positions.get(&item.id) {
            let bucket = &mut buckets[index];
            plain_libs::anyhow::ensure!(bucket.name == item.name, "inconsistent media bucket name");
            bucket.item_count = bucket
                .item_count
                .checked_add(1)
                .ok_or_else(|| plain_libs::anyhow::anyhow!("media bucket item count overflow"))?;
            bucket.size = bucket
                .size
                .checked_add(item.size)
                .ok_or_else(|| plain_libs::anyhow::anyhow!("media bucket size overflow"))?;
            if bucket.cover_items.len() < 4 {
                bucket.cover_items.push(cover);
            }
        } else {
            positions.insert(item.id.clone(), buckets.len());
            let sort_name = sort_key(&item.name);
            buckets.push(Bucket {
                id: item.id,
                name: item.name,
                item_count: 1,
                size: item.size,
                cover_items: vec![cover],
                sort_name,
            });
        }
    }
    sort(&mut buckets);
    Ok(buckets)
}

pub(super) fn sort(buckets: &mut [Bucket]) {
    buckets.sort_by(|a, b| a.sort_name.cmp(&b.sort_name).then_with(|| a.id.cmp(&b.id)));
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/media_buckets.rs"]
mod tests;
