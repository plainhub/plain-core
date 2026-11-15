use super::*;
use std::path::Path;

#[test]
fn changed_locations_seek_each_file_by_its_unique_id() {
    let db = super::super::Db::open(Path::new(":memory:")).unwrap();
    db.with_conn(|conn| {
        let plan = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {}", &*CHANGED_LOCATIONS))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert!(
            plan.iter()
                .any(|line| line.contains("SEARCH f ") && line.contains("(id=?)")),
            "{plan:?}"
        );
    });
}


#[test]
fn recording_ordinary_scan_files_does_not_scan_existing_media_rows() {
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    let fixture = plain_libs::temporary::tempdir().unwrap();
    let first = fixture.path().join("first.txt");
    let second = fixture.path().join("second.txt");
    std::fs::write(&first, b"generated first").unwrap();
    std::fs::write(&second, b"generated second").unwrap();
    let db = super::super::Db::open(Path::new(":memory:")).unwrap();
    let volume_id = db.register_volume(fixture.path()).unwrap().unwrap();
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction().unwrap();
        for index in 0..512 {
            tx.execute("INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) VALUES(?1,?2,?3,?4,'IMAGE','2026-01-01T00:00:00Z')",
                plain_libs::rusqlite::params![format!("seed-{index}"), volume_id, format!("generated/{index}.jpg"), format!("{index}.jpg")]).unwrap();
        }
        tx.commit().unwrap();
    });
    let steps = Arc::new(AtomicUsize::new(0));
    let count = steps.clone();
    db.with_conn(|conn| conn.progress_handler(1, Some(move || { count.fetch_add(1, Ordering::Relaxed); false })));
    db.record_scan_paths(&[
        (first.to_str().unwrap().into(), crate::infrastructure::db::FileKind::File),
        (second.to_str().unwrap().into(), crate::infrastructure::db::FileKind::File),
    ], "generated-scan").unwrap();
    db.with_conn(|conn| conn.progress_handler(0, None::<fn() -> bool>));
    let steps = steps.load(Ordering::Relaxed);
    println!("two ordinary files: {steps} SQLite VM steps with 512 unrelated media rows");
    assert!(steps < 5000, "two ordinary files used {steps} SQLite VM steps with unrelated media rows");
    db.with_conn(|conn| {
        let rows: i64 = conn.query_row("SELECT count(*) FROM files WHERE last_seen_scan_id='generated-scan'", [], |row| row.get(0)).unwrap();
        assert_eq!(rows, 2);
    });
    assert!(db.media_resource_path("seed-0").is_ok());
}

#[test]
fn replacement_conflicts_remove_old_media_paths_and_retain_new_media_paths() {
    for (columns, values, replacement_is_media) in [
        ("rowid,id,volume_id,path,file_name,kind,updated_at", "41,'replacement','fixture','new.txt','new.txt','FILE','2026-01-01T00:00:00Z'", false),
        ("id,volume_id,path,file_name,kind,updated_at", "'original','fixture','new.txt','new.txt','FILE','2026-01-01T00:00:00Z'", false),
        ("id,volume_id,path,file_name,kind,updated_at", "'replacement','fixture','original.jpg','original.jpg','IMAGE','2026-01-01T00:00:00Z'", true),
    ] {
        let db = super::super::Db::open(Path::new(":memory:")).unwrap();
        db.with_conn(|conn| conn.execute_batch("INSERT INTO volumes(id,identity,path,is_available) VALUES('fixture','fixture','/fixture',1); INSERT INTO files(rowid,id,volume_id,path,file_name,kind,updated_at) VALUES(41,'original','fixture','original.jpg','original.jpg','IMAGE','2026-01-01T00:00:00Z');")).unwrap();
        assert_eq!(db.media_resource_path("original").unwrap(), "/fixture/original.jpg");
        db.with_conn(|conn| conn.execute(&format!("INSERT OR REPLACE INTO files({columns}) VALUES({values})"), [])).unwrap();
        assert!(db.media_resource_path("original").is_err());
        if replacement_is_media {
            assert_eq!(db.media_resource_path("replacement").unwrap(), "/fixture/original.jpg");
            db.with_conn(|conn| conn.execute("UPDATE files SET path='moved.jpg' WHERE id='replacement'", [])).unwrap();
            assert_eq!(db.media_resource_path("replacement").unwrap(), "/fixture/moved.jpg");
            db.with_conn(|conn| conn.execute("DELETE FROM files WHERE id='replacement'", [])).unwrap();
            assert!(db.media_resource_path("replacement").is_err());
        }
    }
}
