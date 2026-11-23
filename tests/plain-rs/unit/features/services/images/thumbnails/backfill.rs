use super::*;
use crate::features::enums::DataType;
use crate::features::services::images::thumbnails::Request;
use crate::infrastructure::db::MediaFileMetadata;
use std::sync::Arc;

fn seed(db: &Db, path: String, modified: String, data_type: DataType) {
    db.record_media_files(&[MediaFileMetadata {
        data_type,
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
    seed(&db, path.clone(), first, DataType::Image);
    let mut old = read_batch(&db, None).unwrap();
    assert_eq!(old.len(), 1);
    seed(&db, path.clone(), next.clone(), DataType::Image);
    let hash = Bytes::from_static(&[1; 20]);
    assert_eq!(
        write_batch(&db, &[(old.pop().unwrap(), hash.clone())]).unwrap(),
        Vec::<i32>::new()
    );
    let pending = read_batch(&db, None).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].modified, next);
    assert_eq!(
        write_batch(&db, &[(pending.into_iter().next().unwrap(), hash.clone())]).unwrap(),
        [DataType::Image.kind()]
    );
    assert!(read_batch(&db, None).unwrap().is_empty());
    seed(
        &db,
        path,
        "2026-10-10T12:00:00.000Z".into(),
        DataType::Image,
    );
    assert_eq!(read_batch(&db, None).unwrap().len(), 1);
}

#[test]
fn reads_one_mixed_batch_with_indexed_pagination() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    for i in 0..BATCH_SIZE + 1 {
        let (data_type, suffix) = if i % 2 == 0 {
            (DataType::Image, "jpg")
        } else {
            (DataType::Video, "mp4")
        };
        seed(
            &db,
            format!("/media/{i}.{suffix}"),
            "2026-10-08T12:00:00.000Z".into(),
            data_type,
        );
    }

    let batch = read_batch(&db, None).unwrap();
    assert_eq!(batch.len(), BATCH_SIZE);
    assert!(
        batch
            .iter()
            .any(|candidate| candidate.kind == FileKind::Image)
    );
    assert!(
        batch
            .iter()
            .any(|candidate| candidate.kind == FileKind::Video)
    );
    let last = batch.last().unwrap();
    let cursor = (last.modified.clone(), last.id.clone());
    let next = read_batch(&db, Some(&cursor)).unwrap();
    assert_eq!(next.len(), 1);
    assert!(next[0].id < last.id);

    let plan = db
        .with_conn(|conn| {
            conn.prepare(&format!("EXPLAIN QUERY PLAN {READ_BATCH}"))?
                .query_map(params!["9999", "", BATCH_SIZE as i64], |row| {
                    row.get::<_, String>(3)
                })?
                .collect::<SqlResult<Vec<_>>>()
        })
        .unwrap();
    assert_eq!(
        plan.iter()
            .filter(|line| line.contains("idx_files_thumb_hash_pending"))
            .count(),
        2,
        "{plan:?}"
    );
    assert!(
        !plan.iter().any(|line| line.contains("TEMP B-TREE")),
        "{plan:?}"
    );
}

