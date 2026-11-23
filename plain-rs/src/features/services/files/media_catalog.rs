use crate::features::enums::DataType;
use crate::features::prefs::Prefs;
use crate::features::prefs::settings::MediaSortBy;
use crate::features::services::files::provider_query::{self as query, Provider};
use crate::infrastructure::db::Db;
use crate::infrastructure::db::MediaFileRow;
use plain_libs::anyhow::Result;
use plain_libs::rusqlite::{params_from_iter, types::Value};

fn selection(db: &Db, prefs: &Prefs, data_type: DataType, text: &str) -> Result<(String, Vec<Value>)> {
    selection_with_trash(db, prefs, data_type, text, false)
}

fn selection_with_trash(
    db: &Db,
    prefs: &Prefs,
    data_type: DataType,
    text: &str,
    from_trash: bool,
) -> Result<(String, Vec<Value>)> {
    let provider = match data_type {
        DataType::Audio => Provider::Audio,
        DataType::Video => Provider::Video,
        DataType::Image => Provider::Image,
        DataType::Doc => Provider::Doc,
        _ => plain_libs::anyhow::bail!("Unsupported media catalog type"),
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
        &plain_libs::serde_json::Value::Null,
        None,
        text.is_empty(),
    )?;
    if data_type == DataType::Doc {
        for field in fields.iter().filter(|field| field.name == "text") {
            let (predicate, args) = crate::infrastructure::db::files::name_filter(&field.value);
            plan.clauses.push(predicate);
            plan.args.extend(args);
        }
    }
    plan.exclude_media_directories(prefs)?;
    Ok(selection_from_plan(data_type, plan, from_trash))
}

