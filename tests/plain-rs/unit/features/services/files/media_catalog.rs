use super::*;
use crate::infrastructure::db::MediaFileMetadata;

fn file(data_type: DataType, path: &str, title: &str, size_bytes: i64) -> MediaFileMetadata {
    MediaFileMetadata {
        album: String::new(),
        album_artist: String::new(),

        data_type,
        path: path.into(),
        title: title.into(),
        artist: "Artist".into(),
        bucket_id: "bucket".into(),
        size_bytes,
        duration_ms: 10,
        width: 0,
        height: 0,
        rotation: 0,
        taken_at: None,
        created_at: "2026-10-12T00:00:00.000Z".into(),
        updated_at: "2026-10-12T00:00:00.000Z".into(),
    }
}

#[test]
fn rows_and_count_share_literal_filters_and_stable_pagination() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    db.record_media_files(&[
        file(DataType::Audio, "/audio/one.mp3", "100% Song", 30),
        file(DataType::Audio, "/audio/two.mp3", "Other Song", 10),
        file(DataType::Image, "/photos/one.jpg", "100% Song", 10),
    ])
    .unwrap();
    assert_eq!(count(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Audio, "text:100%").unwrap(), 1);
    assert_eq!(
        rows(
            &db, &crate::features::prefs::Prefs::in_memory(),
            DataType::Audio,
            "text:100%",
            MediaSortBy::NameAsc,
            10,
            0
        )
        .unwrap()
        .len(),
        1
    );
    let rows = rows(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Audio, "", MediaSortBy::SizeAsc, 1, 1).unwrap();
    assert_eq!(rows[0].size_bytes, 30);
    assert_eq!(count(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Audio, "artist:Artist").unwrap(), 2);
    assert!(count(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Audio, "unknown:selection").is_err());
}

#[test]
fn document_filters_and_explicit_trash_selection_use_current_catalog() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    let created = db
        .record_media_files(&[
            file(DataType::Doc, "/docs/one.pdf", "Document title", 30),
            file(DataType::Doc, "/docs/two.txt", "Another title", 10),
        ])
        .unwrap();
    assert_eq!(count(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Doc, "ext:pdf").unwrap(), 1);
    assert_eq!(
        count(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Doc, "type:application/pdf").unwrap(),
        1
    );
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE files SET trashed_at='2026-01-01T00:00:00Z',original_path=path,path='trash/renamed.pdf' WHERE id=?1",
            [&created[0].id],
        )
    })
    .unwrap();
    assert_eq!(count(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Doc, "").unwrap(), 1);
    assert_eq!(count(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Doc, "trash:true ext:pdf").unwrap(), 1);
    let trashed = rows(
        &db, &crate::features::prefs::Prefs::in_memory(),
        DataType::Doc,
        "trash:true",
        MediaSortBy::NameAsc,
        10,
        0,
    )
    .unwrap();
    assert_eq!(trashed[0].id, created[0].id);
    assert_eq!(trashed[0].title, "one.pdf");
}

#[test]
fn buckets_use_shared_visibility_and_original_folder_names() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    let mut older = file(
        DataType::Image,
        "/first/Albums/old.jpg",
        "Metadata title",
        1,
    );
    older.bucket_id = "/first/Albums".into();
    let mut recent = file(
        DataType::Image,
        "/first/Albums/recent.jpg",
        "Different title",
        9_000_000_000,
    );
    recent.bucket_id = older.bucket_id.clone();
    recent.updated_at = "2026-10-13T00:00:00.000Z".into();
    let mut other = file(DataType::Image, "/second/Albums/other.jpg", "Title", 7);
    other.bucket_id = "/second/Albums".into();
    let created = db
        .record_media_files(&[
            older,
            recent,
            other,
            file(DataType::Audio, "/first/Albums/song.mp3", "Song", 10),
        ])
        .unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE files SET trashed_at='2026-01-01T00:00:00Z',original_path=path WHERE id=?1",
            [&created[0].id],
        )
    })
    .unwrap();
    let result = buckets(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Image).unwrap();
    assert_eq!(
        result.iter().map(|b| b.item_count).sum::<u64>(),
        count(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Image, "").unwrap() as u64
    );
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].top_items, ["/first/Albums/recent.jpg"]);
    assert_eq!(result[0].id, "/first/Albums");
    assert_eq!(result[0].name, "Albums");
    assert_eq!(result[0].size, 9_000_000_000i64);
    assert_eq!(result[1].id, "/second/Albums");
    assert!(buckets(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Contact).is_err());
}

#[test]
fn document_buckets_keep_the_same_nonempty_document_filter_as_rows() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    db.record_media_files(&[
        file(DataType::Doc, "/docs/readme.txt", "Readme", 5),
        file(DataType::Doc, "/docs/empty.txt", "Empty", 0),
    ])
    .unwrap();
    let items = buckets(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Doc).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].top_items[0], "/docs/readme.txt");
    assert_eq!(items.len() as i64, count(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Doc, "").unwrap());
}

