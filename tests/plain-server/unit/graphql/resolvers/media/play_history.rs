use super::*;
use crate::graphql::executor::RequestOrigin;
use crate::test_support::schema::SchemaFixture as PublicSchema;
use plain_rs::features::{enums::DataType, prefs::Prefs};
use plain_rs::infrastructure::host::bridge::Host;
use plain_libs::serde_json::{Value, json};

fn fixture(permissions: &[&str]) -> (plain_libs::temporary::TempDir, Arc<Db>, PublicSchema) {
    fixture_with_host(permissions, Arc::new(Host::default()))
}

fn fixture_with_host(permissions: &[&str], host: Arc<Host>) -> (plain_libs::temporary::TempDir, Arc<Db>, PublicSchema) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", permissions).unwrap();
    let db = Arc::new(Db::open(&directory.path().join("db")).unwrap());
    let (events, _) = tokio::sync::broadcast::channel(16);
    let schema = crate::test_support::schema::build(
        host,
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
        conn.execute("INSERT INTO files(id,volume_id,path,file_name,kind,title,artist,size_bytes,duration_ms,created_at,updated_at) VALUES(?1,1,?2,?3,'AUDIO',?3,?4,1,1000,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",plain_libs::rusqlite::params![id,format!("{id}.mp3"),title,artist])
    }).unwrap();
    play_history::record(db, DataType::Audio, id, 123).unwrap();
}

async fn query(schema: &PublicSchema, text: &str, offset: i32, limit: i32) -> Value {
    let response = schema.execute(plain_graphql::Request::new("query($query: String!, $offset: Int!, $limit: Int!) { mediaPlayHistory(query: $query, offset: $offset, limit: $limit) { mediaId mediaType positionMs playCount media { id title path } } }")
        .variables(plain_graphql::Variables::from_json(json!({"query":text,"offset":offset,"limit":limit})))
        .data(RequestOrigin::Remote)).await;
    plain_libs::serde_json::to_value(response).unwrap()
}

#[tokio::test]
async fn history_filters_current_metadata_before_paging_and_skips_missing_media() {
    let (_directory, db, schema) = fixture(&["AUDIO", "VIDEOS"]);
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
    assert_eq!(row["mediaType"], "AUDIO");
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
    let (_directory, db, schema) = fixture(&["AUDIO", "VIDEOS"]);
    audio(&db, "1", "One", "Artist");
    for (offset, limit) in [(-1, 10), (0, -1)] {
        assert!(!query(&schema, "", offset, limit).await["errors"].is_null());
    }
    let empty = query(&schema, "", 0, 0).await;
    assert!(empty["errors"].is_null(), "{empty}");
    assert_eq!(empty["data"]["mediaPlayHistory"], json!([]));
}

#[tokio::test]
async fn history_only_exposes_authorized_media_categories() {
    for (features, expected) in [(&["AUDIO"][..], "a"), (&["VIDEOS"][..], "v")] {
        let (_directory, db, schema) = fixture(features);
        audio(&db, "a", "Song", "Artist");
        db.with_conn(|conn| conn.execute("INSERT INTO files(id,volume_id,path,file_name,kind,title,size_bytes,duration_ms,created_at,updated_at) VALUES('v',1,'v.mp4','v.mp4','VIDEO','Clip',1,1000,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')", [])).unwrap();
        play_history::record(&db, DataType::Video, "v", 0).unwrap();
        let result = query(&schema, "", 0, 10).await;
        assert!(result["errors"].is_null(), "{result}");
        let items = result["data"]["mediaPlayHistory"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["mediaId"], expected);
    }
}

#[tokio::test]
async fn audio_history_uses_a_supported_system_catalog_sort() {
    let host = Arc::new(Host::with_startup_config(plain_rs::infrastructure::host::startup::StartupConfig {
        system_media_catalog: true,
        system_media_trash: true,
        ..plain_rs::infrastructure::platform::startup::config()
    }).unwrap());
    let (_dir, db, schema) = fixture_with_host(&["AUDIO"], host.clone());
    play_history::save(&db, DataType::Audio, "17", 375).unwrap();
    let (generation, mut requests) = host.connect();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let params = &request["params"];
            let sort = plain_libs::serde_json::from_value::<plain_rs::features::prefs::settings::AudioSortBy>(params["sortBy"].clone());
            let reply = if request["method"] == "systemMediaRows" && params["dataType"] == "AUDIO" && sort.is_ok() {
                json!({"id": request["id"], "result": [{"id":"17","title":"Generated audio","durationMs":1000}]})
            } else {
                json!({"id": request["id"], "error":"Unsupported audio catalog request"})
            };
            host.reply(generation, reply).unwrap();
        }
    });
    let result = query(&schema, "type:AUDIO ids:17", 0, 1).await;
    assert!(result["errors"].is_null(), "{result}");
    assert_eq!(result["data"]["mediaPlayHistory"][0]["positionMs"], 375);
}
