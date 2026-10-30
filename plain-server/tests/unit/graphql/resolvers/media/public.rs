use super::*;
use crate::graphql::schemas::public::PublicSchema;
use plain_rs::db::Db;
use plain_rs::prefs::Prefs;
use serde_json::{Value, json};
use std::sync::Arc;

const GRANTED: &str = r#"["WRITE_EXTERNAL_STORAGE"]"#;

fn stub(host: Arc<Host>, handler: impl Fn(&str, Value) -> Value + Send + 'static) {
    let (generation, mut requests) = host.connect();
    let host = host.clone();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let Some(id) = request["id"].as_u64() else {
                continue;
            };
            let result = handler(
                &request["method"].as_str().unwrap_or_default(),
                request["params"].clone(),
            );
            let _ = host.reply(generation, json!({ "id": id, "result": result }));
        }
    });
}

/// `granted` is the platform's runtime answer for WRITE_EXTERNAL_STORAGE.
fn fixture(permissions: &str, granted: bool) -> (tempfile::TempDir, PublicSchema) {
    fixture_with(permissions, granted, move |method, _| match method {
        "systemPermissionFacts" => json!({ "granted": { "WRITE_EXTERNAL_STORAGE": granted } }),
        other => panic!("unexpected host call {other}"),
    })
}

fn fixture_with(
    permissions: &str,
    granted: bool,
    handler: impl Fn(&str, Value) -> Value + Send + 'static,
) -> (tempfile::TempDir, PublicSchema) {
    let (dir, schema, _) = fixture_seeded(permissions, granted, |_| {}, handler);
    (dir, schema)
}

fn fixture_seeded(
    permissions: &str,
    granted: bool,
    seed: impl FnOnce(&Db),
    handler: impl Fn(&str, Value) -> Value + Send + 'static,
) -> (tempfile::TempDir, PublicSchema, Arc<Db>) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs
        .set(
            "api_permissions",
            json!(serde_json::from_str::<Vec<String>>(permissions).unwrap()),
        )
        .unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    seed(&db);
    let host = Arc::new(Host::default());
    stub(host.clone(), move |method, params| {
        if method == "systemPermissionFacts" {
            return json!({ "granted": { "WRITE_EXTERNAL_STORAGE": granted } });
        }
        handler(method, params)
    });
    let directory = dir.path().to_path_buf();
    let (events, _) = tokio::sync::broadcast::channel(16);
    (
        dir,
        crate::graphql::schemas::public::build(host, events, prefs, db.clone(), directory),
        db,
    )
}

fn media(
    data_type: DataType,
    path: &str,
    title: &str,
    size: i64,
    bucket: &str,
) -> plain_rs::db::MediaFileMetadata {
    plain_rs::db::MediaFileMetadata {
        album: String::new(),
        album_artist: String::new(),

        data_type,
        path: path.to_owned(),
        title: title.to_owned(),
        artist: String::new(),
        bucket_id: bucket.to_owned(),
        size_bytes: size,
        duration_ms: 0,
        cover_image_id: None,
        width: 4000,
        height: 3000,
        rotation: 0,
        taken_at: None,
        created_at: "2026-01-02T03:04:05.000Z".to_owned(),
        updated_at: "2026-02-03T04:05:06.000Z".to_owned(),
    }
}

fn image_row(id: &str, path: &str) -> Value {
    json!({
        "id": id, "title": id, "path": path, "size": 2048,
        "width": 4000, "height": 3000, "rotation": 0, "bucketId": "b1",
        "createdAt": "2026-01-02T03:04:05.000Z", "updatedAt": "2026-02-03T04:05:06.000Z",
        "takenAt": "2026-01-01T00:00:00.000Z", "isFavorite": true,
    })
}

/// The contract `Tag` has no numeric kind field — the media kind is implied
/// by the list it came from.
#[tokio::test]
async fn counts_and_buckets_degrade_when_storage_is_not_granted() {
    let (_dir, schema) = fixture(GRANTED, false);
    let response = schema
        .execute(
            r#"query { imageCount(query:"") videoCount(query:"") docCount(query:"")
                      mediaBuckets(type:IMAGE) { id } docExtGroups { ext } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["imageCount"], 0);
    assert_eq!(data["videoCount"], 0);
    assert_eq!(data["docCount"], 0);
    assert_eq!(data["mediaBuckets"], json!([]));
    assert_eq!(data["docExtGroups"], json!([]));
}