#[test]
fn action_ids_share_filters_and_explicit_from_trash_overrides_the_query_mode() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    let created = db
        .record_media_files(&[
            file(DataType::Doc, "/docs/one.pdf", "one", 20),
            file(DataType::Doc, "/docs/two.pdf", "two", 20),
            file(DataType::Doc, "/docs/three.txt", "three", 20),
        ])
        .unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE files SET trashed_at='2026-01-01T00:00:00Z',original_path=path WHERE id=?1",
            [&created[0].id],
        )
    })
    .unwrap();
    assert_eq!(
        ids(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Doc, "ext:pdf", false).unwrap(),
        vec![created[1].id.clone()]
    );
    assert_eq!(
        ids(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Doc, "trash:false ext:pdf", true).unwrap(),
        vec![created[0].id.clone()]
    );
    assert_eq!(
        ids(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Doc, "trash:true ext:pdf", false).unwrap(),
        vec![created[0].id.clone()]
    );
    assert!(ids(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Contact, "", false).is_err());
    assert!(ids(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Doc, "unknown:filter", false).is_err());
}

#[test]
fn bulk_selection_includes_every_row_beyond_ten_thousand_and_deduplicates_ids() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    let rows = db
        .record_media_files(
            &(0..10003)
                .map(|index| {
                    file(
                        DataType::Image,
                        &format!("/catalog/select-{index}.jpg"),
                        "selected",
                        1,
                    )
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
    assert_eq!(
        ids(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Image, "text:select", false)
            .unwrap()
            .len(),
        10003
    );
    assert!(
        ids(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Audio, "text:select", false)
            .unwrap()
            .is_empty()
    );
    let selected = ids(
        &db, &crate::features::prefs::Prefs::in_memory(),
        DataType::Image,
        &format!("ids:{},{}", rows[0].id, rows[0].id),
        false,
    )
    .unwrap();
    assert_eq!(selected, vec![rows[0].id.clone()]);
}

#[test]
fn document_extensions_keep_filters_and_combine_extension_case() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    db.record_media_files(&[
        file(DataType::Doc, "/documents/report.PDF", "Report", 30),
        file(DataType::Doc, "/documents/other.pdf", "Other", 30),
        file(DataType::Doc, "/elsewhere/notes.txt", "Notes", 30),
    ])
    .unwrap();
    assert_eq!(
        document_extensions(&db, &crate::features::prefs::Prefs::in_memory(), "bucket_id:/documents").unwrap(),
        vec![("PDF".into(), 2)]
    );
    assert_eq!(
        document_extensions(&db, &crate::features::prefs::Prefs::in_memory(), "text:report").unwrap(),
        vec![("PDF".into(), 1)]
    );
    assert!(document_extensions(&db, &crate::features::prefs::Prefs::in_memory(), "trash:true").unwrap().is_empty());
}

#[test]
fn counting_one_media_kind_does_not_scan_unrelated_files() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    db.with_conn(|connection|{
        connection.execute("INSERT INTO volumes(id,identity,path,is_available) VALUES('media-count','media-count','/media-count',1)",[]).unwrap();
        let volume: String = connection.query_row("SELECT id FROM volumes ORDER BY rowid DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        connection.execute("WITH RECURSIVE numbers(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM numbers WHERE n<2000) INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) SELECT printf('file-%06d',n),?1,'file-'||n,'file-'||n,'FILE','2026-01-01' FROM numbers",[&volume]).unwrap();
        connection.execute("INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) VALUES('file-'||'photo.jpg',?1,'photo.jpg','photo.jpg','IMAGE','2026-01-01')",[&volume]).unwrap();
    });
    let steps = Arc::new(AtomicUsize::new(0));
    let observed = steps.clone();
    db.with_conn(|connection| {
        connection.progress_handler(
            1,
            Some(move || {
                observed.fetch_add(1, Ordering::Relaxed);
                false
            }),
        )
    });
    let total = count(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Image, "").unwrap();
    assert_eq!(db.media_file_count(DataType::Image).unwrap(), 1);
    assert_eq!(db.media_file_list(DataType::Image, 1, 0).unwrap().len(), 1);
    db.with_conn(|connection| connection.progress_handler(0, None::<fn() -> bool>));
    assert_eq!(total, 1);
    assert!(
        steps.load(Ordering::Relaxed) < 500,
        "one media count executed {} VM steps",
        steps.load(Ordering::Relaxed)
    );
}