#[tokio::test]
async fn backfill_only_stores_hash_and_leaves_thumbnail_generation_to_requests() {
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
    plain_libs::image::RgbImage::from_pixel(800, 600, plain_libs::image::Rgb([26, 98, 162]))
        .save(&path)
        .unwrap();
    let facts = crate::features::media::file_facts::read_file_metadata_with_availability(
        &path,
        DataType::Image,
    )
    .unwrap();
    let indexed = state.db.record_media_file_reads(&[facts]).unwrap();
    populate(&state).await.unwrap();
    assert!(read_batch(&state.db, None).unwrap().is_empty());
    assert_eq!(
        state
            .thumbnails
            .cache_bytes_written
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
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
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst),
        0,
    );
    for (pixels, short_edge) in [(128, None), (512, Some(256))] {
        let response = state
            .thumbnails
            .get_exact(
                &state,
                Request {
                    path: path.to_str().unwrap().into(),
                    width: pixels,
                    height: pixels,
                    center_crop: false,
                    short_edge,
                    file_id: String::new(),
                    file_name: String::new(),
                    access_order: None,
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert!(response.starts_with(b"\x89PNG\r\n\x1a\n") || response.starts_with(b"\xff\xd8"));
    }
    populate(&state).await.unwrap();
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst),
        2
    );
    core.stop_core().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn cold_video_thumb_hash_batch_benchmark() {
    use std::time::Instant;

    let clips: [&[u8]; 2] = [
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/testdata/video-h264-baseline.mp4"
        )),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/testdata/video-h265-hvc1.mp4"
        )),
    ];
    let mut elapsed_ms = Vec::new();
    for run in 0..6 {
        let dir = plain_libs::temporary::tempdir().unwrap();
        let core = crate::test_runtime::Core::open_with_cache(
            &dir.path().join("data"),
            &dir.path().join("cache"),
        )
        .unwrap();
        let state = core.runtime_state();
        let facts: Vec<_> = (0..BATCH_SIZE)
            .map(|i| {
                let path = dir.path().join(format!("indexed-{i}.mp4"));
                std::fs::write(&path, clips[i % clips.len()]).unwrap();
                crate::features::media::file_facts::read_file_metadata_with_availability(
                    &path,
                    DataType::Video,
                )
                .unwrap()
            })
            .collect();
        assert_eq!(
            state.db.record_media_file_reads(&facts).unwrap().len(),
            BATCH_SIZE
        );
        let start = Instant::now();
        populate(&state).await.unwrap();
        if run > 0 {
            elapsed_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let stored = state
            .db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT count(thumb_hash) FROM files WHERE kind='VIDEO'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
            })
            .unwrap();
        assert_eq!(stored, BATCH_SIZE as i64);
        core.stop_core().await;
    }
    elapsed_ms.sort_by(f64::total_cmp);
    println!(
        "rows={BATCH_SIZE} trials=5 cold_median_ms={:.2}",
        elapsed_ms[2]
    );
}

#[tokio::test]
async fn commits_only_full_successful_batches_and_the_final_partial_batch() {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    let dir = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open_with_cache(
        &dir.path().join("data"),
        &dir.path().join("cache"),
    )
    .unwrap();
    let state = core.runtime_state();
    let fixture = dir.path().join("fixture.png");
    plain_libs::image::RgbImage::from_pixel(80, 60, plain_libs::image::Rgb([12, 44, 91]))
        .save(&fixture)
        .unwrap();

    let facts: Vec<_> = (0..BATCH_SIZE + 8)
        .map(|index| {
            let source = dir.path().join(format!("image-{index}.png"));
            std::fs::copy(&fixture, &source).unwrap();
            crate::features::media::file_facts::read_file_metadata_with_availability(
                &source,
                DataType::Image,
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        state.db.record_media_file_reads(&facts).unwrap().len(),
        BATCH_SIZE + 8
    );

    let first = read_batch(&state.db, None).unwrap();
    let changes = Arc::new(AtomicUsize::new(0));
    let committed = Arc::new(Mutex::new(Vec::new()));
    state.db.with_conn(|conn| {
        for candidate in &first[..5] {
            conn.execute(
                "UPDATE files SET size_bytes=size_bytes+1 WHERE id=?1",
                [&candidate.id],
            )
            .unwrap();
        }
        let source_updates = changes.clone();
        conn.update_hook(Some(move |_, _: &str, table: &str, _| {
            if table == "files" {
                source_updates.fetch_add(1, Ordering::Relaxed);
            }
        }))
        .unwrap();
        let batch_sizes = committed.clone();
        conn.commit_hook(Some(move || {
            let count = changes.swap(0, Ordering::Relaxed);
            if count != 0 {
                batch_sizes.lock().unwrap().push(count);
            }
            false
        }))
        .unwrap();
    });

    populate(&state).await.unwrap();
    assert_eq!(*committed.lock().unwrap(), [BATCH_SIZE, 3]);
    assert_eq!(
        state.thumbnails.cache_bytes_written.load(Ordering::Relaxed),
        0
    );
    core.stop_core().await;
}
