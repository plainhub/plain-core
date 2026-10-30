use crate::graphql::resolvers::files::types::FileSortBy;
use crate::host::query::{self, Provider};
use anyhow::Result;
use plain_rs::db::{Db, MediaFileRow};
use plain_rs::enums::DataType;
use rusqlite::{params_from_iter, types::Value};

fn selection(db: &Db, data_type: DataType, text: &str) -> Result<(String, Vec<Value>)> {
    selection_with_trash(db, data_type, text, false)
}

fn selection_with_trash(
    db: &Db,
    data_type: DataType,
    text: &str,
    from_trash: bool,
) -> Result<(String, Vec<Value>)> {
    let provider = match data_type {
        DataType::Audio => Provider::Audio,
        DataType::Video => Provider::Video,
        DataType::Image => Provider::Image,
        DataType::Doc => Provider::Doc,
        _ => anyhow::bail!("Unsupported media catalog type"),
    };
    let fields = query::fields(db, text)?;
    let filters = fields
        .iter()
        .filter(|field| data_type != DataType::Doc || field.name != "text")
        .cloned()
        .collect::<Vec<_>>();
    let mut plan = query::plan(
        provider,
        &filters,
        &serde_json::Value::Null,
        None,
        text.is_empty(),
    )?;
    if data_type == DataType::Doc {
        for field in fields.iter().filter(|field| field.name == "text") {
            let (predicate, args) = plain_rs::db::files::name_filter(&field.value);
            plan.clauses.push(predicate);
            plan.args.extend(args);
        }
    }
    Ok(selection_from_plan(data_type, plan, from_trash))
}

fn selection_from_plan(
    data_type: DataType,
    plan: query::Plan,
    from_trash: bool,
) -> (String, Vec<Value>) {
    let mut args = vec![
        Value::Integer(i64::from(data_type.kind())),
        Value::Integer(i64::from(from_trash || plan.trash.unwrap_or(false))),
    ];
    args.extend(plan.args.into_iter().map(Value::Text));
    let mut clauses = vec![
        "trashed=?".to_string(),
        "missing=0".into(),
        "is_available=1".into(),
    ];
    clauses.extend(plan.clauses);
    if plan.ids_column.is_some() {
        if plan.ids.is_empty() {
            clauses.push("0=1".into());
        } else {
            clauses.push(format!("_id IN ({})", vec!["?"; plan.ids.len()].join(",")));
            args.extend(plan.ids.into_iter().map(Value::Text));
        }
    }
    let sql = "WITH selected AS (SELECT *,id AS _id,path AS _data,file_name AS _display_name,size_bytes AS _size FROM file_catalog WHERE data_type=?)";
    (
        format!(
            "{sql} SELECT {{columns}} FROM selected WHERE {}",
            clauses.join(" AND ")
        ),
        args,
    )
}

pub(crate) fn rows(
    db: &Db,
    data_type: DataType,
    query: &str,
    sort: FileSortBy,
    limit: u32,
    offset: u32,
) -> Result<Vec<MediaFileRow>> {
    selected_rows(
        db,
        data_type,
        selection(db, data_type, query)?,
        sort,
        limit,
        offset,
    )
}

pub(crate) fn rows_with_plan(
    db: &Db,
    data_type: DataType,
    plan: query::Plan,
    sort: FileSortBy,
    limit: u32,
    offset: u32,
) -> Result<Vec<MediaFileRow>> {
    selected_rows(
        db,
        data_type,
        selection_from_plan(data_type, plan, false),
        sort,
        limit,
        offset,
    )
}