#[test]
fn counts_use_a_covering_index_without_sorting_all_media() {
    let db = crate::infrastructure::db::Db::open(std::path::Path::new(":memory:")).unwrap();
    db.with_conn(|c|c.execute_batch("INSERT INTO volumes(id,identity,path,is_available) VALUES('v','v','/media',1); WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000) INSERT INTO files(id,volume_id,path,file_name,kind,updated_at,size_bytes) SELECT 'f'||x,'v','images/'||x||'.jpg',x||'.jpg','IMAGE','2026',100 FROM n;")).unwrap();
    let (base, args) = super::selection(&db, &crate::features::prefs::Prefs::in_memory(), crate::features::enums::DataType::Image, "").unwrap();
    let sql = base.replace("{columns}", "COUNT(*)");
    let details = db
        .with_conn(|c| {
            c.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?
                .query_map(plain_libs::rusqlite::params_from_iter(args.iter()), |r| {
                    r.get::<_, String>(3)
                })?
                .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
        })
        .unwrap();
    assert!(
        details
            .iter()
            .any(|detail| detail.contains("COVERING INDEX idx_files_kind_indexed_updated_at_id")),
        "{details:?}"
    );
    assert!(
        details.iter().all(|detail| !detail.contains("TEMP B-TREE")),
        "{details:?}"
    );
    assert_eq!(
        super::count(&db, &crate::features::prefs::Prefs::in_memory(), crate::features::enums::DataType::Image, "").unwrap(),
        1000
    );
}

#[test]
fn video_pages_sorted_by_capture_time_read_only_a_bounded_part_of_the_catalog() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.with_conn(|conn| conn.execute_batch("INSERT INTO volumes(id,identity,path,is_available) VALUES('v','v','/media',1),('offline','offline','/offline',0);
WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<6000)
INSERT INTO files(id,volume_id,path,file_name,kind,updated_at)
SELECT printf('clip-%04d',x),'v',x||'.mp4',x||'.mp4','VIDEO','2026-01-01T00:00:00Z' FROM n;
INSERT INTO files(id,volume_id,path,file_name,kind,updated_at,created_at,taken_at,indexed) VALUES
('captured','v','captured.mp4','captured.mp4','VIDEO','2026-01-01T00:00:00Z',NULL,'2026-01-05T00:00:00Z',1),
('created','v','created.mp4','created.mp4','VIDEO','2026-01-02T00:00:00Z','2026-01-04T00:00:00Z',NULL,1),
('updated','v','updated.mp4','updated.mp4','VIDEO','2026-01-03T00:00:00Z',NULL,NULL,1),
('offline-video','offline','offline.mp4','offline.mp4','VIDEO','2026-01-08T00:00:00Z',NULL,NULL,1),
('unindexed','v','unindexed.mp4','unindexed.mp4','VIDEO','2026-01-08T00:00:00Z',NULL,NULL,0);"))
        .unwrap();
    let samples = Arc::new(AtomicUsize::new(0));
    db.with_conn(|conn| {
        let samples = samples.clone();
        conn.progress_handler(
            100,
            Some(move || {
                samples.fetch_add(1, Ordering::Relaxed);
                false
            }),
        );
    });
    let prefs = Prefs::in_memory();
    let first = rows(
        &db,
        &prefs,
        DataType::Video,
        "trash:false",
        MediaSortBy::TakenAtDesc,
        64,
        0,
    )
    .unwrap();
    let second = rows(
        &db,
        &prefs,
        DataType::Video,
        "trash:false",
        MediaSortBy::TakenAtDesc,
        64,
        64,
    )
    .unwrap();
    db.with_conn(|conn| conn.progress_handler(0, None::<fn() -> bool>));
    assert_eq!(first.len(), 64);
    assert_eq!(second.len(), 64);
    assert_eq!(
        first[..3]
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["captured", "created", "updated"]
    );
    assert_eq!(first[63].id, "clip-0061");
    assert_eq!(second[0].id, "clip-0062");
    let executed = samples.load(Ordering::Relaxed);
    assert!(
        executed < 200,
        "opening two 64-item pages took at least {} SQLite VM steps",
        executed * 100
    );
}

#[test]
fn name_title_modified_and_capture_time_have_distinct_ordering() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("sort.db")).unwrap();
    let mut a = file(DataType::Audio, "/audio/a.mp3", "Zulu", 10);
    let mut b = file(DataType::Audio, "/audio/b.mp3", "Alpha", 20);
    a.updated_at = "2026-01-02T00:00:00.000Z".into();
    b.updated_at = "2026-01-01T00:00:00.000Z".into();
    let mut photo = file(DataType::Image, "/photos/captured.jpg", "Captured", 10);
    photo.taken_at = Some("2026-01-01T00:00:00.000Z".into());
    let mut undated = file(DataType::Image, "/photos/undated.jpg", "Undated", 10);
    undated.created_at = "2026-01-03T00:00:00.000Z".into();
    db.record_media_files(&[a, b, photo, undated]).unwrap();
    for (sort, expected) in [
        (MediaSortBy::NameAsc, "/audio/a.mp3"),
        (MediaSortBy::TitleAsc, "/audio/b.mp3"),
        (MediaSortBy::UpdatedAtAsc, "/audio/b.mp3"),
    ] {
        assert_eq!(
            rows(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Audio, "", sort, 10, 0).unwrap()[0].path,
            expected
        );
    }
    assert_eq!(
        rows(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Image, "", MediaSortBy::TakenAtDesc, 10, 0).unwrap()[0].path,
        "/photos/undated.jpg"
    );
}

