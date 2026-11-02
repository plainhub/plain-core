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
        cover_image_id: None,
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
    let directory = tempfile::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    db.record_media_files(&[
        file(DataType::Audio, "/audio/one.mp3", "100% Song", 30),
        file(DataType::Audio, "/audio/two.mp3", "Other Song", 10),
        file(DataType::Image, "/photos/one.jpg", "100% Song", 10),
    ])
    .unwrap();
    assert_eq!(count(&db, DataType::Audio, "text:100%").unwrap(), 1);
    assert_eq!(
        rows(
            &db,
            DataType::Audio,
            "text:100%",
            FileSortBy::NameAsc,
            10,
            0
        )
        .unwrap()
        .len(),
        1
    );
    let rows = rows(&db, DataType::Audio, "", FileSortBy::SizeAsc, 1, 1).unwrap();
    assert_eq!(rows[0].size_bytes, 30);
    assert_eq!(count(&db, DataType::Audio, "artist:Artist").unwrap(), 2);
    assert!(count(&db, DataType::Audio, "unknown:selection").is_err());
}

#[test]
fn document_filters_and_explicit_trash_selection_use_current_catalog() {
    let directory = tempfile::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    let created = db
        .record_media_files(&[
            file(DataType::Doc, "/docs/one.pdf", "Document title", 30),
            file(DataType::Doc, "/docs/two.txt", "Another title", 10),
        ])
        .unwrap();
    assert_eq!(count(&db, DataType::Doc, "ext:pdf").unwrap(), 1);
    assert_eq!(
        count(&db, DataType::Doc, "type:application/pdf").unwrap(),
        1
    );
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE files SET trashed_at='2026-01-01T00:00:00Z',original_path=path WHERE id=?1",
            [&created[0].id],
        )
    })
    .unwrap();
    assert_eq!(count(&db, DataType::Doc, "").unwrap(), 1);
    assert_eq!(count(&db, DataType::Doc, "trash:true ext:pdf").unwrap(), 1);
    assert_eq!(
        rows(&db, DataType::Doc, "trash:true", FileSortBy::NameAsc, 10, 0).unwrap()[0].id,
        created[0].id
    );
}

#[test]
fn bucket_items_use_shared_visibility_and_original_folder_names() {
    let directory = tempfile::tempdir().unwrap();
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
    let items = bucket_item_facts(&db, DataType::Image).unwrap();
    assert_eq!(items.len() as i64, count(&db, DataType::Image, "").unwrap());
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["path"], "/first/Albums/recent.jpg");
    assert_eq!(items[0]["id"], "/first/Albums");
    assert_eq!(items[0]["name"], "Albums");
    assert!(items[0].get("sortName").is_none());
    assert_eq!(items[0]["size"], 9_000_000_000i64);
    assert_eq!(items[1]["id"], "/second/Albums");
    assert!(
        items
            .iter()
            .all(|item| item.as_object().unwrap().len() == 4)
    );
    assert!(bucket_item_facts(&db, DataType::Contact).is_err());
}

#[test]
fn document_bucket_items_keep_the_same_nonempty_document_filter_as_rows() {
    let directory = tempfile::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    db.record_media_files(&[
        file(DataType::Doc, "/docs/readme.txt", "Readme", 5),
        file(DataType::Doc, "/docs/empty.txt", "Empty", 0),
    ])
    .unwrap();
    let items = bucket_item_facts(&db, DataType::Doc).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["path"], "/docs/readme.txt");
    assert_eq!(items.len() as i64, count(&db, DataType::Doc, "").unwrap());
}

#[test]
fn action_ids_share_filters_and_explicit_from_trash_overrides_the_query_mode() {
    let directory = tempfile::tempdir().unwrap();
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
        ids(&db, DataType::Doc, "ext:pdf", false).unwrap(),
        vec![created[1].id.clone()]
    );
    assert_eq!(
        ids(&db, DataType::Doc, "trash:false ext:pdf", true).unwrap(),
        vec![created[0].id.clone()]
    );
    assert_eq!(
        ids(&db, DataType::Doc, "trash:true ext:pdf", false).unwrap(),
        vec![created[0].id.clone()]
    );
    assert!(ids(&db, DataType::Contact, "", false).is_err());
    assert!(ids(&db, DataType::Doc, "unknown:filter", false).is_err());
}

#[test]
fn bulk_selection_includes_every_row_beyond_ten_thousand_and_deduplicates_ids() {
    let directory = tempfile::tempdir().unwrap();
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
        ids(&db, DataType::Image, "text:select", false)
            .unwrap()
            .len(),
        10003
    );
    assert!(
        ids(&db, DataType::Audio, "text:select", false)
            .unwrap()
            .is_empty()
    );
    let selected = ids(
        &db,
        DataType::Image,
        &format!("ids:{},{}", rows[0].id, rows[0].id),
        false,
    )
    .unwrap();
    assert_eq!(selected, vec![rows[0].id.clone()]);
}

#[test]
fn document_extensions_keep_filters_and_combine_extension_case() {
    let directory = tempfile::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    db.record_media_files(&[
        file(DataType::Doc, "/documents/report.PDF", "Report", 30),
        file(DataType::Doc, "/documents/other.pdf", "Other", 30),
        file(DataType::Doc, "/elsewhere/notes.txt", "Notes", 30),
    ])
    .unwrap();
    assert_eq!(
        document_extensions(&db, "bucket_id:/documents").unwrap(),
        vec![("PDF".into(), 2)]
    );
    assert_eq!(
        document_extensions(&db, "text:report").unwrap(),
        vec![("PDF".into(), 1)]
    );
    assert!(document_extensions(&db, "trash:true").unwrap().is_empty());
}

#[test]
fn counting_one_media_kind_does_not_scan_unrelated_files() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let directory = tempfile::tempdir().unwrap();
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
    let total = count(&db, DataType::Image, "").unwrap();
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
fn counts_and_bucket_facts_use_a_covering_index_without_sorting_all_media() {
    let db = crate::infrastructure::db::Db::open(std::path::Path::new(":memory:")).unwrap();
    db.with_conn(|c|c.execute_batch("INSERT INTO volumes VALUES('v','v','/media',1); WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000) INSERT INTO files(id,volume_id,path,file_name,kind,updated_at,size_bytes) SELECT 'f'||x,'v','images/'||x||'.jpg',x||'.jpg','IMAGE','2026',100 FROM n;")).unwrap();
    let (base, args) = super::selection(&db, crate::features::enums::DataType::Image, "").unwrap();
    for sql in [
        base.replace("{columns}", "COUNT(*)"),
        format!(
            "{} ORDER BY updated_at DESC,file_id",
            base.replace("{columns}", "bucket_id,path,size_bytes")
        ),
    ] {
        let details = db
            .with_conn(|c| {
                c.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?
                    .query_map(rusqlite::params_from_iter(args.iter()), |r| {
                        r.get::<_, String>(3)
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap();
        assert!(
            details.iter().any(
                |detail| detail.contains("COVERING INDEX idx_files_kind_indexed_updated_at_id")
            ),
            "{details:?}"
        );
        assert!(
            details.iter().all(|detail| !detail.contains("TEMP B-TREE")),
            "{details:?}"
        );
    }
    assert_eq!(
        super::count(&db, crate::features::enums::DataType::Image, "").unwrap(),
        1000
    );
    assert_eq!(
        super::bucket_item_facts(&db, crate::features::enums::DataType::Image)
            .unwrap()
            .len(),
        1000
    );
}
