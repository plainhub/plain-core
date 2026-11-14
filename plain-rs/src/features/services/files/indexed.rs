use crate::features::prefs::settings::FileSortBy;
use crate::features::services::files::provider_query as query;
use crate::features::services::files::provider_query::Provider;
use crate::infrastructure::db::Db;
use crate::infrastructure::host::bridge::Host;
use plain_libs::anyhow::Result;
use plain_libs::rusqlite::{params_from_iter, types::Value as SqlValue};
use plain_libs::serde_json::{Value, json};

pub async fn platform(
    db: &Db,
    host: &Host,
    query: &str,
    offset: i32,
    limit: i32,
    sort: FileSortBy,
    count_only: bool,
) -> Result<Value> {
    let fields = query::fields(db, query)?;
    let parent = fields
        .iter()
        .find(|field| field.name == "parent")
        .map(|field| field.value.as_str());
    let parent_id = if let Some(path) = parent {
        let fact = host
            .call("systemFileIndexId", json!({"path":path}))
            .await
            .map_err(plain_libs::anyhow::Error::msg)?;
        Some(match fact {
            Value::Null => "-1".to_owned(),
            Value::String(id) => id,
            _ => plain_libs::anyhow::bail!("invalid file index identity"),
        })
    } else {
        None
    };
    let mut plan = query::plan(
        Provider::File,
        &fields,
        &Value::Null,
        parent_id.as_deref(),
        query.is_empty(),
    )?;
    query::index_scope(&mut plan, db)?;
    host.call(
        if count_only {
            "systemFileIndexCount"
        } else {
            "systemFileIndexRows"
        },
        json!({"plan":plan,"offset":offset.max(0),"limit":limit.max(0),"sortBy":sort.as_str()}),
    )
    .await
    .map_err(plain_libs::anyhow::Error::msg)
}

pub fn native(
    db: &Db,
    query: &str,
    offset: i32,
    limit: i32,
    sort: FileSortBy,
    count_only: bool,
) -> Result<Value> {
    let fields = query::fields(db, query)?;
    let texts = fields
        .iter()
        .filter(|field| field.name == "text")
        .map(|field| field.value.as_str())
        .collect::<Vec<_>>();
    let filters = fields
        .iter()
        .filter(|field| {
            !matches!(
                field.name.as_str(),
                "text" | "path" | "root_path" | "updated_at" | "kind"
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let parent = fields
        .iter()
        .find(|field| field.name == "parent")
        .map(|field| field.value.as_str());
    let plan = query::plan(
        Provider::File,
        &filters,
        &Value::Null,
        parent,
        query.is_empty(),
    )?;
    let mut clauses = plan.clauses;
    let mut args = plan
        .args
        .into_iter()
        .map(SqlValue::Text)
        .collect::<Vec<_>>();
    for text in texts {
        let (predicate, values) = crate::infrastructure::db::files::name_filter(text);
        clauses.push(predicate);
        args.extend(values.into_iter().map(SqlValue::Text));
    }
    for field in &fields {
        match field.name.as_str() {
            "path" | "root_path" => {
                let root = crate::features::files::index_scope::normalized(std::path::Path::new(
                    &field.value,
                ));
                plain_libs::anyhow::ensure!(
                    std::path::Path::new(&root).is_absolute(),
                    "Absolute search directory required"
                );
                let prefix = format!("{}/", root.trim_end_matches('/'));
                clauses.push("(path=? OR substr(path,1,length(?))=?)".into());
                args.extend([root, prefix.clone(), prefix].map(SqlValue::Text));
            }
            "kind" => {
                let kind = field.value.to_ascii_uppercase();
                plain_libs::anyhow::ensure!(
                    ["FILE", "DIRECTORY", "IMAGE", "VIDEO", "AUDIO", "DOC"]
                        .contains(&kind.as_str()),
                    "Unknown file kind"
                );
                clauses.push("kind=?".into());
                args.push(SqlValue::Text(kind));
            }
            "updated_at" => {
                let (op, value) = query::comparison(field);
                plain_libs::anyhow::ensure!(
                    ["=", "!=", ">", ">=", "<", "<="].contains(&op),
                    "Invalid modification time comparison"
                );
                let value = plain_libs::chrono::DateTime::parse_from_rfc3339(value)?
                    .with_timezone(&plain_libs::chrono::Utc)
                    .to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true);
                clauses.push(format!("updated_at {op} ?"));
                args.push(SqlValue::Text(value));
            }
            _ => {}
        }
    }
    if plan.ids_column.is_some() {
        clauses.push(format!("_id IN ({})", vec!["?"; plan.ids.len()].join(",")));
        args.extend(plan.ids.into_iter().map(SqlValue::Text));
    }
    let selection = format!(
        "WITH indexed AS (SELECT *,id AS _id,file_name AS _display_name,size_bytes AS _size,CASE WHEN length(path)=length(file_name)+1 THEN '/' ELSE substr(path,1,length(path)-length(file_name)-1) END AS parent FROM file_catalog WHERE trashed=0 AND missing=0 AND indexed=1 AND is_available=1) SELECT {{columns}} FROM indexed{}",
        if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        }
    );
    db.with_conn(|conn| {
        if count_only {
            return conn.query_row(&selection.replace("{columns}","COUNT(*)"), params_from_iter(args.iter()), |row| row.get::<_,i64>(0)).map(|count| json!(count));
        }
        let order = match sort {
            FileSortBy::SizeAsc => "size_bytes ASC,file_id", FileSortBy::SizeDesc => "size_bytes DESC,file_id",
            FileSortBy::UpdatedAtAsc => "updated_at ASC,file_id", FileSortBy::UpdatedAtDesc => "updated_at DESC,file_id",
            FileSortBy::NameDesc => "file_name COLLATE NOCASE DESC,file_id", FileSortBy::NameAsc => "file_name COLLATE NOCASE ASC,file_id",
        };
        let sql = format!("{} ORDER BY {order} LIMIT ? OFFSET ?",selection.replace("{columns}","id,file_name,path,size_bytes,created_at,updated_at,kind"));
        args.extend([SqlValue::Integer(limit.max(0).into()),SqlValue::Integer(offset.max(0).into())]);
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(args.iter()), |row| {
            let created: String = row.get(4)?;
            let updated: String = row.get(5)?;
            Ok(json!({"mediaId":row.get::<_,String>(0)?,"name":row.get::<_,String>(1)?,"path":row.get::<_,String>(2)?,"size":row.get::<_,i64>(3)?,"createdAt":millis(&created,4)?,"updatedAt":millis(&updated,5)?,"isDir":row.get::<_,String>(6)?=="DIRECTORY","childCount":0,"permission":""}))
        })?.collect::<plain_libs::rusqlite::Result<Vec<_>>>()?;
        Ok(Value::Array(rows))
    }).map_err(Into::into)
}

fn millis(value: &str, column: usize) -> plain_libs::rusqlite::Result<i64> {
    plain_libs::chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| date.timestamp_millis())
        .map_err(|error| {
            plain_libs::rusqlite::Error::FromSqlConversionFailure(
                column,
                plain_libs::rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/indexed.rs"]
mod tests;