#[test]
fn sqlite_buckets_return_one_row_per_folder_with_four_ordered_covers() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.with_conn(|conn| conn.execute_batch("INSERT INTO volumes(id,identity,path,is_available) VALUES('v','v','/media',1);
WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000)
INSERT INTO files(id,volume_id,path,file_name,kind,updated_at,size_bytes)
SELECT printf('f%04d',x),'v','images/'||x||'.jpg',x||'.jpg','IMAGE',CASE WHEN x=2 THEN '2027' ELSE '2026' END,100 FROM n;")).unwrap();
    let (sql, args) = bucket_selection(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Image).unwrap();
    db.with_conn(|conn| {
        let mut statement = conn.prepare(&sql).unwrap();
        let mut rows = statement.query(params_from_iter(args.iter())).unwrap();
        let row = rows.next().unwrap().unwrap();
        assert_eq!(row.get::<_, String>(0).unwrap(), "/media/images");
        assert_eq!(row.get::<_, i64>(1).unwrap(), 1000);
        assert_eq!(row.get::<_, i64>(2).unwrap(), 100_000);
        assert_eq!(row.as_ref().column_count(), 7);
        assert!(
            rows.next().unwrap().is_none(),
            "SQLite must return one row, not 1000 file facts"
        );
    });
    let details = db
        .with_conn(|conn| {
            conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?
                .query_map(params_from_iter(args.iter()), |row| row.get::<_, String>(3))?
                .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
        })
        .unwrap();
    assert!(
        details
            .iter()
            .any(|detail| detail.contains("COVERING INDEX idx_files_kind_indexed_updated_at_id")),
        "{details:?}"
    );
    let result = buckets(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Image).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].item_count, 1000);
    assert_eq!(result[0].size, 100_000);
    assert_eq!(
        result[0].top_items,
        [
            "/media/images/2.jpg",
            "/media/images/1.jpg",
            "/media/images/3.jpg",
            "/media/images/4.jpg"
        ]
    );
}

#[test]
fn sqlite_buckets_preserve_visibility_types_sorting_and_empty_results() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    assert!(buckets(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Image).unwrap().is_empty());
    db.with_conn(|conn| conn.execute_batch("INSERT INTO volumes(id,identity,path,is_available) VALUES('v','v','/media',1),('offline','offline','/offline',0);
INSERT INTO files(id,volume_id,path,file_name,kind,updated_at,size_bytes,indexed,missing) VALUES
('a','v','Zulu/a.jpg','a.jpg','IMAGE','2026',3,1,0),
('b','v','中国/b.jpg','b.jpg','IMAGE','2026',4,1,0),
('unindexed','v','Zulu/unindexed.jpg','unindexed.jpg','IMAGE','2026',8,0,0),
('missing','v','Zulu/missing.jpg','missing.jpg','IMAGE','2026',16,1,1),
('offline','offline','offline.jpg','offline.jpg','IMAGE','2026',32,1,0),
('trashed','v','Zulu/trashed.jpg','trashed.jpg','IMAGE','2026',64,1,0),
('audio','v','Zulu/audio.mp3','audio.mp3','AUDIO','2026',0,1,0),
('video','v','Zulu/video.mp4','video.mp4','VIDEO','2026',0,1,0);
UPDATE files SET trashed_at='2026',original_path=path WHERE id='trashed';")).unwrap();
    let result = buckets(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Image).unwrap();
    assert_eq!(
        result.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
        ["/media/中国", "/media/Zulu"]
    );
    assert_eq!(
        result
            .iter()
            .map(|b| (b.item_count, b.size))
            .collect::<Vec<_>>(),
        [(1, 4), (1, 3)]
    );
    for kind in [DataType::Audio, DataType::Video] {
        let result = buckets(&db, &crate::features::prefs::Prefs::in_memory(), kind).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].item_count, 1);
        assert_eq!(result[0].size, 0);
    }
}

#[test]
fn sqlite_buckets_reject_integer_sum_overflow() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.record_media_files(&[
        file(DataType::Image, "/photos/large.jpg", "Large", i64::MAX),
        file(DataType::Image, "/photos/small.jpg", "Small", 1),
    ])
    .unwrap();
    assert!(buckets(&db, &crate::features::prefs::Prefs::in_memory(), DataType::Image).is_err());
}
