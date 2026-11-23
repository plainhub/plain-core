use super::*;
use crate::features::enums::DataType;
use crate::infrastructure::db::MediaFileMetadata;

fn seed(db: &Db, path: String, modified: String) {
    db.record_media_files(&[MediaFileMetadata {
        data_type: DataType::Image,
        title: "test".into(),
        artist: String::new(),
        album: String::new(),
        album_artist: String::new(),
        path,
        bucket_id: String::new(),
        size_bytes: 123,
        duration_ms: 0,
        width: 50,
        height: 50,
        rotation: 0,
        taken_at: None,
        created_at: modified.clone(),
        updated_at: modified,
    }])
    .unwrap();
}

#[test]
fn writes_each_batch_once_and_rejects_outdated_source_versions() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    let path = "/media/fixture.jpg".to_string();
    let first = "2026-10-08T12:00:00.000Z".to_string();
    let next = "2026-10-09T12:00:00.000Z".to_string();
    seed(&db, path.clone(), first);
    let mut old = read_batch(&db, "IMAGE", None).unwrap();
    assert_eq!(old.len(), 1);
    seed(&db, path.clone(), next.clone());
    let hash = vec![1; 20];
    assert_eq!(
        write_batch(&db, &[(old.pop().unwrap(), hash.clone())]).unwrap(),
        0
    );
    let pending = read_batch(&db, "IMAGE", None).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].modified, next);
    assert_eq!(
        write_batch(&db, &[(pending.into_iter().next().unwrap(), hash.clone())]).unwrap(),
        1
    );
    assert!(read_batch(&db, "IMAGE", None).unwrap().is_empty());
    seed(&db, path, "2026-10-10T12:00:00.000Z".into());
    assert_eq!(read_batch(&db, "IMAGE", None).unwrap().len(), 1);
}

#[test]
fn reads_disjoint_bounded_batches_by_time_and_id() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    for i in 0..BATCH_SIZE + 1 {
        seed(
            &db,
            format!("/media/{i}.jpg"),
            "2026-10-08T12:00:00.000Z".into(),
        );
    }
    let batch = read_batch(&db, "IMAGE", None).unwrap();
    assert_eq!(batch.len(), BATCH_SIZE);
    let last = batch.last().unwrap();
    let cursor = (last.modified.clone(), last.id.clone());
    let next = read_batch(&db, "IMAGE", Some(&cursor)).unwrap();
    assert_eq!(next.len(), 1);
    assert!(next[0].id < last.id);
}

#[tokio::test]
async fn backfill_populates_the_served_thumbnail_and_hash_with_one_source_decode() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(crate::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs
        .set("client_id", "706c6169-9052-8f83-87af-3929ef3aa462")
        .unwrap();
    let core = crate::test_runtime::Core::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[4; 32]),
        prefs,
    )
    .unwrap();
    let state = core.runtime_state();
    let path = dir.path().join("thumbhash-source.png");
    plain_libs::image::RgbImage::from_pixel(320, 240, plain_libs::image::Rgb([26, 98, 162]))
        .save(&path)
        .unwrap();
    let facts = crate::features::media::file_facts::read_file_metadata_with_availability(
        &path,
        DataType::Image,
    )
    .unwrap();
    let indexed = state.db.record_media_file_reads(&[facts]).unwrap();
    populate(&state).await.unwrap();
    assert!(read_batch(&state.db, "IMAGE", None).unwrap().is_empty());
    state.db.with_conn(|conn| {
        let stored: Vec<u8> = conn
            .query_row(
                "SELECT thumb_hash FROM files WHERE id=?1",
                [&indexed[0].id],
                |row| row.get(0),
            )
            .unwrap();
        assert!((5..=25).contains(&stored.len()));
    });
    let response = state
        .thumbnails
        .get_exact(
            &state,
            Request {
                path: path.to_str().unwrap().into(),
                width: 128,
                height: 128,
                center_crop: false,
                short_edge: None,
                file_id: String::new(),
                file_name: String::new(),
                access_order: None,
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert!(response.starts_with(b"\x89PNG\r\n\x1a\n") || response.starts_with(b"\xff\xd8"));
    populate(&state).await.unwrap();
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    core.stop_core().await;
}
