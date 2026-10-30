use super::*;

fn metadata() -> plain_rs::db::MediaFileMetadata {
    plain_rs::db::MediaFileMetadata {
        album: String::new(),
        album_artist: String::new(),

        data_type: DataType::Image,
        path: "/catalog/picture.jpg".into(),
        title: "Picture".into(),
        artist: String::new(),
        bucket_id: "pictures".into(),
        size_bytes: 2048,
        duration_ms: 0,
        cover_image_id: None,
        width: 800,
        height: 600,
        rotation: 0,
        taken_at: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

#[tokio::test]
async fn native_catalog_reads_rows_counts_ids_and_buckets_without_a_host() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let stored = db.record_media_files(&[metadata()]).unwrap();
    let catalog = MediaCatalog::new(db, Arc::new(Host::default()));
    assert!(matches!(catalog, MediaCatalog::Native(_)));
    let rows = catalog
        .rows(DataType::Image, "", FileSortBy::NameAsc, 10, 0)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], stored[0].id);
    assert_eq!(rows[0]["width"], 800);
    assert_eq!(catalog.count(DataType::Image, "").await.unwrap(), 1);
    assert_eq!(catalog.count(DataType::Video, "").await.unwrap(), 0);
    assert_eq!(
        catalog.ids(DataType::Image, "", false).await.unwrap(),
        vec![stored[0].id.clone()]
    );
    assert!(
        catalog
            .ids(DataType::Image, "", true)
            .await
            .unwrap()
            .is_empty()
    );
    let buckets = catalog.buckets(DataType::Image).await.unwrap();
    assert_eq!(buckets[0]["id"], "/catalog");
}

#[tokio::test]
async fn platform_catalog_reads_os_facts_without_a_local_catalogue() {
    let host = Arc::new(
        Host::with_startup_config(crate::host::startup::StartupConfig {
            system_media_catalog: true,
            ..crate::platform::startup::config()
        })
        .unwrap(),
    );
    let (generation, mut requests) = host.connect();
    let replies = host.clone();
    let worker = tokio::spawn(async move {
        let mut methods = Vec::new();
        for _ in 0..4 {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["params"]["dataType"], "IMAGE");
            let method = request["method"].as_str().unwrap();
            let value = match method {
                "systemMediaRows" => {
                    assert!(request["params"].get("query").is_none());
                    assert_eq!(
                        request["params"]["plan"]["args"],
                        json!(["picture", "picture"])
                    );
                    assert_eq!(request["params"]["offset"], 4);
                    assert_eq!(request["params"]["limit"], 8);
                    assert_eq!(request["params"]["sortBy"], "NAME_ASC");
                    json!([{"id":"42","path":"content://media/images/42"}])
                }
                "systemMediaCount" => json!(1),
                "systemMediaIdsFacts" => {
                    assert_eq!(request["params"]["plan"]["trash"], true);
                    json!(["42"])
                }
                "systemMediaBucketItemFacts" => {
                    assert_eq!(
                        request["params"]["plan"]["clauses"],
                        json!(["bucket_display_name != ''"])
                    );
                    assert_eq!(request["params"]["plan"]["args"], json!([]));
                    json!([{"id":"camera","path":"content://media/images/42"}])
                }
                other => panic!("unexpected host operation {other}"),
            };
            methods.push(method.to_owned());
            replies
                .reply(generation, json!({"id":request["id"],"result":value}))
                .unwrap();
        }
        methods
    });
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let catalog = MediaCatalog::new(db, host);
    let rows = catalog
        .rows(DataType::Image, "text:picture", FileSortBy::NameAsc, 8, 4)
        .await
        .unwrap();
    assert_eq!(rows[0]["id"], "42");
    assert_eq!(catalog.count(DataType::Image, "").await.unwrap(), 1);
    assert_eq!(
        catalog.ids(DataType::Image, "", true).await.unwrap(),
        vec!["42"]
    );
    assert_eq!(
        catalog.buckets(DataType::Image).await.unwrap()[0]["id"],
        "camera"
    );
    assert_eq!(worker.await.unwrap().len(), 4);
}

