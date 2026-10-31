use super::*;
use crate::test_support::schema::SchemaFixture as PublicSchema;
use serde_json::Value;
use serde_json::json;
fn fixture() -> (tempfile::TempDir, PublicSchema) {
    let dir = tempfile::tempdir().unwrap();
    let path = plain_rs::data_paths::DataPaths::new(dir.path()).database();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&plain_rs::prefs::default_path(dir.path())).unwrap());
    let db = Arc::new(plain_rs::db::Db::open(&path).unwrap());
    db.with_conn(|conn| conn.execute_batch("CREATE TABLE test_rows (key TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT 'untitled', size INTEGER, ratio REAL, bytes BLOB); INSERT INTO test_rows VALUES('a','First',42,1.5,NULL),('b','Second',43,2.5,NULL);").unwrap());
    let host = Arc::new(plain_rs::host::bridge::Host::default());
    let (events, _) = tokio::sync::broadcast::channel(16);
    let schema =
        crate::test_support::schema::build(host, events, prefs, db, dir.path().to_path_buf());
    (dir, schema)
}

#[tokio::test]
async fn owned_database_browser_works_without_a_connected_host() {
    let (dir, schema) = fixture();
    let response=schema.execute(r#"{dbPath dbTables dbTableInfo(table:"test_rows"){idKey} dbTableRowCount(table:"test_rows") dbTableColumns(table:"test_rows"){name dataType notNull defaultValue primaryKey} dbTableRows(table:"test_rows",offset:1,limit:1)}"#).await;
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
    assert!(
        data["dbTables"]
            .as_array()
            .unwrap()
            .contains(&json!("test_rows"))
    );
    assert_eq!(data["dbTableInfo"]["idKey"], "key");
    assert_eq!(data["dbTableRowCount"], 2);
    let columns = data["dbTableColumns"].as_array().unwrap();
    assert_eq!(columns[0]["dataType"], "TEXT");
    assert_eq!(columns[0]["primaryKey"], true);
    assert_eq!(columns[1]["notNull"], true);
    assert_eq!(columns[1]["defaultValue"], "'untitled'");
    assert_eq!(columns[2]["dataType"], "INTEGER");
    assert_eq!(columns[3]["dataType"], "REAL");
    assert_eq!(columns[4]["dataType"], "BLOB");
    let rows = data["dbTableRows"].as_array().unwrap();
    let row: Value = serde_json::from_str(rows[0].as_str().unwrap()).unwrap();
    assert_eq!(row["key"], "b");
    assert_eq!(row["size"], "43");
    let response=schema.execute(r#"mutation {createDbTableRow(table:"test_rows",row:"{\"key\":\"c\",\"title\":\"Third\"}") deleteDbTableRows(table:"test_rows",ids:["a","b"])}"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap(),
        json!({"createDbTableRow":true,"deleteDbTableRows":true})
    );
    let response = schema
        .execute(r#"{dbTableRowCount(table:"test_rows")}"#)
        .await;
    assert_eq!(response.data.into_json().unwrap()["dbTableRowCount"], 1);
}
#[tokio::test]
async fn invalid_room_row_is_false_and_empty_delete_is_false() {
    let (_dir, schema) = fixture();
    let response=schema.execute(r#"mutation {createDbTableRow(table:"test_rows",row:"bad") deleteDbTableRows(table:"test_rows",ids:[])}"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap(),
        json!({"createDbTableRow":false,"deleteDbTableRows":false})
    );
}
#[test]
fn an_unknown_column_type_is_unknown() {
    assert_eq!(DbColumnType::parse("WAT"), DbColumnType::Unknown);
}
