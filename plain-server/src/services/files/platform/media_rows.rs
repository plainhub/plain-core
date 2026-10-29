use super::super::file_task_media::Identity;
use plain_rs::enums::DataType;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Row {
    id: String,
    path: String,
    provider_type: i32,
    mime_type: Option<String>,
}

pub(super) fn identities(facts: serde_json::Value) -> anyhow::Result<Vec<Identity>> {
    let rows: Vec<Row> = serde_json::from_value(facts)?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let kind = match row.provider_type {
                1 => DataType::Image,
                2 => DataType::Audio,
                3 => DataType::Video,
                _ if row
                    .mime_type
                    .as_deref()
                    .is_some_and(plain_rs::media::documents::is_document_mime) =>
                {
                    DataType::Doc
                }
                _ => return None,
            };
            Some(Identity {
                media_type: kind.kind(),
                file_id: row.id,
                path: row.path,
            })
        })
        .collect())
}

#[cfg(test)]
#[path = "../../../../tests/unit/services/files/platform/media_rows.rs"]
mod tests;