#[test]
fn sampled_media_revisions_are_canonical_and_reject_malformed_volumes() {
    let page: MediaPage = serde_json::from_value(serde_json::json!({
        "rows":[],
        "revisionBefore":[{"volume":"b","version":"v","generation":2},{"volume":"a","version":"v","generation":1}],
        "revisionAfter":[{"volume":"a","version":"v","generation":1},{"volume":"b","version":"v","generation":2}]
    })).unwrap();
    let page = page.validate().unwrap();
    assert_eq!(page.revision_before, page.revision_after);
    for revision in [
        serde_json::json!([{"volume":"a","version":"","generation":1}]),
        serde_json::json!([{"volume":"a","version":"v","generation":-1}]),
        serde_json::json!([{"volume":"a","version":"v","generation":1},{"volume":"a","version":"v","generation":1}]),
    ] {
        let page: MediaPage = serde_json::from_value(
            serde_json::json!({"rows":[],"revisionBefore":revision,"revisionAfter":null}),
        )
        .unwrap();
        assert!(page.validate().is_err());
    }
}

#[tokio::test]
async fn semantic_image_candidates_intersect_platform_ids_and_keep_non_text_filters() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let tag = plain_rs::tags::create_tag(&db, DataType::Image.kind(), "selected").unwrap();
    plain_rs::tags::add_relations(&db, &[(tag.id.clone(), "2".into())]).unwrap();
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let replies = host.clone();
    let worker = tokio::spawn(async move {
        for _ in 0..2 {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "systemMediaRows");
            let params = &request["params"];
            assert_eq!(params["dataType"], "IMAGE");
            assert_eq!(params["sortBy"], "NAME_ASC");
            assert_eq!(params["limit"], 1);
            assert_eq!(params["offset"], 0);
            assert!(params.get("query").is_none());
            assert_eq!(params["plan"]["idsColumn"], "_id");
            assert_eq!(params["plan"]["ids"], json!(["2"]));
            assert_eq!(params["plan"]["trash"], true);
            assert_eq!(
                params["plan"]["clauses"],
                json!(["bucket_id = ?", "_size > ?"])
            );
            assert_eq!(params["plan"]["args"], json!(["summer albums", "100"]));
            replies
                .reply(
                    generation,
                    json!({"id":request["id"],"result":[{"id":"2"}]}),
                )
                .unwrap();
        }
    });
    let catalog = MediaCatalog::Platform { db, host };
    let candidates = ["2", "3", "2"].map(str::to_owned);
    for selection in ["ids:1,2".to_owned(), format!("tag_id:{}", tag.id)] {
        let query = format!(
            "text:\"red cats\" {selection} bucket_id:\"summer albums\" size:>100 trash:true"
        );
        assert_eq!(
            catalog
                .image_matches(&query, &candidates, false)
                .await
                .unwrap(),
            vec![json!({"id":"2"})]
        );
    }
    worker.await.unwrap();
    for query in ["text:cat ids:", "text:cat ids:9", "text:cat tag_id:missing"] {
        assert!(
            catalog
                .image_matches(query, &candidates, false)
                .await
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        catalog
            .image_matches("text:cat unsupported:value", &candidates, false)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn document_extensions_read_only_bounded_path_pages() {
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&directory.path().join("data.db")).unwrap());
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let replies = host.clone();
    let worker = tokio::spawn(async move {
        for offset in [0, 512] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "systemMediaRows");
            let params = &request["params"];
            assert_eq!(params["dataType"], "DOC");
            assert_eq!(params["pathsOnly"], true);
            assert_eq!(params["limit"], 512);
            assert_eq!(params["offset"], offset);
            assert_eq!(
                params["plan"]["args"],
                json!([
                    "text/%",
                    "application/pdf",
                    "application/msword",
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                    "application/javascript",
                    "/documents",
                ])
            );
            let paths = if offset == 0 {
                vec!["/documents/report.PDF"; 512]
            } else {
                vec!["/documents/notes.txt", "/documents/no-extension"]
            };
            replies
                .reply(generation, json!({"id":request["id"],"result":paths}))
                .unwrap();
        }
    });
    let catalog = MediaCatalog::Platform { db, host };
    assert_eq!(
        catalog
            .document_extensions("bucket_id:/documents")
            .await
            .unwrap(),
        vec![("PDF".into(), 512), ("TXT".into(), 1)]
    );
    worker.await.unwrap();
}
