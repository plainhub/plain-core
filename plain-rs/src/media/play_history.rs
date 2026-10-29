use crate::db::{Db, MediaPlayHistoryRow, media_play_history};
use crate::enums::DataType;
use crate::error::{DomainError, DomainResult};
use crate::utils::search_dsl::FilterField;
use rusqlite::{params_from_iter, types::Value};

pub fn matching(db: &Db, fields: &[FilterField]) -> DomainResult<Vec<MediaPlayHistoryRow>> {
    let mut conditions = Vec::new();
    let mut values = Vec::new();
    for field in fields {
        match field.name.as_str() {
            "type" | "ids" => {
                let operator = match field.op.as_str() {
                    "" | "=" | "in" => "IN",
                    "!=" | "nin" | "NOT" => "NOT IN",
                    _ => return Err(invalid("Invalid media history filter operator")),
                };
                let entries = field.value.split(',').collect::<Vec<_>>();
                let (column, value) = if field.name == "type" {
                    let kinds = entries
                        .iter()
                        .map(|value| match *value {
                            "AUDIO" => Ok(DataType::Audio.kind()),
                            "VIDEO" => Ok(DataType::Video.kind()),
                            _ => Err(invalid("Media history type must be AUDIO or VIDEO")),
                        })
                        .collect::<DomainResult<Vec<_>>>()?;
                    (
                        "media_type",
                        serde_json::to_string(&kinds)
                            .map_err(|error| invalid(&error.to_string()))?,
                    )
                } else {
                    (
                        "file_id",
                        serde_json::to_string(&entries)
                            .map_err(|error| invalid(&error.to_string()))?,
                    )
                };
                conditions.push(format!(
                    "{column} {operator} (SELECT value FROM json_each(?))"
                ));
                values.push(Value::Text(value));
            }
            "played_at" | "position_ms" | "play_count" => {
                let operator = match field.op.as_str() {
                    "" | "=" => "=",
                    "!=" => "!=",
                    ">" => ">",
                    ">=" => ">=",
                    "<" => "<",
                    "<=" => "<=",
                    _ => return Err(invalid("Invalid media history filter operator")),
                };
                let value =
                    if field.name == "played_at" {
                        Value::Text(crate::utils::dbtime::normalize(&field.value)?)
                    } else {
                        Value::Integer(field.value.parse().map_err(|_| {
                            invalid("Media history numeric filter requires an integer")
                        })?)
                    };
                conditions.push(format!("{} {operator} ?", field.name));
                values.push(value);
            }
            _ => {
                return Err(invalid(&format!(
                    "Unknown media history filter: {}",
                    field.name
                )));
            }
        }
    }
    let mut query = format!(
        "SELECT {} FROM media_play_history",
        media_play_history::COLUMNS
    );
    if !conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&conditions.join(" AND "));
    }
    query.push_str(" ORDER BY played_at DESC,rowid DESC");
    Ok(
        db.with_conn(|conn| -> rusqlite::Result<Vec<MediaPlayHistoryRow>> {
            conn.prepare(&query)?
                .query_map(params_from_iter(values), media_play_history::from_row)?
                .collect()
        })?,
    )
}

pub fn matches_text(fields: &[FilterField], values: &[&str]) -> bool {
    fields.iter().all(|field| {
        let needle = field.value.to_lowercase();
        let matched = values
            .iter()
            .any(|value| value.to_lowercase().contains(&needle));
        if matches!(field.op.as_str(), "!=" | "nin" | "NOT") {
            !matched
        } else {
            matched
        }
    })
}

pub fn record(db: &Db, media_type: DataType, file_id: &str, position_ms: i64) -> DomainResult<()> {
    validate(media_type, file_id, position_ms)?;
    Ok(db.media_play_history_record(media_type, file_id, position_ms)?)
}

pub fn save(db: &Db, media_type: DataType, file_id: &str, position_ms: i64) -> DomainResult<()> {
    validate(media_type, file_id, position_ms)?;
    Ok(db.media_play_history_save(media_type, file_id, position_ms)?)
}

fn validate(media_type: DataType, file_id: &str, position_ms: i64) -> DomainResult<()> {
    if !matches!(media_type, DataType::Audio | DataType::Video) {
        return Err(invalid("Media history type must be AUDIO or VIDEO"));
    }
    if file_id.trim().is_empty() || position_ms < 0 {
        return Err(invalid("Invalid media play history"));
    }
    Ok(())
}

fn invalid(message: &str) -> DomainError {
    DomainError::Other(message.into())
}

#[cfg(test)]
#[path = "../../tests/unit/media/play_history.rs"]
mod tests;