/// The same degradation applies without the web-client opt-in at all.
#[tokio::test]
async fn counts_degrade_without_the_api_permission_too() {
    let (_dir, schema) = fixture("[]", true);
    let response = schema
        .execute(r#"query { imageCount(query:"") mediaBuckets(type:VIDEO) { id } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["imageCount"], 0);
    assert_eq!(data["mediaBuckets"], json!([]));
}

/// Browsing is not a soft operation — without the opt-in it errors, so a
/// client cannot mistake "you may not look" for "there is nothing there".
#[tokio::test]
async fn browsing_media_errors_out_without_the_api_permission() {
    let (_dir, schema) = fixture("[]", true);
    let response = schema
        .execute(
            r#"query { images(offset:0, limit:10, query:"", sortBy:NAME_ASC) { id }
                      videos(offset:0, limit:10, query:"", sortBy:NAME_ASC) { id }
                      docs(offset:0, limit:10, query:"", sortBy:NAME_ASC) { id } }"#,
        )
        .await;
    assert_eq!(response.errors.len(), 3);
    for error in &response.errors {
        assert_eq!(error.message, "no_permission");
    }
}

#[tokio::test]
async fn images_carry_their_row_and_their_tags() {
    let (_dir, schema, db) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            let mut first = media(DataType::Image, "/storage/DCIM/one.jpg", "i1", 2048, "b1");
            first.taken_at = Some("2026-01-01T00:00:00.000Z".to_owned());
            let mut second = media(DataType::Image, "/storage/DCIM/two.jpg", "i2", 4096, "b1");
            second.updated_at = "2025-01-01T00:00:00.000Z".to_owned();
            db.record_media_files(&[first, second]).unwrap();
        },
        |method, _| panic!("unexpected host call {method}"),
    );
    let kind = plain_rs::enums::DataType::Image.kind();
    let tag = plain_rs::tags::create_tag(&db, kind, "holiday").unwrap();
    let rows = db.media_file_list(DataType::Image, 10, 0).unwrap();
    db.with_conn(|conn| {
        conn.execute("UPDATE files SET favorited=1 WHERE id=?1", [&rows[0].id])
            .unwrap();
    });
    let relations = (0..7)
        .map(|i| {
            (
                tag.id.clone(),
                if i == 0 {
                    rows[0].id.clone()
                } else {
                    format!("other{i}")
                },
            )
        })
        .collect::<Vec<_>>();
    plain_rs::tags::add_relations(&db, &relations).unwrap();
    let response = schema
        .execute(
            r#"query { images(offset:0, limit:20, query:"", sortBy:DATE_DESC)
                      { id title path size bucketId createdAt updatedAt takenAt isFavorite
                        tags { id name count } } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let items = data["images"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["title"], "i1");
    assert_eq!(items[0]["bucketId"], "/storage/DCIM");
    assert_eq!(items[0]["size"], 2048);
    assert_eq!(items[0]["isFavorite"], true);
    assert_eq!(items[0]["takenAt"], "2026-01-01T00:00:00.000Z");
    assert_eq!(items[0]["tags"].as_array().unwrap().len(), 1);
    assert_eq!(items[0]["tags"][0]["name"], "holiday");
    assert_eq!(items[0]["tags"][0]["count"], 7);
    // An item with no relations reports an empty list, not null — clients
    // spread this straight into a chip row.
    assert_eq!(items[1]["tags"], json!([]));
}

#[tokio::test]
async fn filename_search_receives_the_complete_query_without_loaded_models() {
    let (_dir, schema, _) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            db.record_media_files(&[
                media(
                    DataType::Image,
                    "/storage/DCIM/sunset-a.jpg",
                    "sunset a",
                    4096,
                    "b1",
                ),
                media(
                    DataType::Image,
                    "/storage/DCIM/small.jpg",
                    "small",
                    10,
                    "b1",
                ),
            ])
            .unwrap();
        },
        |method, _| panic!("unexpected host call {method}"),
    );
    let response = schema
        .execute(
            r#"query { images(offset:0, limit:5, query:"text:sunset size:>100", sortBy:NAME_ASC) { title } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["images"],
        json!([{"title": "sunset a"}])
    );
}

#[tokio::test]
async fn filename_search_preserves_queries_without_text() {
    let (_dir, schema, _) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            db.record_media_files(&[
                media(DataType::Image, "/storage/DCIM/big.jpg", "big", 4096, "b1"),
                media(
                    DataType::Image,
                    "/storage/DCIM/small.jpg",
                    "small",
                    10,
                    "b1",
                ),
            ])
            .unwrap();
        },
        |method, _| panic!("unexpected host call {method}"),
    );
    let response = schema
        .execute(
            r#"query { images(offset:0, limit:5, query:"size:>100", sortBy:NAME_ASC) { title } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["images"],
        json!([{"title": "big"}])
    );
}

