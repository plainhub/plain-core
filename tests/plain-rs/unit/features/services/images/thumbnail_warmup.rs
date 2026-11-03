use super::*;
use crate::features::enums::DataType;

#[test]
fn cache_trimming_preserves_non_thumbnail_entries() {
    let directory = tempfile::tempdir().unwrap();
    let old = directory.path().join("old.thumb");
    std::fs::File::create(&old)
        .unwrap()
        .set_len(257 * 1024 * 1024)
        .unwrap();
    let original = directory.path().join("original.txt");
    std::fs::write(&original, b"original").unwrap();
    trim_cache(directory.path()).unwrap();
    assert!(!old.exists());
    assert_eq!(std::fs::read(original).unwrap(), b"original");
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn index_updates_prewarm_the_same_cache_used_by_foreground_requests() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.jpg");
    image::RgbImage::from_pixel(800, 600, image::Rgb([220, 40, 30]))
        .save(&path)
        .unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let catalog =
        crate::features::services::files::catalog_scan::NativeMediaCatalog::new(state.clone());
    catalog
        .scan_file(DataType::Image, path.to_str().unwrap())
        .await
        .unwrap();
    let request = Request {
        path: path.to_str().unwrap().into(),
        file_id: "different-caller-id".into(),
        file_name: "caller-title.jpg".into(),
        width: 512,
        height: 512,
        center_crop: true,
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::SeqCst)
            < 2
        {
            tokio::task::yield_now().await;
        }
        while state.thumbnails.queue.waiting() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let before = state
        .thumbnails
        .native_decodes
        .load(std::sync::atomic::Ordering::SeqCst);
    for size in [96, 256] {
        let bytes = state
            .thumbnails
            .get(
                &state,
                Request {
                    width: size,
                    height: size,
                    ..request.clone()
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(image::load_from_memory(&bytes).unwrap().width(), size);
        assert_eq!(
            state
                .thumbnails
                .native_decodes
                .load(std::sync::atomic::Ordering::SeqCst),
            before
        );
    }
    core.stop_core().await;
}

#[cfg(unix)]
#[test]
fn cache_trimming_refuses_a_symlink_root_without_touching_its_target() {
    let cache = tempfile::tempdir().unwrap();
    let original = tempfile::tempdir().unwrap();
    let path = original.path().join("photo.jpg");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(257 * 1024 * 1024)
        .unwrap();
    let link = cache.path().join("thumbs");
    std::os::unix::fs::symlink(original.path(), &link).unwrap();
    assert_eq!(
        trim_cache(&link).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(std::fs::metadata(path).unwrap().len(), 257 * 1024 * 1024);
}

#[tokio::test]
async fn warmup_queries_do_not_visit_unrelated_catalog_rows() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let task = state.thumbnails.warmer.lock().unwrap().take().unwrap();
    task.abort();
    let _ = task.await;
    state.db.with_conn(|connection| {
        connection.execute("INSERT INTO volumes(id,identity,path,is_available) VALUES('warmup-test','warmup-test','/warmup-test',1)", []).unwrap();
        let volume: String = connection.query_row("SELECT id FROM volumes ORDER BY rowid DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        connection.execute("WITH RECURSIVE numbers(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM numbers WHERE n<2000) INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) SELECT printf('file-%06d',n),?1,'other/'||n,'file-'||n,'FILE','2026-01-01' FROM numbers", [&volume]).unwrap();
    });
    let steps = Arc::new(AtomicUsize::new(0));
    let observed = steps.clone();
    state.db.with_conn(|connection| {
        connection.progress_handler(
            100,
            Some(move || {
                observed.fetch_add(100, Ordering::Relaxed);
                false
            }),
        )
    });
    warm_catalog(&state).await.unwrap();
    state
        .db
        .with_conn(|connection| connection.progress_handler(0, None::<fn() -> bool>));
    assert!(
        steps.load(Ordering::Relaxed) < 5000,
        "background queries executed {} VM steps for an empty media catalog",
        steps.load(Ordering::Relaxed)
    );
    core.stop_core().await;
}

#[test]
fn indexed_cursor_pages_cover_tied_dates_and_exclude_unavailable_sources() {
    let directory = tempfile::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    db.with_conn(|connection| {
        connection.execute("INSERT INTO volumes(id,identity,path,is_available) VALUES('page-live','page-live','/media',1)", []).unwrap();
        let volume: String = connection.query_row("SELECT id FROM volumes ORDER BY rowid DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        connection.execute("WITH RECURSIVE numbers(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM numbers WHERE n<67) INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) SELECT printf('file-%06d',n),?1,'photo-'||n,'photo-'||n,'IMAGE',CASE WHEN n<34 THEN '2026-01-02' ELSE '2026-01-01' END FROM numbers", [&volume]).unwrap();
        for (name,kind,indexed,missing,trashed) in [("audio","AUDIO",1,0,false),("unindexed","IMAGE",0,0,false),("missing","IMAGE",1,1,false),("trash","IMAGE",1,0,true)] {
            connection.execute("INSERT INTO files(id,volume_id,path,file_name,kind,updated_at,indexed,missing,trashed_at,original_path) VALUES('file-'||?2,?1,?2,?2,?3,'2026-02-01',?4,?5,?6,?7)", rusqlite::params![&volume,name,kind,indexed,missing,trashed.then_some("2026-02-01"),trashed.then_some(name)]).unwrap();
        }
        connection.execute("INSERT INTO volumes(id,identity,path,is_available) VALUES('page-offline','page-offline','/offline',0)", []).unwrap();
        connection.execute("INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) VALUES('file-'||'offline.jpg',?1,'offline.jpg','offline.jpg','IMAGE','2026-02-01')", ["page-offline"]).unwrap();
    });
    let mut cursor = None;
    let mut actual = Vec::new();
    loop {
        let rows = page(&db, FileKind::Image, cursor.as_ref()).unwrap();
        assert!(rows.len() <= 32);
        if rows.is_empty() {
            break;
        }
        for row in rows {
            assert!(row.path.starts_with("/media/photo-"));
            cursor = Some(Cursor {
                updated_at: row.updated_at.clone(),
                id: row.id.clone(),
            });
            actual.push((row.updated_at, row.id));
        }
    }
    assert_eq!(actual.len(), 67);
    assert!(actual.windows(2).all(|pair| pair[0] > pair[1]));
    assert_eq!(
        actual
            .iter()
            .map(|row| &row.1)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        67
    );
    assert_eq!(page(&db, FileKind::Audio, None).unwrap().len(), 1);
    assert!(page(&db, FileKind::Video, None).unwrap().is_empty());
}

#[test]
fn a_deep_cursor_seeks_directly_to_the_requested_index_range() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let directory = tempfile::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    db.with_conn(|connection| {
        connection.execute("INSERT INTO volumes(id,identity,path,is_available) VALUES('deep-page','deep-page','/deep-page',1)", []).unwrap();
        let volume: String = connection.query_row("SELECT id FROM volumes ORDER BY rowid DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        connection.execute("WITH RECURSIVE numbers(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM numbers WHERE n<2000) INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) SELECT printf('file-%06d',n),?1,'photo-'||n,'photo-'||n,'IMAGE','2026-01-01' FROM numbers", [&volume]).unwrap();
    });
    let steps = Arc::new(AtomicUsize::new(0));
    let observed = steps.clone();
    db.with_conn(|connection| {
        connection.progress_handler(
            100,
            Some(move || {
                observed.fetch_add(100, Ordering::Relaxed);
                false
            }),
        )
    });
    let rows = page(
        &db,
        FileKind::Image,
        Some(&Cursor {
            updated_at: "2026-01-01".into(),
            id: "file-000100".into(),
        }),
    )
    .unwrap();
    db.with_conn(|connection| connection.progress_handler(0, None::<fn() -> bool>));
    assert_eq!(rows.len(), 32);
    assert_eq!(rows.first().unwrap().id, "file-000099");
    assert_eq!(rows.last().unwrap().id, "file-000068");
    assert!(
        steps.load(Ordering::Relaxed) < 5000,
        "cursor query executed {} VM steps",
        steps.load(Ordering::Relaxed)
    );
}

#[test]
fn prewarming_reserves_foreground_capacity() {
    assert_eq!([1, 2, 3, 4, 16].map(background_workers), [1, 1, 2, 3, 3]);
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn foreground_decode_can_start_while_background_decoders_are_occupied() {
    use std::sync::{Arc, atomic::Ordering};
    let dir = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(&dir.path().join("core")).unwrap();
    let state = core.runtime_state();
    let workers = background_workers(std::thread::available_parallelism().map_or(1, |n| n.get()));
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *state.thumbnails.decode_gate.lock().unwrap() = Some(gate.clone());
    let mut paths = Vec::new();
    for n in 0..=workers {
        let path = dir.path().join(format!("source-{n}.jpg"));
        image::RgbImage::from_pixel(800, 600, image::Rgb([20, 100, 60]))
            .save(&path)
            .unwrap();
        paths.push(path.to_string_lossy().into_owned());
    }
    state
        .db
        .record_file_paths(&paths[..workers], Some(FileKind::Image))
        .unwrap();
    let background = tokio::spawn({
        let state = state.clone();
        async move { warm_catalog(&state).await }
    });
    let wait_for = |count| {
        let state = state.clone();
        async move {
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                while state.thumbnails.native_decodes.load(Ordering::SeqCst) < count {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        }
    };
    wait_for(workers).await;
    let request = Request {
        path: paths.pop().unwrap(),
        width: 256,
        height: 256,
        center_crop: true,
        file_id: String::new(),
        file_name: String::new(),
    };
    let foreground = tokio::spawn({
        let state = state.clone();
        async move { state.thumbnails.get(&state, request).await }
    });
    wait_for(workers + 1).await;
    gate.add_permits(2 * workers + 1);
    assert!(foreground.await.unwrap().unwrap().is_some());
    background.await.unwrap().unwrap();
    core.stop_core().await;
}