fn selection_from_plan(
    data_type: DataType,
    plan: query::Plan,
    from_trash: bool,
) -> (String, Vec<Value>) {
    let kind = match data_type {
        DataType::Audio => "kind='AUDIO'",
        DataType::Video => "kind='VIDEO'",
        DataType::Image => "kind='IMAGE'",
        DataType::Doc => "kind='DOC'",
        DataType::File => "kind IN ('FILE','DIRECTORY')",
        _ => "0",
    };
    let mut args = plan.args.into_iter().map(Value::Text).collect::<Vec<_>>();
    let mut clauses = vec![
        if from_trash || plan.trash.unwrap_or(false) {
            "trashed_at IS NOT NULL"
        } else {
            "trashed_at IS NULL"
        }
        .to_string(),
        "missing=0".into(),
        "indexed=1".into(),
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
    let sql = format!(
        "WITH selected AS (SELECT *,id AS _id,path AS _data,file_name AS _display_name,size_bytes AS _size FROM file_catalog WHERE {kind})"
    );
    (
        format!(
            "{sql} SELECT {{columns}} FROM selected WHERE {}",
            clauses.join(" AND ")
        ),
        args,
    )
}

pub fn rows(
    db: &Db,
    prefs: &Prefs,
    data_type: DataType,
    query: &str,
    sort: MediaSortBy,
    limit: u32,
    offset: u32,
) -> Result<Vec<MediaFileRow>> {
    selected_rows(
        db,
        data_type,
        selection(db, prefs, data_type, query)?,
        sort,
        limit,
        offset,
    )
}

pub fn rows_with_plan(
    db: &Db,
    prefs: &Prefs,
    data_type: DataType,
    mut plan: query::Plan,
    sort: MediaSortBy,
    limit: u32,
    offset: u32,
) -> Result<Vec<MediaFileRow>> {
    plan.exclude_media_directories(prefs)?;
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
    sort: MediaSortBy,
    limit: u32,
    offset: u32,
) -> Result<Vec<MediaFileRow>> {
    let order = match sort {
        MediaSortBy::UpdatedAtAsc => "updated_at ASC,file_id",
        MediaSortBy::UpdatedAtDesc => "updated_at DESC,file_id",
        MediaSortBy::SizeAsc => "size_bytes ASC,file_id",
        MediaSortBy::SizeDesc => "size_bytes DESC,file_id",
        MediaSortBy::TitleAsc => "title COLLATE NOCASE ASC,file_id",
        MediaSortBy::NameAsc => "file_name COLLATE NOCASE ASC,file_id",
        MediaSortBy::TitleDesc => "title COLLATE NOCASE DESC,file_id",
        MediaSortBy::NameDesc => "file_name COLLATE NOCASE DESC,file_id",
        MediaSortBy::TakenAtDesc => "capture_at DESC,updated_at DESC,file_id",
    };
    args.extend([
        Value::Integer(i64::from(limit)),
        Value::Integer(i64::from(offset)),
    ]);
    let sql = format!(
        "{} ORDER BY {order} LIMIT ? OFFSET ?",
        sql.replace("{columns}", crate::infrastructure::db::media_file::COLUMNS)
    );
    Ok(db.with_conn(|conn| {
        let mut statement = conn.prepare(&sql)?;
        statement
            .query_map(
                params_from_iter(args.iter()),
                crate::infrastructure::db::media_file::from_row,
            )?
            .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
    })?)
}

pub fn count(db: &Db, prefs: &Prefs, data_type: DataType, query: &str) -> Result<i64> {
    let (sql, args) = selection(db, prefs, data_type, query)?;
    Ok(db.with_conn(|conn| {
        conn.query_row(
            &sql.replace("{columns}", "COUNT(*)"),
            params_from_iter(args.iter()),
            |row| row.get(0),
        )
    })?)
}

pub fn ids(db: &Db, prefs: &Prefs, data_type: DataType, query: &str, from_trash: bool) -> Result<Vec<String>> {
    let (sql, args) = selection_with_trash(db, prefs, data_type, query, from_trash)?;
    let sql = format!(
        "{} ORDER BY updated_at DESC,file_id",
        sql.replace("{columns}", "id")
    );
    Ok(db.with_conn(|conn| {
        let mut statement = conn.prepare(&sql)?;
        statement
            .query_map(params_from_iter(args.iter()), |row| row.get(0))?
            .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
    })?)
}

pub fn document_extensions(db: &Db, prefs: &Prefs, query: &str) -> Result<Vec<(String, i32)>> {
    let (sql, args) = selection(db, prefs, DataType::Doc, query)?;
    Ok(db.with_conn(|conn| -> plain_libs::rusqlite::Result<_> {
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

pub fn add_document_extension(counts: &mut std::collections::BTreeMap<String, i32>, path: &str) {
    if let Some(extension) = std::path::Path::new(path)
        .extension()
        .filter(|value| !value.is_empty())
    {
        *counts
            .entry(extension.to_string_lossy().to_uppercase())
            .or_default() += 1;
    }
}

fn bucket_selection(db: &Db, prefs: &Prefs, data_type: DataType) -> Result<(String, Vec<Value>)> {
    let (sql, args) = selection(db, prefs, data_type, "")?;
    let selection = sql.replace(
        "{columns}",
        "bucket_id,file_id,path,size_bytes,ROW_NUMBER() OVER (PARTITION BY bucket_id ORDER BY updated_at DESC,file_id) AS position",
    );
    Ok((
        format!(
            "WITH ranked AS ({selection}), covers AS (
                SELECT bucket_id,COUNT(*) AS item_count,SUM(size_bytes) AS size,
                MAX(CASE WHEN position=1 THEN file_id END) AS id1,
                MAX(CASE WHEN position=2 THEN file_id END) AS id2,
                MAX(CASE WHEN position=3 THEN file_id END) AS id3,
                MAX(CASE WHEN position=4 THEN file_id END) AS id4,
                MAX(CASE WHEN position=1 THEN path END) AS path1,
                MAX(CASE WHEN position=2 THEN path END) AS path2,
                MAX(CASE WHEN position=3 THEN path END) AS path3,
                MAX(CASE WHEN position=4 THEN path END) AS path4
                FROM ranked GROUP BY bucket_id
             )
             SELECT bucket_id,item_count,size,
                path1,f1.updated_at,f1.thumb_hash,path2,f2.updated_at,f2.thumb_hash,
                path3,f3.updated_at,f3.thumb_hash,path4,f4.updated_at,f4.thumb_hash
             FROM covers
             LEFT JOIN files f1 ON f1.id=id1 LEFT JOIN files f2 ON f2.id=id2
             LEFT JOIN files f3 ON f3.id=id3 LEFT JOIN files f4 ON f4.id=id4"
        ),
        args,
    ))
}

pub fn buckets(db: &Db, prefs: &Prefs, data_type: DataType) -> Result<Vec<super::media_buckets::Bucket>> {
    let (sql, args) = bucket_selection(db, prefs, data_type)?;
    let mut buckets = db.with_conn(|conn| {
        let mut statement = conn.prepare(&sql)?;
        statement
            .query_map(params_from_iter(args.iter()), |row| {
                let id: String = row.get(0)?;
                let name = std::path::Path::new(&id)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&id)
                    .to_owned();
                let mut cover_items = Vec::with_capacity(4);
                for column in [3, 6, 9, 12] {
                    if let Some(path) = row.get::<_, Option<String>>(column)? {
                        cover_items.push(super::media_buckets::CoverItem {
                            path,
                            updated_at: row.get(column + 1)?,
                            thumb_hash: row.get(column + 2)?,
                        });
                    }
                }
                Ok(super::media_buckets::Bucket {
                    sort_name: plain_libs::utils::sort_name::sort_name(&name),
                    id,
                    name,
                    item_count: row.get::<_, i64>(1)? as u64,
                    size: row.get(2)?,
                    cover_items,
                })
            })?
            .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
    })?;
    super::media_buckets::sort(&mut buckets);
    Ok(buckets)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/media_catalog.rs"]
mod tests;