#[tokio::test]
async fn image_count_uses_the_catalogue_without_loaded_models() {
    let (_dir, schema, _) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            db.record_media_files(&[
                media(
                    DataType::Image,
                    "/storage/DCIM/cat-1.jpg",
                    "cat one",
                    10,
                    "b1",
                ),
                media(
                    DataType::Image,
                    "/storage/DCIM/cat-2.jpg",
                    "cat two",
                    10,
                    "b1",
                ),
                media(DataType::Image, "/storage/DCIM/dog.jpg", "dog", 10, "b1"),
            ])
            .unwrap();
        },
        |method, _| panic!("unexpected host call {method}"),
    );
    let response = schema
        .execute(r#"query { imageCount(query:"text:cat") }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["imageCount"], 2);
}

#[tokio::test]
async fn videos_carry_the_duration_and_the_taken_time() {
    let (_dir, schema, _) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            db.record_media_files(&[media(
                DataType::Video,
                "/storage/Movies/clip.mp4",
                "clip",
                900,
                "b2",
            )])
            .unwrap();
        },
        |method, _| panic!("unexpected host call {method}"),
    );
    let response = schema
        .execute(
            r#"query { videos(offset:0, limit:10, query:"", sortBy:SIZE_ASC)
                      { id durationMs size takenAt tags { id } } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["videos"][0]["durationMs"], 0);
    assert_eq!(data["videos"][0]["size"], 900);
    assert_eq!(data["videos"][0]["takenAt"], Value::Null);
    // No tag rows at all still yields an empty list per item.
    assert_eq!(data["videos"][0]["tags"], json!([]));
}

/// `Doc.extension` is derived from the path, matching plain-app's lazy
/// property — the media store does not carry an extension column.
#[tokio::test]
async fn doc_extensions_come_from_the_path() {
    let (_dir, schema, _) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            db.record_media_files(&[
                media(DataType::Doc, "/storage/Docs/a.pdf", "a", 10, "b3"),
                media(DataType::Doc, "/storage/Docs/b.txt", "b", 10, "b3"),
            ])
            .unwrap();
        },
        |method, _| panic!("unexpected host call {method}"),
    );
    let response = schema
        .execute(
            r#"query { docs(offset:0, limit:10, query:"", sortBy:NAME_ASC) { id extension } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let docs = data["docs"].as_array().unwrap();
    let extensions = Value::Array(
        docs.iter()
            .map(|d| d["extension"].clone())
            .collect::<Vec<_>>(),
    );
    assert_eq!(extensions, json!(["pdf", "txt"]), "{data}");
}

