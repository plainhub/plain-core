use super::*;
use crate::graphql::executor::RequestOrigin;
use crate::test_support::schema::SchemaFixture as PublicSchema;
use plain_rs::host::bridge::Host;
use plain_rs::{enums::DataType, prefs::Prefs};
use serde_json::{Value, json};

fn fixture(permissions: &[&str]) -> (tempfile::TempDir, Arc<Db>, PublicSchema) {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", permissions).unwrap();
    let db = Arc::new(Db::open(&directory.path().join("db")).unwrap());
    let (events, _) = tokio::sync::broadcast::channel(16);
    let schema = crate::test_support::schema::build(
        Arc::new(Host::default()),
        events,
        prefs,
        db.clone(),
        directory.path().to_owned(),
    );
    (directory, db, schema)
}

fn audio(db: &Db, id: &str, title: &str, artist: &str) {
    db.with_conn(|conn| {
        conn.execute("INSERT OR IGNORE INTO volumes(id,identity,path,is_available) VALUES(1,'TEST','/music',1)",[])?;
        conn.execute("INSERT INTO files(id,volume_id,path,file_name,kind,title,artist,size_bytes,duration_ms,created_at,updated_at) VALUES(?1,1,?2,?3,'AUDIO',?3,?4,1,1000,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",rusqlite::params![id.parse::<i64>().unwrap(),format!("{id}.mp3"),title,artist])
    }).unwrap();
    play_history::record(db, DataType::Audio, id, 123).unwrap();
}

async fn query(schema: &PublicSchema, text: &str, offset: i32, limit: i32) -> Value {
    let response = schema.execute(async_graphql::Request::new("query($query: String!, $offset: Int!, $limit: Int!) { mediaPlayHistory(query: $query, offset: $offset, limit: $limit) { mediaId positionMs playCount media { id title path } } }")
        .variables(async_graphql::Variables::from_json(json!({"query":text,"offset":offset,"limit":limit})))
        .data(RequestOrigin::Remote)).await;
    serde_json::to_value(response).unwrap()
}

#[tokio::test]
async fn history_filters_current_metadata_before_paging_and_skips_missing_media() {
    let (_directory, db, schema) = fixture(&[STORAGE]);
    audio(&db, "1", "Road Song", "Ferry");
    audio(&db, "2", "Another Song", "Ferry");
    audio(&db, "3", "Unrelated", "Other");
    play_history::record(&db, DataType::Audio, "999", 0).unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE media_play_history SET played_at='2026-01-01T00:00:00.000Z'",
            [],
        )
    })
    .unwrap();
    let all = query(&schema, "type:AUDIO ferry", 0, 10).await;
    assert!(all["errors"].is_null(), "{all}");
    assert_eq!(
        all["data"]["mediaPlayHistory"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["mediaId"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["2", "1"]
    );
    let page = query(&schema, "type:AUDIO ferry", 1, 1).await;
    assert!(page["errors"].is_null(), "{page}");
    assert_eq!(
        page["data"]["mediaPlayHistory"].as_array().unwrap().len(),
        1
    );
    assert_eq!(page["data"]["mediaPlayHistory"][0]["mediaId"], "1");
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE files SET title='Updated',artist='Changed' WHERE id='1'",
            [],
        )
    })
    .unwrap();
    let updated = query(&schema, "ids:1 changed", 0, 10).await;
    assert!(updated["errors"].is_null(), "{updated}");
    let row = &updated["data"]["mediaPlayHistory"][0];
    assert_eq!(row["media"]["title"], "Updated");
    assert_eq!(row["positionMs"], 123);
    assert_eq!(row["playCount"], 1);
    assert_eq!(
        query(&schema, "ferry", 0, 10).await["data"]["mediaPlayHistory"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn history_requires_storage_access_and_rejects_invalid_page_windows() {
    let (_denied_dir, denied_db, denied) = fixture(&[]);
    audio(&denied_db, "1", "Private", "Artist");
    assert!(!query(&denied, "", 0, 10).await["errors"].is_null());
    let (_directory, db, schema) = fixture(&[STORAGE]);
    audio(&db, "1", "One", "Artist");
    for (offset, limit) in [(-1, 10), (0, -1)] {
        assert!(!query(&schema, "", offset, limit).await["errors"].is_null());
    }
    let empty = query(&schema, "", 0, 0).await;
    assert!(empty["errors"].is_null(), "{empty}");
    assert_eq!(empty["data"]["mediaPlayHistory"], json!([]));
}
