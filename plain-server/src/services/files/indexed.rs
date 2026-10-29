use crate::graphql::resolvers::files::types::FileSortBy;
use crate::host::{
    bridge::Host,
    query::{self, Provider},
};
use anyhow::Result;
use plain_rs::db::Db;
use rusqlite::{params_from_iter, types::Value as SqlValue};
use serde_json::{Value, json};

pub(crate) async fn platform(
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
            .map_err(anyhow::Error::msg)?;
        Some(match fact {
            Value::Null => "-1".to_owned(),
            Value::String(id) => id,
            _ => anyhow::bail!("invalid file index identity"),
        })
    } else {
        None
    };
    let plan = query::plan(
        Provider::File,
        &fields,
        &Value::Null,
        parent_id.as_deref(),
        query.is_empty(),
    )?;
    host.call(
        if count_only {
            "systemFileIndexCount"
        } else {
            "systemFileIndexRows"
        },
        json!({"plan":plan,"offset":offset.max(0),"limit":limit.max(0),"sortBy":sort.as_str()}),
    )
    .await
    .map_err(anyhow::Error::msg)
}

pub(crate) fn native(
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
        .filter(|field| field.name != "text")
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
        let (predicate, values) = plain_rs::db::files::name_filter(text);
        clauses.push(predicate);
        args.extend(values.into_iter().map(SqlValue::Text));
    }
    if plan.ids_column.is_some() {
        clauses.push(format!("_id IN ({})", vec!["?"; plan.ids.len()].join(",")));
        args.extend(plan.ids.into_iter().map(SqlValue::Text));
    }
    let selection = format!(
        "WITH indexed AS (SELECT *,id AS _id,file_name AS _display_name,size_bytes AS _size,CASE WHEN length(path)=length(file_name)+1 THEN '/' ELSE substr(path,1,length(path)-length(file_name)-1) END AS parent FROM file_catalog WHERE trashed=0 AND missing=0 AND is_available=1) SELECT {{columns}} FROM indexed{}",
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
            FileSortBy::DateAsc => "updated_at ASC,file_id", FileSortBy::DateDesc => "updated_at DESC,file_id",
            FileSortBy::NameDesc => "file_name COLLATE NOCASE DESC,file_id", _ => "file_name COLLATE NOCASE ASC,file_id",
        };
        let sql = format!("{} ORDER BY {order} LIMIT ? OFFSET ?",selection.replace("{columns}","id,file_name,path,size_bytes,created_at,updated_at,kind"));
        args.extend([SqlValue::Integer(limit.max(0).into()),SqlValue::Integer(offset.max(0).into())]);
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(args.iter()), |row| {
            let created: String = row.get(4)?;
            let updated: String = row.get(5)?;
            Ok(json!({"mediaId":row.get::<_,String>(0)?,"name":row.get::<_,String>(1)?,"path":row.get::<_,String>(2)?,"size":row.get::<_,i64>(3)?,"createdAt":millis(&created,4)?,"updatedAt":millis(&updated,5)?,"isDir":row.get::<_,String>(6)?=="DIRECTORY","childCount":0,"permission":""}))
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Value::Array(rows))
    }).map_err(Into::into)
}

fn millis(value: &str, column: usize) -> rusqlite::Result<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| date.timestamp_millis())
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                column,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/indexed.rs"]
mod tests;