/// One row per folder, sorted by the lowercased folder name, with a bounded
/// number of sample paths for the thumbnail grid.
#[tokio::test]
async fn buckets_collapse_rows_into_folders() {
    let (_dir, schema, _) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            let rows: Vec<_> = [
                ("/storage/Camera/a.mp4", "b1"),
                ("/storage/Movies/a.mp4", "b2"),
                ("/storage/Movies/b.mp4", "b2"),
                ("/storage/Movies/c.mp4", "b2"),
                ("/storage/Movies/d.mp4", "b2"),
                ("/storage/Movies/e.mp4", "b2"),
            ]
            .into_iter()
            .map(|(path, bucket)| media(DataType::Video, path, "clip", 10, bucket))
            .collect();
            db.record_media_files(&rows).unwrap();
        },
        |method, _| panic!("unexpected host call {method}"),
    );
    let response = schema
        .execute(r#"query { mediaBuckets(type:VIDEO) { id name itemCount size topItemPaths } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let buckets = data["mediaBuckets"].as_array().unwrap();
    assert_eq!(buckets.len(), 2);
    // Camera sorts before Movies.
    assert_eq!(buckets[0]["id"], "/storage/Camera");
    assert_eq!(buckets[0]["itemCount"], 1);
    assert_eq!(buckets[0]["size"], 10);
    assert_eq!(buckets[1]["id"], "/storage/Movies");
    assert_eq!(buckets[1]["itemCount"], 5);
    assert_eq!(buckets[1]["size"], 50);
    // Five rows in, but only four samples out — the grid never needs more.
    assert_eq!(buckets[1]["topItemPaths"].as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn doc_extension_groups_count_the_catalogue() {
    let (_dir, schema, _) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            db.record_media_files(&[
                media(DataType::Doc, "/d/a.pdf", "a", 10, "b"),
                media(DataType::Doc, "/d/b.pdf", "b", 10, "b"),
                media(DataType::Doc, "/d/c.txt", "c", 10, "b"),
            ])
            .unwrap();
        },
        |method, _| panic!("unexpected host call {method}"),
    );
    let response = schema
        .execute(r#"query { docExtGroups { ext count } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let groups = response.data.into_json().unwrap()["docExtGroups"].clone();
    assert_eq!(
        groups,
        json!([{"ext": "PDF", "count": 2}, {"ext": "TXT", "count": 1}])
    );
}

#[tokio::test]
async fn video_and_doc_counts_count_the_catalogue_per_kind() {
    let (_dir, schema, _) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            db.record_media_files(&[
                media(DataType::Video, "/v/a.mp4", "a", 10, "b"),
                media(DataType::Video, "/v/b.mp4", "b", 10, "b"),
                media(DataType::Doc, "/d/a.pdf", "a", 10, "b"),
                media(DataType::Doc, "/d/b.pdf", "b", 10, "b"),
                media(DataType::Audio, "/m/a.mp3", "a", 10, "b"),
            ])
            .unwrap();
        },
        |method, _| panic!("unexpected host call {method}"),
    );
    let response = schema
        .execute(r#"query { videoCount(query:"") docCount(query:"") audioCount(query:"") }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["videoCount"], 2);
    assert_eq!(data["docCount"], 2);
    assert_eq!(data["audioCount"], 1);
}

/// An empty page must not cost a tag round trip.
#[tokio::test]
async fn an_empty_page_skips_the_tag_lookup() {
    let (_dir, schema) = fixture_with(GRANTED, true, |method, _| match method {
        "systemMediaRows" => json!([]),
        other => panic!("unexpected host call {other}"),
    });
    let response = schema
        .execute(r#"query { images(offset:0, limit:10, query:"", sortBy:NAME_ASC) { id } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["images"], json!([]));
}

#[test]
fn the_contract_kind_names_are_what_the_platform_expects() {
    assert_eq!(MediaDataType::AUDIO.as_str(), "AUDIO");
    assert_eq!(MediaDataType::VIDEO.as_str(), "VIDEO");
    assert_eq!(MediaDataType::IMAGE.as_str(), "IMAGE");
    assert_eq!(MediaDataType::DOC.as_str(), "DOC");
}

#[tokio::test]
async fn all_media_lists_accept_the_original_shared_interface_fragment() {
    let (_dir, schema, _) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            db.record_media_files(&[
                media(
                    DataType::Audio,
                    "/media/file.mp3",
                    "Media",
                    2048,
                    "bucket-1",
                ),
                media(
                    DataType::Video,
                    "/media/file.mp4",
                    "Media",
                    2048,
                    "bucket-1",
                ),
                media(DataType::Doc, "/media/file.pdf", "Media", 2048, "bucket-1"),
            ])
            .unwrap();
        },
        |method, _| match method {
            "systemMediaRows" => json!([{
                "id": "media-1", "title": "Media", "path": "/media/file",
                "size": 2048, "bucketId": "bucket-1",
                "createdAt": "2026-01-02T03:04:05.000Z", "updatedAt": "2026-02-03T04:05:06.000Z"
            }]),
            other => panic!("unexpected host call {other}"),
        },
    );
    let response = schema
        .execute(
            r#"
        query {
            audios(query: "", offset: 0, limit: 1, sortBy: DATE_DESC) { ...OriginalMedia }
            images(query: "", offset: 0, limit: 1, sortBy: DATE_DESC) { ...OriginalMedia }
            videos(query: "", offset: 0, limit: 1, sortBy: DATE_DESC) { ...OriginalMedia }
            docs(query: "", offset: 0, limit: 1, sortBy: DATE_DESC) { ...OriginalMedia }
        }
        fragment OriginalMedia on MediaItem { id title path size bucketId createdAt updatedAt }
    "#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    for field in ["audios", "videos", "docs"] {
        assert_eq!(data[field][0]["title"], "Media", "{field}: {data}");
        assert_eq!(data[field][0]["size"], 2048, "{field}: {data}");
        assert_eq!(data[field][0]["bucketId"], "/media", "{field}: {data}");
        assert_eq!(
            data[field][0].as_object().unwrap().len(),
            7,
            "{field}: {data}"
        );
    }
}

