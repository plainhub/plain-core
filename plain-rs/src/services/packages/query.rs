use plain_rs::{db::Db, utils::search_dsl};
use serde_json::Value;

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_lowercase()
}
fn instant(value: &Value, key: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    value[key]
        .as_str()
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
}
pub fn packages(
    db: &Db,
    mut facts: Vec<Value>,
    query: &str,
    sort_by: &str,
) -> anyhow::Result<Vec<Value>> {
    let groups = search_dsl::parse(query);
    let first = |key| {
        groups
            .iter()
            .find(|g| g.name == key)
            .map(|g| g.value.as_str())
            .unwrap_or_default()
    };
    let filter_type = first("type");
    let filter_text = first("text").to_lowercase();
    let filter_ids = first("ids")
        .split(',')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let tag_ids = first("tag_id")
        .split(',')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let tagged = if tag_ids.is_empty() {
        None
    } else {
        let mut ids = std::collections::HashSet::new();
        for tag in tag_ids {
            ids.extend(plain_rs::db::tag::keys_for_tag(db, tag)?);
        }
        Some(ids)
    };
    facts.retain(|item| {
        let id = item["id"].as_str().unwrap_or_default();
        (filter_type.is_empty() || item["type"].as_str() == Some(filter_type))
            && (filter_ids.is_empty() || filter_ids.contains(&id))
            && tagged.as_ref().is_none_or(|ids| ids.contains(id))
            && (filter_text.is_empty()
                || text(item, "id").contains(&filter_text)
                || text(item, "name").contains(&filter_text)
                || item["certs"].as_array().is_some_and(|certs| {
                    certs.iter().any(|c| {
                        text(c, "issuer").contains(&filter_text)
                            || text(c, "subject").contains(&filter_text)
                    })
                }))
    });
    Ok(sort_packages(
        facts,
        sort_by,
        plain_rs::utils::sort_name::sort_name,
    ))
}

fn sort_packages(
    mut items: Vec<Value>,
    sort_by: &str,
    mut name_key: impl FnMut(&str) -> String,
) -> Vec<Value> {
    match sort_by {
        "SIZE_ASC" => items.sort_by_key(|item| item["size"].as_i64()),
        "SIZE_DESC" => items.sort_by_key(|item| std::cmp::Reverse(item["size"].as_i64())),
        "DATE_ASC" => items.sort_by_cached_key(|item| instant(item, "updatedAt")),
        "DATE_DESC" => {
            items.sort_by_cached_key(|item| std::cmp::Reverse(instant(item, "updatedAt")))
        }
        _ => {
            let mut keyed = items
                .into_iter()
                .map(|item| (name_key(item["name"].as_str().unwrap_or_default()), item))
                .collect::<Vec<_>>();
            keyed.sort_by(|a, b| {
                if sort_by == "NAME_DESC" {
                    b.0.cmp(&a.0)
                } else {
                    a.0.cmp(&b.0)
                }
            });
            return keyed.into_iter().map(|(_, item)| item).collect();
        }
    }
    items
}

#[cfg(test)]
#[path = "../../../tests/unit/services/packages/query.rs"]
mod tests;
