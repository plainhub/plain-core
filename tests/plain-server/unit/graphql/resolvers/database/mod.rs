use std::sync::Arc;
use super::*;
use crate::test_support::schema::SchemaFixture as PublicSchema;
use plain_libs::serde_json::json;
fn fixture() -> (plain_libs::temporary::TempDir, PublicSchema) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = plain_rs::infrastructure::data_paths::DataPaths::new(dir.path()).database();
    let prefs = Arc::new(
        plain_rs::features::prefs::Prefs::load(&plain_rs::features::prefs::default_path(
            dir.path(),
        ))
        .unwrap(),
    );
    prefs.set("developer_mode", true).unwrap();
    let db = Arc::new(plain_rs::infrastructure::db::Db::open(&path).unwrap());
    db.with_conn(|conn| conn.execute_batch("CREATE TABLE test_rows (key TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT 'untitled', size INTEGER, ratio REAL, bytes BLOB); INSERT INTO test_rows VALUES('a','First',42,1.5,NULL),('b','Second',43,2.5,NULL);").unwrap());
    let host = Arc::new(plain_rs::infrastructure::host::bridge::Host::with_startup_config(
        plain_rs::infrastructure::host::startup::StartupConfig {
            debug: false,
            ..plain_rs::infrastructure::platform::startup::config()
        },
    ).unwrap());
    let (events, _) = tokio::sync::broadcast::channel(16);
    let schema =
        crate::test_support::schema::build(host, events, prefs, db, dir.path().to_path_buf());
    (dir, schema)
}

