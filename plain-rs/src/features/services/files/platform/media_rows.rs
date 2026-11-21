use crate::features::enums::DataType;
use crate::features::services::files::file_task_media::Identity;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Row {
    id: String,
    path: String,
    provider_type: i32,
    mime_type: Option<String>,
}

pub(in crate::features::services::files) fn kind(provider_type: i32, mime: Option<&str>) -> Option<DataType> {
    match provider_type {
        1 => Some(DataType::Image),
        2 => Some(DataType::Audio),
        3 => Some(DataType::Video),
        _ if mime.is_some_and(plain_libs::media::documents::is_document_mime) => Some(DataType::Doc),
        _ => None,
    }
}

pub(super) fn identities(facts: plain_libs::serde_json::Value) -> plain_libs::anyhow::Result<Vec<Identity>> {
    let rows: Vec<Row> = plain_libs::serde_json::from_value(facts)?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let kind = kind(row.provider_type, row.mime_type.as_deref())?;
            Some(Identity {
                media_type: kind.kind(),
                file_id: row.id,
                path: row.path,
            })
        })
        .collect())
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/features/services/files/platform/media_rows.rs"]
mod tests;