fn selected_rows(
    db: &Db,
    data_type: DataType,
    (sql, mut args): (String, Vec<Value>),
    sort: FileSortBy,
    limit: u32,
    offset: u32,
) -> Result<Vec<MediaFileRow>> {
    let order = match sort {
        FileSortBy::DateAsc => "updated_at ASC,file_id",
        FileSortBy::DateDesc => "updated_at DESC,file_id",
        FileSortBy::SizeAsc => "size_bytes ASC,file_id",
        FileSortBy::SizeDesc => "size_bytes DESC,file_id",
        FileSortBy::NameAsc if data_type == DataType::Audio => "title COLLATE NOCASE ASC,file_id",
        FileSortBy::NameAsc => "file_name COLLATE NOCASE ASC,file_id",
        FileSortBy::NameDesc if data_type == DataType::Audio => "title COLLATE NOCASE DESC,file_id",
        FileSortBy::NameDesc => "file_name COLLATE NOCASE DESC,file_id",
        FileSortBy::TakenAtDesc => "taken_at DESC,updated_at DESC,file_id",
    };
    args.extend([
        Value::Integer(i64::from(limit)),
        Value::Integer(i64::from(offset)),
    ]);
    let sql = format!(
        "{} ORDER BY {order} LIMIT ? OFFSET ?",
        sql.replace("{columns}", plain_rs::db::media_file::COLUMNS)
    );
    Ok(db.with_conn(|conn| {
        let mut statement = conn.prepare(&sql)?;
        statement
            .query_map(
                params_from_iter(args.iter()),
                plain_rs::db::media_file::from_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()
    })?)
}

pub(crate) fn count(db: &Db, data_type: DataType, query: &str) -> Result<i64> {
    let (sql, args) = selection(db, data_type, query)?;
    Ok(db.with_conn(|conn| {
        conn.query_row(
            &sql.replace("{columns}", "COUNT(*)"),
            params_from_iter(args.iter()),
            |row| row.get(0),
        )
    })?)
}

pub(crate) fn ids(
    db: &Db,
    data_type: DataType,
    query: &str,
    from_trash: bool,
) -> Result<Vec<String>> {
    let (sql, args) = selection_with_trash(db, data_type, query, from_trash)?;
    let sql = format!(
        "{} ORDER BY updated_at DESC,file_id",
        sql.replace("{columns}", "id")
    );
    Ok(db.with_conn(|conn| {
        let mut statement = conn.prepare(&sql)?;
        statement
            .query_map(params_from_iter(args.iter()), |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()
    })?)
}

pub(crate) fn document_extensions(db: &Db, query: &str) -> Result<Vec<(String, i32)>> {
    let (sql, args) = selection(db, DataType::Doc, query)?;
    Ok(db.with_conn(|conn| -> rusqlite::Result<_> {
        let mut statement = conn.prepare(&sql.replace("{columns}", "file_name"))?;
        let mut counts = std::collections::BTreeMap::new();
        let paths =
            statement.query_map(params_from_iter(args.iter()), |row| row.get::<_, String>(0))?;
        for path in paths {
            add_document_extension(&mut counts, &path?);
        }
        Ok(counts.into_iter().collect())
    })?)
}

pub(crate) fn add_document_extension(
    counts: &mut std::collections::BTreeMap<String, i32>,
    path: &str,
) {
    if let Some(extension) = std::path::Path::new(path)
        .extension()
        .filter(|value| !value.is_empty())
    {
        *counts
            .entry(extension.to_string_lossy().to_uppercase())
            .or_default() += 1;
    }
}

pub(crate) fn bucket_item_facts(db: &Db, data_type: DataType) -> Result<Vec<serde_json::Value>> {
    let (sql, args) = selection(db, data_type, "")?;
    let sql = format!(
        "{} ORDER BY updated_at DESC,file_id",
        sql.replace("{columns}", "bucket_id,path,size_bytes")
    );
    Ok(db.with_conn(|conn| {
        let mut statement = conn.prepare(&sql)?;
        statement
            .query_map(params_from_iter(args.iter()), |row| {
                let id: String = row.get(0)?;
                let path: String = row.get(1)?;
                let size: i64 = row.get(2)?;
                let name = std::path::Path::new(&path)
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or(&id);
                Ok(serde_json::json!({
                    "id":id,"name":name,"size":size,"path":path
                }))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
    })?)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/media_catalog.rs"]
mod tests;
