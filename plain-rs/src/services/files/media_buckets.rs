pub fn platform_plan(
    kind: plain_rs::enums::DataType,
) -> anyhow::Result<plain_rs::services::files::provider_query::Plan> {
    anyhow::ensure!(
        matches!(
            kind,
            plain_rs::enums::DataType::Image
                | plain_rs::enums::DataType::Video
                | plain_rs::enums::DataType::Audio
                | plain_rs::enums::DataType::Doc
        ),
        "unsupported media bucket type"
    );
    let mut plan = plain_rs::services::files::provider_query::plan(
        plain_rs::services::files::provider_query::media_provider(kind)?,
        &[],
        &serde_json::Value::Null,
        None,
        true,
    )?;
    if kind == plain_rs::enums::DataType::Audio {
        plan.add("duration > 0".into(), vec![]);
    }
    plan.add("bucket_display_name != ''".into(), vec![]);
    Ok(plan)
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemFact {
    pub id: String,
    pub name: String,
    pub size: i64,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub id: String,
    pub name: String,
    pub item_count: u64,
    pub size: i64,
    pub top_items: Vec<String>,
    sort_name: String,
}

pub fn aggregate(data_type: &str, items: Vec<ItemFact>) -> anyhow::Result<Vec<Bucket>> {
    aggregate_with_sort_key(data_type, items, plain_rs::utils::sort_name::sort_name)
}

fn aggregate_with_sort_key(
    data_type: &str,
    items: Vec<ItemFact>,
    mut sort_key: impl FnMut(&str) -> String,
) -> anyhow::Result<Vec<Bucket>> {
    anyhow::ensure!(
        matches!(data_type, "IMAGE" | "VIDEO" | "AUDIO" | "DOC"),
        "unsupported media bucket type"
    );
    let mut positions = HashMap::<String, usize>::new();
    let mut buckets = Vec::<Bucket>::new();
    for item in items {
        anyhow::ensure!(item.size >= 0, "invalid media bucket item size");
        if let Some(&index) = positions.get(&item.id) {
            let bucket = &mut buckets[index];
            anyhow::ensure!(bucket.name == item.name, "inconsistent media bucket name");
            bucket.item_count = bucket
                .item_count
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("media bucket item count overflow"))?;
            bucket.size = bucket
                .size
                .checked_add(item.size)
                .ok_or_else(|| anyhow::anyhow!("media bucket size overflow"))?;
            if bucket.top_items.len() < 4 {
                bucket.top_items.push(item.path);
            }
        } else {
            positions.insert(item.id.clone(), buckets.len());
            let sort_name = sort_key(&item.name);
            buckets.push(Bucket {
                id: item.id,
                name: item.name,
                item_count: 1,
                size: item.size,
                top_items: vec![item.path],
                sort_name,
            });
        }
    }
    buckets.sort_by(|a, b| a.sort_name.cmp(&b.sort_name).then_with(|| a.id.cmp(&b.id)));
    Ok(buckets)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/media_buckets.rs"]
mod tests;
