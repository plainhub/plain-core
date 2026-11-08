use super::*;
use crate::features::enums::DataType;
use crate::infrastructure::db::MediaFileMetadata;
use std::sync::{Arc, Mutex};

fn seed(db: &Db, path: &str, size: i64) {
    db.record_media_files(&[MediaFileMetadata {
        album: String::new(),
        album_artist: String::new(),

        data_type: DataType::Doc,
        path: path.into(),
        title: "Catalog title".into(),
        artist: String::new(),
        bucket_id: String::new(),
        size_bytes: size,
        duration_ms: 0,
        cover_image_id: None,
        width: 0,
        height: 0,
        rotation: 0,
        taken_at: None,
        created_at: "2026-10-01T00:00:00.123Z".into(),
        updated_at: "2026-10-01T01:00:00.456Z".into(),
    }])
    .unwrap();
}

#[test]
fn native_search_uses_indexed_names_literal_filters_parent_and_matching_counts() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    seed(&db, "/docs/100%.pdf", 30);
    seed(&db, "/docs/other.pdf", 10);
    seed(&db, "/else/100%.pdf", 20);
    let result = native(
        &db,
        "text:100% parent:/docs",
        0,
        10,
        FileSortBy::NameAsc,
        false,
    )
    .unwrap();
    assert_eq!(result.as_array().unwrap().len(), 1);
    assert_eq!(result[0]["path"], "/docs/100%.pdf");
    assert_eq!(result[0]["createdAt"], 1790812800123_i64);
    assert_eq!(
        native(
            &db,
            "text:100% parent:/docs",
            0,
            0,
            FileSortBy::NameAsc,
            true
        )
        .unwrap(),
        json!(1)
    );
    assert_eq!(
        native(&db, "", 1, 1, FileSortBy::SizeAsc, false).unwrap()[0]["size"],
        20
    );
    assert!(native(&db, "unknown:value", 0, 10, FileSortBy::NameAsc, false).is_err());
}

#[tokio::test]
async fn platform_query_passes_prepared_conditions_and_only_resolves_parent_when_present() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let reply = host.clone();
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let observed = calls.clone();
    let worker = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let method = request["method"].as_str().unwrap();
            observed.lock().unwrap().push(method.to_owned());
            let result = match method {
                "systemFileIndexId" => {
                    assert_eq!(request["params"]["path"], "/docs");
                    json!("9")
                }
                "systemFileIndexRows" => {
                    assert!(request["params"].get("query").is_none());
                    assert_eq!(request["params"]["plan"]["args"], json!([".%", "100\\%"]));
                    assert_eq!(request["params"]["limit"], 10);
                    json!([])
                }
                "systemFileIndexCount" => {
                    assert!(
                        request["params"]["plan"]["args"]
                            .as_array()
                            .unwrap()
                            .contains(&json!("9"))
                    );
                    json!(3)
                }
                other => panic!("unexpected host method {other}"),
            };
            reply
                .reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    assert_eq!(
        platform(&db, &host, "text:100%", 0, 10, FileSortBy::NameAsc, false)
            .await
            .unwrap(),
        json!([])
    );
    assert_eq!(
        platform(&db, &host, "parent:/docs", 0, 0, FileSortBy::NameAsc, true)
            .await
            .unwrap(),
        json!(3)
    );
    assert_eq!(
        *calls.lock().unwrap(),
        [
            "systemFileIndexRows",
            "systemFileIndexId",
            "systemFileIndexCount"
        ]
    );
    worker.abort();
}

#[test]
fn scope_changes_hide_existing_records_and_preserve_file_identity_and_user_state() {
    use crate::features::files::index_scope::{Defaults, Scope};
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    seed(&db, "/docs/private/report.pdf", 10);
    seed(&db, "/docs/private-notes/report.pdf", 20);
    seed(&db, "/outside/report.pdf", 30);
    let before = db
        .with_conn(|conn| {
            conn.execute("UPDATE files SET favorited=1", [])?;
            conn.query_row(
                "SELECT file_id FROM file_catalog WHERE path='/docs/private/report.pdf'",
                [],
                |row| row.get::<_, String>(0),
            )
        })
        .unwrap();
    let mut scope = Scope {
        volumes: Vec::new(),
        directories: vec!["/docs".into()],
        exclusions: vec!["/docs/private".into()],
        defaults: Defaults {
            excluded_directories: vec![],
            excluded_names: vec![],
        },
    };
    db.update_index_visibility(&scope, None).unwrap();
    assert_eq!(
        native(&db, "text:report", 0, 10, FileSortBy::NameAsc, false)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        native(&db, "text:report", 0, 0, FileSortBy::NameAsc, true).unwrap(),
        json!(1)
    );
    assert_eq!(db.media_file_count(DataType::Doc).unwrap(), 1);
    let stored = db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT favorited,missing FROM files WHERE id=?1",
                [&before],
                |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
            )
        })
        .unwrap();
    assert_eq!(stored, (true, false));
    scope.exclusions.clear();
    db.update_index_visibility(&scope, None).unwrap();
    assert_eq!(
        native(&db, "text:report", 0, 0, FileSortBy::NameAsc, true).unwrap(),
        json!(2)
    );
    assert_eq!(db.media_file_count(DataType::Doc).unwrap(), 2);
    assert_eq!(
        db.with_conn(|conn| conn.query_row(
            "SELECT file_id FROM file_catalog WHERE path='/docs/private/report.pdf'",
            [],
            |row| row.get::<_, String>(0)
        ))
        .unwrap(),
        before
    );
}

#[test]
fn indexed_search_combines_subtree_kind_and_modification_time() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    seed(&db, "/docs/child/report.pdf", 10);
    seed(&db, "/docs-other/report.pdf", 20);
    let query = "path:/docs kind:DOC updated_at:>=2026-10-01T00:00:00Z";
    assert_eq!(
        native(&db, query, 0, 0, FileSortBy::NameAsc, true).unwrap(),
        json!(1)
    );
    assert_eq!(
        native(&db, query, 0, 10, FileSortBy::NameAsc, false).unwrap()[0]["path"],
        "/docs/child/report.pdf"
    );
    assert!(native(&db, "path:relative", 0, 10, FileSortBy::NameAsc, false).is_err());
    assert!(native(&db, "updated_at:invalid", 0, 10, FileSortBy::NameAsc, false).is_err());
}