#[tokio::test]
async fn owned_database_browser_works_without_a_connected_host() {
    let (dir, schema) = fixture();
    let response=schema.execute(r#"{dbPath dbTables{name createSql columns{name dataType notNull defaultValue primaryKey}} dbTableRowCount(table:"test_rows") dbTableRows(table:"test_rows",offset:1,limit:1)}"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(
        data["dbPath"],
        dir.path()
            .join("plain.db")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    let tables = data["dbTables"].as_array().unwrap();
    assert!(
        !tables
            .iter()
            .any(|table| table["name"] == "replay_receipts")
    );
    let table = tables
        .iter()
        .find(|table| table["name"] == "test_rows")
        .unwrap();
    assert_eq!(
        table["createSql"],
        "CREATE TABLE test_rows (key TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT 'untitled', size INTEGER, ratio REAL, bytes BLOB)"
    );
    assert_eq!(data["dbTableRowCount"], 2);
    let columns = table["columns"].as_array().unwrap();
    assert_eq!(columns[0]["dataType"], "TEXT");
    assert_eq!(columns[0]["primaryKey"], true);
    assert_eq!(columns[1]["notNull"], true);
    assert_eq!(columns[1]["defaultValue"], "'untitled'");
    assert_eq!(columns[2]["dataType"], "INTEGER");
    assert_eq!(columns[3]["dataType"], "REAL");
    assert_eq!(columns[4]["dataType"], "BLOB");
    let rows = data["dbTableRows"].as_array().unwrap();
    let row = &rows[0];
    assert_eq!(row["key"], "b");
    assert_eq!(row["size"], "43");
    let response=schema.execute(r#"mutation {createDbTableRow(table:"test_rows",row:{key:"c",title:"Third"}) deleteDbTableRows(table:"test_rows",keys:[{key:"a"},{key:"b"}]){affectedCount}}"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap(),
        json!({"createDbTableRow":true,"deleteDbTableRows":{"affectedCount":2}})
    );
    let response = schema
        .execute(r#"{dbTableRowCount(table:"test_rows")}"#)
        .await;
    assert_eq!(response.data.into_json().unwrap()["dbTableRowCount"], 1);
}
#[tokio::test]
async fn invalid_row_returns_an_error_and_empty_delete_affects_zero() {
    let (_dir, schema) = fixture();
    let response = schema
        .execute(r#"mutation {createDbTableRow(table:"test_rows",row:{})}"#)
        .await;
    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "row must not be empty");
    let response = schema
        .execute(r#"mutation {deleteDbTableRows(table:"test_rows",keys:[]){affectedCount}}"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap(),
        json!({"deleteDbTableRows":{"affectedCount":0}})
    );
}

#[tokio::test]
async fn json_null_is_stored_as_sql_null_and_null_text_is_preserved() {
    let (_dir, schema) = fixture();
    let response = schema
        .execute(r#"mutation {createDbTableRow(table:"test_rows",row:{key:"c",title:"null",size:null,bytes:null})}"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["createDbTableRow"], true);
    let db = schema
        .data::<Arc<plain_rs::infrastructure::db::Db>>()
        .unwrap();
    let stored: (bool, bool, String) = db.with_conn(|conn| {
        conn.query_row(
            "SELECT size IS NULL, bytes IS NULL, title FROM test_rows WHERE key='c'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    });
    assert_eq!(stored, (true, true, "null".into()));
}

#[tokio::test]
async fn database_write_errors_are_returned_without_changing_rows() {
    let (_dir, schema) = fixture();
    for (document, message) in [
        (
            r#"mutation {createDbTableRow(table:"test_rows",row:{key:"a",title:"Duplicate"})}"#,
            "UNIQUE constraint failed",
        ),
        (
            r#"mutation {createDbTableRow(table:"test_rows",row:{key:"c",title:null})}"#,
            "NOT NULL constraint failed",
        ),
        (
            r#"mutation {createDbTableRow(table:"missing_table",row:{key:"c"})}"#,
            "Table not found or invalid identifier",
        ),
        (
            r#"mutation {deleteDbTableRows(table:"missing_table",keys:[{key:"a"}]){affectedCount}}"#,
            "Table not found or invalid identifier",
        ),
    ] {
        let response = schema.execute(document).await;
        assert_eq!(response.errors.len(), 1, "{document}");
        assert!(
            response.errors[0].message.contains(message),
            "{document}: {:?}",
            response.errors
        );
    }
    let db = schema
        .data::<Arc<plain_rs::infrastructure::db::Db>>()
        .unwrap();
    db.with_conn(|conn| {
        conn.execute_batch("CREATE TRIGGER reject_test_row_delete BEFORE DELETE ON test_rows BEGIN SELECT RAISE(ABORT, 'test deletion rejected'); END;")
            .unwrap();
    });
    let response = schema
        .execute(
            r#"mutation {deleteDbTableRows(table:"test_rows",keys:[{key:"a"}]){affectedCount}}"#,
        )
        .await;
    assert_eq!(response.errors.len(), 1);
    assert!(
        response.errors[0]
            .message
            .contains("test deletion rejected")
    );
    let rows: Vec<(String, String)> = db.with_conn(|conn| {
        let mut statement = conn
            .prepare("SELECT key,title FROM test_rows ORDER BY key")
            .unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    });
    assert_eq!(
        rows,
        vec![("a".into(), "First".into()), ("b".into(), "Second".into())]
    );
}
#[test]
fn an_unknown_column_type_is_unknown() {
    assert_eq!(DbColumnType::parse("WAT"), DbColumnType::Unknown);
}

#[tokio::test]
async fn developer_mode_is_checked_for_every_database_request() {
    let (_directory, schema) = fixture();
    let prefs = schema
        .data::<Arc<plain_rs::features::prefs::Prefs>>()
        .unwrap();
    let requests = [
        "{dbPath}",
        "{dbTables{name createSql columns{name}}}",
        "{dbTableRowCount(table:\"test_rows\")}",
        "{dbTableRows(table:\"test_rows\",offset:0,limit:1)}",
        r#"mutation {createDbTableRow(table:"test_rows",row:{key:"denied"})}"#,
        r#"mutation {deleteDbTableRows(table:"test_rows",keys:[{key:"a"}]){affectedCount}}"#,
        r#"mutation {deleteDbTableRows(table:"test_rows",keys:[]){affectedCount}}"#,
    ];
    for enabled in [false, true, false] {
        prefs.set("developer_mode", enabled).unwrap();
        for document in requests {
            if enabled && document.starts_with("mutation") {
                continue;
            }
            let response = schema.execute(document).await;
            if enabled {
                assert!(
                    response.errors.is_empty(),
                    "{document}: {:?}",
                    response.errors
                );
            } else {
                assert_eq!(response.errors.len(), 1, "{document}");
                assert_eq!(
                    response.errors[0].message, "No permission: developer mode is disabled",
                    "{document}"
                );
            }
        }
    }
    prefs.set("developer_mode", true).unwrap();
    let response = schema
        .execute("{dbTableRowCount(table:\"test_rows\")}")
        .await;
    assert_eq!(response.data.into_json().unwrap()["dbTableRowCount"], 2);
}

#[tokio::test]
async fn row_variables_are_objects_and_invalid_shapes_do_not_write() {
    let (_dir, schema) = fixture();
    let sdl = schema.sdl();
    assert!(sdl.contains("createDbTableRow(table: String!, row: JSON!): Boolean!"));
    assert!(sdl.contains("dbTableRows(table: String!, offset: Int!, limit: Int!): [JSON!]!"));
    let document = "mutation($row: JSON!) { createDbTableRow(table: \"test_rows\", row: $row) }";
    let row = json!({"key": "c", "title": "quote \" and 中文", "size": 7, "ratio": 2.5});
    let response = schema
        .execute(
            plain_graphql::Request::new(document)
                .variables(plain_graphql::Variables::from_json(json!({"row": row}))),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["createDbTableRow"], true);
    for row in [
        json!("{\"key\":\"d\"}"),
        json!([]),
        json!(null),
        json!(1),
        json!(true),
    ] {
        let response = schema
            .execute(
                plain_graphql::Request::new(document)
                    .variables(plain_graphql::Variables::from_json(json!({"row": row}))),
            )
            .await;
        assert!(!response.errors.is_empty(), "{row}");
    }
    let response = schema.execute(r#"{dbTableRowCount(table:"test_rows") dbTableRows(table:"test_rows",offset:2,limit:1)}"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["dbTableRowCount"], 3);
    assert_eq!(
        data["dbTableRows"],
        json!([{"key":"c", "title":"quote \" and 中文", "size":"7", "ratio":"2.5", "bytes":null}])
    );
    let response = schema
        .execute(r#"{dbTableRows(table:"test_rows",offset:3,limit:1)}"#)
        .await;
    assert_eq!(response.data.into_json().unwrap()["dbTableRows"], json!([]));
}

#[tokio::test]
async fn partial_primary_key_cannot_delete_sibling_folders() {
    let (_dir, schema) = fixture();
    let db = schema
        .data::<Arc<plain_rs::infrastructure::db::Db>>()
        .unwrap();
    db.with_conn(|c| {
        c.execute_batch(
            "INSERT INTO favorite_folders VALUES('root','one',NULL),('root','two',NULL)",
        )
        .unwrap()
    });
    let legacy = schema
        .execute(r#"mutation { deleteDbTableRows(table:"favorite_folders", ids:["root"]) }"#)
        .await;
    let remaining = || {
        db.with_conn(|c| {
            c.query_row("SELECT COUNT(*) FROM favorite_folders", [], |r| {
                r.get::<_, i32>(0)
            })
            .unwrap()
        })
    };
    assert_eq!(remaining(), 2);
    assert!(!legacy.errors.is_empty());
    let partial = schema.execute(r#"mutation { deleteDbTableRows(table:"favorite_folders",keys:[{root_path:"root",relative_path:"one"},{root_path:"root"}]) { affectedCount } }"#).await;
    assert!(!partial.errors.is_empty());
    assert_eq!(remaining(), 2);
    let full = r#"mutation { deleteDbTableRows(table:"favorite_folders",keys:[{root_path:"root",relative_path:"one"}]) { affectedCount } }"#;
    for count in [1, 0] {
        let result = schema.execute(full).await;
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(
            result.data.into_json().unwrap()["deleteDbTableRows"]["affectedCount"],
            count
        );
    }
    assert_eq!(remaining(), 1);
    assert_eq!(
        db.with_conn(|c| c
            .query_row("SELECT relative_path FROM favorite_folders", [], |r| r
                .get::<_, String>(
                0
            ))
            .unwrap()),
        "two"
    );
}

#[tokio::test]
async fn tables_without_primary_keys_are_read_only() {
    let (_dir, schema) = fixture();
    let db = schema
        .data::<Arc<plain_rs::infrastructure::db::Db>>()
        .unwrap();
    db.with_conn(|conn| {
        conn.execute_batch("CREATE TABLE unkeyed (label TEXT); INSERT INTO unkeyed VALUES ('keep')")
    })
    .unwrap();
    for mutation in [
        r#"mutation { createDbTableRow(table:"unkeyed", row:{label:"extra"}) }"#,
        r#"mutation { deleteDbTableRows(table:"unkeyed", keys:[{label:"keep"}]) { affectedCount } }"#,
    ] {
        assert!(!schema.execute(mutation).await.errors.is_empty());
    }
    let rows = schema
        .execute(r#"{dbTableRows(table:"unkeyed",offset:0,limit:10)}"#)
        .await;
    assert_eq!(
        rows.data.into_json().unwrap()["dbTableRows"],
        json!([{"label":"keep"}])
    );
}

#[tokio::test]
async fn deleting_a_tag_target_or_sms_keeps_its_sibling() {
    let (_dir, schema) = fixture();
    let db = schema
        .data::<Arc<plain_rs::infrastructure::db::Db>>()
        .unwrap();
    db.with_conn(|c| {
        c.execute_batch(
            "INSERT INTO tags VALUES('a','Notes','NOTE','t','t');
        INSERT INTO tag_relations VALUES('a','first','t'),('a','second','t');
        INSERT INTO trashed_sms VALUES('42',0,'t'),('42',1,'t');",
        )
    })
    .unwrap();
    for (table, key, remaining) in [
        (
            "tag_relations",
            r#"{tag_id:"a",target_key:"first"}"#,
            json!({"tag_id":"a","target_key":"second","created_at":"t"}),
        ),
        (
            "trashed_sms",
            r#"{message_id:"42",is_mms:"0"}"#,
            json!({"message_id":"42","is_mms":"1","trashed_at":"t"}),
        ),
    ] {
        let result = schema
            .execute(format!(
                r#"mutation {{deleteDbTableRows(table:"{table}",keys:[{key}]){{affectedCount}}}}"#
            ))
            .await;
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(
            result.data.into_json().unwrap()["deleteDbTableRows"]["affectedCount"],
            1
        );
        let result = schema
            .execute(format!(
                r#"{{dbTableRows(table:"{table}",offset:0,limit:5)}}"#
            ))
            .await;
        assert_eq!(
            result.data.into_json().unwrap()["dbTableRows"],
            json!([remaining])
        );
    }
}
