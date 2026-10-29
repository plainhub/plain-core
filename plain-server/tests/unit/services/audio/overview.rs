use super::*;
use crate::host::bridge::Host;
use serde_json::json;

#[tokio::test]
async fn sdk_summary_batches_the_full_catalog_without_metadata_or_per_row_host_calls() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let replies = host.clone();
    let worker = tokio::spawn(async move {
        let mut offsets = Vec::new();
        for expected in [0, 512, 1024] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "systemMediaRows");
            assert_eq!(request["params"]["dataType"], "AUDIO");
            assert_eq!(request["params"]["limit"], 512);
            assert_eq!(request["params"]["offset"], expected);
            assert_eq!(request["params"]["sortBy"], "NAME_ASC");
            assert_eq!(request["params"]["includeRevision"], true);
            offsets.push(expected);
            let rows = (expected..(expected + 512).min(1025))
                .map(|i| {
                    json!({
                        "id":i.to_string(),"path":format!("/{i}.mp3"),"artist":"Artist",
                        "createdAt":"2026-01-01T00:00:00Z",
                    })
                })
                .collect::<Vec<_>>();
            replies
                .reply(generation, json!({"id":request["id"],"result":{"rows":rows,"revisionBefore":null,"revisionAfter":null}}))
                .unwrap();
        }
        assert!(requests.try_recv().is_err());
        offsets
    });
    let summary = read(db.clone(), MediaCatalog::Platform { db, host })
        .await
        .unwrap();
    assert_eq!(summary.artists[0].item_count, 1025);
    assert_eq!(summary.recent_items.len(), 8);
    assert_eq!(worker.await.unwrap(), [0, 512, 1024]);
}

#[tokio::test]
async fn malformed_or_repeated_catalog_pages_fail_instead_of_returning_partial_totals() {
    for rows in [
        json!([{"id":"one","path":"/one"},{"id":"one","path":"/one"}]),
        json!([{"id":"","path":"/one"}]),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
        let host = Arc::new(Host::default());
        let (generation, mut requests) = host.connect();
        let replies = host.clone();
        let worker = tokio::spawn(async move {
            let request = requests.recv().await.unwrap();
            replies
                .reply(generation, json!({"id":request["id"],"result":{"rows":rows,"revisionBefore":null,"revisionAfter":null}}))
                .unwrap();
        });
        assert!(
            read(db.clone(), MediaCatalog::Platform { db, host })
                .await
                .is_err()
        );
        worker.await.unwrap();
    }
}

#[tokio::test]
async fn native_summary_streams_the_full_database_and_projects_selected_rows_only() {
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&directory.path().join("db")).unwrap());
    db.with_conn(|connection| connection.execute_batch(
        "INSERT INTO volumes(id,identity,path,is_available) VALUES(1,'TEST','/music',1); WITH RECURSIVE entries(n) AS (VALUES(0) UNION ALL SELECT n+1 FROM entries WHERE n<1099)
         INSERT INTO files(id,volume_id,path,file_name,kind,title,artist,size_bytes,duration_ms,width,height,rotation,created_at,updated_at,album_id,favorited)
         SELECT n+1,1,printf('%d.mp3',n),printf('%d.mp3',n),'AUDIO',printf('Song %d',n),'Artist',123,456,0,0,0,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','album',1 FROM entries;
         INSERT INTO media_play_history(media_type,file_id,position_ms,play_count,played_at) VALUES(1,'1100',0,7,'2026-01-02T00:00:00Z');"
    )).unwrap();
    let summary = read(db.clone(), MediaCatalog::Native(db)).await.unwrap();
    assert_eq!(summary.artists[0].item_count, 1100);
    assert_eq!(summary.artists[0].play_count, 7);
    assert_eq!(summary.recent_items.len(), 1);
    let recent = &summary.recent_items[0];
    assert_eq!(recent["id"], "1100");
    assert_eq!(recent["path"], "/music/1099.mp3");
    assert_eq!(recent["albumId"], "album");
    assert_eq!(recent["durationMs"], 456);
    assert_eq!(recent["bucketId"], "/music");
    assert_eq!(recent["isFavorite"], true);
}

fn sdk_row(id: usize, artist: &str) -> Value {
    json!({"id":id.to_string(),"path":format!("/{id}.mp3"),"artist":artist,"createdAt":"2026-01-01T00:00:00Z"})
}

fn sdk_revision(version: &str, generation: i64) -> Value {
    json!([{"volume":"external_primary","version":version,"generation":generation}])
}

#[tokio::test]
async fn sdk_changed_page_is_discarded_and_summary_restarts_from_zero() {
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&directory.path().join("db")).unwrap());
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let replies = host.clone();
    let worker = tokio::spawn(async move {
        for (offset, rows, before, after) in [
            (
                0,
                (0..512).map(|id| sdk_row(id, "Old")).collect::<Vec<_>>(),
                sdk_revision("a", 1),
                sdk_revision("a", 1),
            ),
            (
                512,
                vec![sdk_row(512, "Old")],
                sdk_revision("a", 2),
                sdk_revision("a", 2),
            ),
            (
                0,
                vec![sdk_row(999, "Current")],
                sdk_revision("a", 2),
                sdk_revision("a", 2),
            ),
        ] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "systemMediaRows");
            assert_eq!(request["params"]["offset"], offset);
            replies.reply(generation, json!({"id":request["id"],"result":{"rows":rows,"revisionBefore":before,"revisionAfter":after}})).unwrap();
        }
        assert!(requests.try_recv().is_err());
    });
    let summary = read(db.clone(), MediaCatalog::Platform { db, host })
        .await
        .unwrap();
    assert_eq!(summary.artists.len(), 1);
    assert_eq!(summary.artists[0].name, "Current");
    assert_eq!(summary.artists[0].item_count, 1);
    assert_eq!(summary.recent_items[0]["id"], "999");
    worker.await.unwrap();
}

#[tokio::test]
async fn sdk_changes_inside_queries_or_version_resets_have_bounded_retries() {
    for reset in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open(&directory.path().join("db")).unwrap());
        let host = Arc::new(Host::default());
        let (generation, mut requests) = host.connect();
        let replies = host.clone();
        let worker = tokio::spawn(async move {
            for _ in 0..3 {
                let request = requests.recv().await.unwrap();
                assert_eq!(request["params"]["offset"], 0);
                let before = sdk_revision("a", 1);
                let after = if reset {
                    sdk_revision("b", 1)
                } else {
                    sdk_revision("a", 2)
                };
                replies.reply(generation, json!({"id":request["id"],"result":{"rows":[sdk_row(1,"Stale")],"revisionBefore":before,"revisionAfter":after}})).unwrap();
            }
            requests
        });
        assert!(
            read(db.clone(), MediaCatalog::Platform { db, host })
                .await
                .err()
                .unwrap()
                .contains("kept changing")
        );
        let mut requests = worker.await.unwrap();
        assert!(requests.try_recv().is_err());
    }
}