#[tokio::test]
async fn media_tags_are_loaded_only_when_selected_and_batch_all_rows() {
    use crate::graphql::resolvers::tags::loader::TagsLoader;
    use async_graphql::dataloader::DataLoader;
    use std::sync::atomic::Ordering;
    let (_dir, schema, db) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            db.record_media_files(&[
                media(DataType::Image, "/fixture/one.png", "one", 10, "images"),
                media(DataType::Image, "/fixture/two.png", "two", 10, "images"),
            ])
            .unwrap();
        },
        |method, _| panic!("unexpected host call {method}"),
    );
    let rows = db.media_file_list(DataType::Image, 10, 0).unwrap();
    let tag = plain_rs::tags::create_tag(&db, DataType::Image.kind(), "work").unwrap();
    plain_rs::tags::add_relations(
        &db,
        &rows
            .iter()
            .map(|row| (tag.id.clone(), row.id.clone()))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let loads = &schema
        .data::<DataLoader<TagsLoader>>()
        .unwrap()
        .loader()
        .loads;
    let response = schema
        .execute(r#"{ images(offset:0, limit:10, query:"", sortBy:NAME_ASC) { id } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["images"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(loads.load(Ordering::Relaxed), 0);
    let response = schema
        .execute(
            r#"{ images(offset:0, limit:10, query:"", sortBy:NAME_ASC) { id tags { name } } }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let rows = data["images"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row["tags"][0]["name"], "work");
    }
    assert_eq!(loads.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn image_search_uses_the_original_query_for_text_filters_paging_and_count() {
    let (_dir, schema, _) = fixture_seeded(
        GRANTED,
        true,
        |db| {
            db.record_media_files(&[
                media(
                    DataType::Image,
                    "/fixture/a.jpg",
                    "sunset blue a",
                    100,
                    "b1",
                ),
                media(
                    DataType::Image,
                    "/fixture/b.jpg",
                    "sunset blue b",
                    100,
                    "b1",
                ),
                media(
                    DataType::Image,
                    "/other-fixture/c.jpg",
                    "sunset blue c",
                    100,
                    "b2",
                ),
                media(DataType::Image, "/fixture/d.jpg", "sunset red", 100, "b1"),
            ])
            .unwrap();
        },
        |method, _| panic!("unexpected host call {method}"),
    );
    let response = schema
        .execute(
            async_graphql::Request::new(
                "query($query:String!){images(query:$query,offset:1,limit:1,sortBy:NAME_ASC){title} imageCount(query:$query)}",
            )
            .variables(async_graphql::Variables::from_json(json!({
                "query": "text:\"sunset blue\" bucket_id:/fixture trash:false"
            }))),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["images"], json!([{"title":"sunset blue b"}]));
    assert_eq!(data["imageCount"], 2);
}

#[tokio::test]
async fn loaded_image_search_preserves_filters_counts_and_semantic_order() {
    use crate::services::images::{index::ImageIndex, models::Runtime};
    use plain_rs::media::images::embeddings::{self, EmbeddingInput};
    use std::time::Duration;

    let (dir, schema, db) = fixture_seeded(
        GRANTED,
        true,
        |_| {},
        |method, _| panic!("unexpected host call {method}"),
    );
    let models = schema.data::<Arc<Runtime>>().unwrap();
    let index = schema.data::<Arc<ImageIndex>>().unwrap();
    index.use_native_catalog_for_test();
    plain_inference::models::test_fixture::fixture(&dir.path().join("ai_models/incoming"));
    models.import().await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while models.snapshot().status.status
            != crate::graphql::resolvers::images::public_index::ImageSearchStatusType::Ready
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        index.wait_until_idle().await;
    })
    .await
    .unwrap();

    // Seed after activation: its initial empty scan must not clear our
    // deterministic embeddings or try to decode these synthetic paths.
    let stored = db
        .record_media_files(&[
            media(DataType::Image, "/fixture/a.jpg", "cat", 100, "b1"),
            media(
                DataType::Image,
                "/fixture/z.jpg",
                "semantic first",
                100,
                "b1",
            ),
            media(
                DataType::Image,
                "/fixture/b.jpg",
                "semantic second",
                200,
                "b1",
            ),
            media(DataType::Image, "/fixture/c.jpg", "not a match", 100, "b1"),
            media(DataType::Image, "/fixture/d.jpg", "untagged", 100, "b1"),
            media(
                DataType::Image,
                "/other-fixture/e.jpg",
                "other folder",
                100,
                "b2",
            ),
            media(DataType::Image, "/fixture/f.jpg", "in trash", 100, "b1"),
        ])
        .unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE files SET trashed_at='2026-01-01T00:00:00Z',original_path=path WHERE id=?1",
            [&stored[6].id],
        )
    })
    .unwrap();
    let tag = plain_rs::tags::create_tag(&db, DataType::Image.kind(), "selected").unwrap();
    let empty_tag = plain_rs::tags::create_tag(&db, DataType::Image.kind(), "empty").unwrap();
    plain_rs::tags::add_relations(
        &db,
        &[0, 1, 2, 3, 5, 6].map(|i| (tag.id.clone(), stored[i].id.clone())),
    )
    .unwrap();
    let embedding = |id: String, path: String, score: f32| EmbeddingInput {
        id,
        path,
        embedding_base64: plain_rs::utils::base64::base64_encode(
            &[score, 0.0, 0.0, 0.0]
                .into_iter()
                .flat_map(f32::to_be_bytes)
                .collect::<Vec<_>>(),
        ),
    };
    let mut embeddings = [0, 1, 2, 4, 5, 6]
        .into_iter()
        .map(|i| {
            embedding(
                stored[i].id.clone(),
                stored[i].path.clone(),
                if i == 2 { 0.8 } else { 1.0 },
            )
        })
        .collect::<Vec<_>>();
    embeddings.push(embedding(
        "missing".into(),
        "/fixture/missing.jpg".into(),
        1.0,
    ));
    embeddings::save(&db, &embeddings).unwrap();
    assert_eq!(models.search("cat", 50).await.unwrap().len(), 7);

    let active = format!("text:cat tag_id:{} bucket_id:/fixture trash:false", tag.id);
    let cases = [
        (active.clone(), 0, 10, vec![0, 1, 2], 3),
        (
            format!("text:cat tag_id:{} bucket_id:/fixture", tag.id),
            0,
            10,
            vec![0, 1, 2],
            3,
        ),
        (
            format!("{active} excluded_dir:/fixture/z"),
            0,
            10,
            vec![0, 2],
            2,
        ),
        (active.clone(), 1, 1, vec![1], 3),
        (active.clone(), 2, 1, vec![2], 3),
        (active.clone(), 3, 1, vec![], 3),
        (active.clone(), 0, 0, vec![], 3),
        (format!("{active} size:>150"), 0, 10, vec![2], 1),
        (
            format!("text:cat tag_id:{} bucket_id:/fixture trash:true", tag.id),
            0,
            10,
            vec![6],
            1,
        ),
        (
            format!("text:cat ids:{} trash:false", stored[1].id),
            0,
            10,
            vec![1],
            1,
        ),
        (
            format!(
                "text:cat tag_id:{} bucket_id:/fixture trash:false",
                empty_tag.id
            ),
            0,
            10,
            vec![],
            0,
        ),
        ("text:cat ids: trash:false".into(), 0, 10, vec![], 0),
    ];
    let mut failures = Vec::new();
    for (query, offset, limit, expected, count) in cases {
        let response = schema.execute(
            async_graphql::Request::new(
                "query($query:String!,$offset:Int!,$limit:Int!){images(query:$query,offset:$offset,limit:$limit,sortBy:NAME_ASC){id} imageCount(query:$query)}",
            ).variables(async_graphql::Variables::from_json(json!({"query":query,"offset":offset,"limit":limit}))),
        ).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        let expected = json!({
            "images": expected.into_iter().map(|i| json!({"id":stored[i].id})).collect::<Vec<_>>(),
            "imageCount": count,
        });
        if data != expected {
            failures.push(format!(
                "{query}, offset={offset}, limit={limit}: expected {expected}, got {data}"
            ));
        }
    }
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = calls.clone();
    let host = Arc::new(Host::default());
    stub(host.clone(), move |method, params| {
        captured.lock().unwrap().push((method.to_owned(), params));
        json!([{"id":"page-one"},{"id":"page-two"}])
    });
    let catalog = crate::services::files::catalog::MediaCatalog::Platform { db, host };
    let page = crate::services::images::search::rows(
        &catalog, models, "cat", "text:cat", 100, 2, "NAME_ASC",
    )
    .await
    .unwrap();
    assert_eq!(page.len(), 2);
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "systemMediaRows");
    assert_eq!(calls[0].1["offset"], 100);
    assert_eq!(calls[0].1["limit"], 2);
    drop(calls);
    models.shutdown().await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
