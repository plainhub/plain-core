use super::*;
use crate::features::{
    enums::DataType, prefs::settings::FileSortBy, services::files::media_catalog,
};

#[test]
fn photo_snapshot_populates_shared_image_queries_and_preserves_identity() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("db")).unwrap();
    let mut asset = Asset {
        id: "asset/L0/001".into(),
        name: "photo.jpg".into(),
        media_type: 1,
        duration_ms: 0,
        width: 800,
        height: 600,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
        favorite: false,
    };
    replace(&db, std::slice::from_ref(&asset)).unwrap();
    let rows = media_catalog::rows(&db, DataType::Image, "", FileSortBy::NameAsc, 10, 0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].path, "ph://asset/L0/001");
    assert_eq!(rows[0].size_bytes, 0);
    assert_eq!(rows[0].bucket_id, "ph://photos");
    let buckets = media_catalog::bucket_item_facts(&db, DataType::Image).unwrap();
    assert_eq!(buckets[0]["name"], "photos");
    let id = rows[0].id.clone();
    assert_eq!(db.media_resource_path(&id).unwrap(), rows[0].path);
    let snapshot = crate::infrastructure::db::image_catalog::snapshot(&db).unwrap();
    assert_eq!(
        crate::infrastructure::db::image_catalog::page(&db, &snapshot.revision, "", 10)
            .unwrap()
            .items[0]
            .path,
        rows[0].path
    );
    let scope = crate::features::files::index_scope::Scope::load(
        &db,
        &directory.path().join("private"),
        &directory.path().join("cache"),
        &[],
        crate::features::files::index_scope::Defaults {
            excluded_directories: vec![],
            excluded_names: vec![],
        },
    )
    .unwrap();
    for paths in [None, Some(vec!["/".to_owned()])] {
        db.update_index_visibility(&scope, paths.as_deref())
            .unwrap();
        assert_eq!(media_catalog::count(&db, DataType::Image, "").unwrap(), 1);
    }
    db.with_conn(|conn| conn.execute("UPDATE files SET indexed=0 WHERE volume_id='photos'", []))
        .unwrap();
    asset.name = "edited.jpg".into();
    asset.favorite = true;
    asset.created_at = "2026-01-02T00:00:00Z".into();
    replace(&db, &[asset]).unwrap();
    let rows = media_catalog::rows(&db, DataType::Image, "", FileSortBy::NameAsc, 10, 0).unwrap();
    assert_eq!(rows[0].id, id);
    assert_eq!(rows[0].title, "edited.jpg");
    assert!(rows[0].favorited);
    assert!(
        rows[0]
            .taken_at
            .as_deref()
            .unwrap()
            .starts_with("2026-01-02")
    );
    db.refresh_volume_availability().unwrap();
    assert_eq!(media_catalog::count(&db, DataType::Image, "").unwrap(), 1);
    replace(&db, &[]).unwrap();
    assert_eq!(media_catalog::count(&db, DataType::Image, "").unwrap(), 0);
}

#[tokio::test]
async fn photo_queries_use_raw_pages_once_per_revision_and_remove_hidden_assets() {
    use std::sync::Arc;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("db")).unwrap();
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let replies = host.clone();
    let worker = tokio::spawn(async move {
        for (method, value) in [
            ("photoCatalogSnapshot", json!({"revision":"1","total":1})),
            (
                "photoCatalogPage",
                json!([{"id":"asset/L0/001","name":"photo.jpg","mediaType":1,"durationMs":0,"width":800,"height":600,"createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z","favorite":false}]),
            ),
            ("photoCatalogVerify", json!(true)),
            ("photoCatalogSnapshot", json!({"revision":"1","total":1})),
            ("photoCatalogSnapshot", json!({"revision":"1","total":0})),
            ("photoCatalogVerify", json!(true)),
        ] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], method);
            replies
                .reply(generation, json!({"id":request["id"],"result":value}))
                .unwrap();
        }
    });
    let catalog = PhotoCatalog::default();
    catalog.sync(&db, &host).await.unwrap();
    assert_eq!(media_catalog::count(&db, DataType::Image, "").unwrap(), 1);
    catalog.sync(&db, &host).await.unwrap();
    assert_eq!(media_catalog::count(&db, DataType::Image, "").unwrap(), 1);
    catalog.sync(&db, &host).await.unwrap();
    assert_eq!(media_catalog::count(&db, DataType::Image, "").unwrap(), 0);
    worker.await.unwrap();
}

#[test]
fn photo_snapshot_stores_videos_in_the_shared_catalog_without_copying() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("db")).unwrap();
    let assets: Vec<Asset> = plain_libs::serde_json::from_value(json!([
        {"id":"photo/L0/001","name":"photo.jpg","mediaType":1,"durationMs":0,"width":800,"height":600,"createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z","favorite":false},
        {"id":"video/L0/001","name":"video.mov","mediaType":2,"durationMs":1250,"width":1920,"height":1080,"createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z","favorite":true}
    ])).unwrap();
    replace(&db, &assets).unwrap();
    let videos = media_catalog::rows(&db, DataType::Video, "", FileSortBy::NameAsc, 10, 0).unwrap();
    assert_eq!(videos.len(), 1);
    assert_eq!(videos[0].path, "ph://video/L0/001");
    assert_eq!(videos[0].size_bytes, 0);
    assert_eq!(videos[0].duration_ms, 1250);
    assert_eq!(videos[0].width, 1920);
    assert_eq!(media_catalog::count(&db, DataType::Image, "").unwrap(), 1);
    assert_eq!(
        db.media_resource_path(&videos[0].id).unwrap(),
        videos[0].path
    );
    replace(&db, &assets[..1]).unwrap();
    assert_eq!(media_catalog::count(&db, DataType::Video, "").unwrap(), 0);
}
