use super::*;

#[test]
fn file_paths_are_relative_and_preserve_user_state() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("你好.txt");
    std::fs::write(&path, b"hello").unwrap();
    let db = Db::open(Path::new(":memory:")).unwrap();
    let first = db
        .record_file_paths(&[path.to_str().unwrap().into()], None)
        .unwrap()
        .remove(0);
    for value in [&first.id, &first.volume_id] {
        let id = uuid::Uuid::parse_str(value).unwrap();
        assert_eq!(id.get_version_num(), 7);
        assert_eq!(id.get_variant(), uuid::Variant::RFC4122);
    }
    assert!(!Path::new(&first.path).is_absolute());
    db.with_conn(|conn| conn.execute("UPDATE files SET favorited=1 WHERE id=?1", [&first.id]))
        .unwrap();
    db.with_conn(|conn|conn.execute_batch("CREATE TRIGGER reject_volume_rewrite BEFORE UPDATE ON volumes WHEN NEW.path=OLD.path AND NEW.is_available=OLD.is_available BEGIN SELECT RAISE(ABORT,'unchanged volume rewritten'); END;")).unwrap();
    let again = db
        .record_file_paths(&[path.to_str().unwrap().into()], None)
        .unwrap()
        .remove(0);
    assert_eq!(again.id, first.id);
    assert!(again.favorited);
    assert_eq!(again.duration_ms, None);
    let volume = db.volumes().unwrap().remove(0);
    assert_eq!(
        Path::new(&volume.path).join(again.path),
        locate(path.to_str().unwrap()).unwrap().path
    );
}

#[test]
fn fts_updates_and_deletes_follow_the_file_transaction() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    db.with_conn(|conn| {
        conn.execute("INSERT INTO volumes(id,identity,path,is_available) VALUES('TEST','TEST','/media',1)",[]).unwrap();
        conn.execute("INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) VALUES('file-你好世界.txt','TEST','你好世界.txt','你好世界.txt','FILE','2026-01-01T00:00:00Z')",[]).unwrap();
        let count=|text:&str|conn.query_row("SELECT count(*) FROM file_names_fts WHERE file_names_fts MATCH ?1",[text],|row|row.get::<_,i64>(0)).unwrap();
        assert_eq!(count("你好世"),1);
        { let tx=conn.unchecked_transaction().unwrap(); tx.execute("UPDATE files SET file_name='replacement.txt' WHERE id='file-你好世界.txt'",[]).unwrap(); }
        assert_eq!(count("你好世"),1);
        conn.execute("UPDATE files SET file_name='replacement.txt' WHERE id='file-你好世界.txt'",[]).unwrap();
        assert_eq!(count("你好世"),0);
        assert_eq!(count("replacement"),1);
        conn.execute("DELETE FROM files WHERE id='file-你好世界.txt'",[]).unwrap();
        assert_eq!(count("replacement"),0);
    });
}

#[test]
fn literal_name_search_handles_short_chinese_symbols_and_fts_syntax() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    db.with_conn(|conn| {
        conn.execute("INSERT INTO volumes(id,identity,path,is_available) VALUES('TEST','TEST','/catalog',1)",[]).unwrap();
        for name in ["你好世界.txt","road-AND-test.txt","100%_literal.txt","a\"b-file.txt","prefix.Interior.suffix"] {
            conn.execute("INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) VALUES('file-'||?1,'TEST',?1,?1,'FILE','2026-01-01T00:00:00Z')",[name]).unwrap();
        }
        for (text,total) in [("你好",1),("好世界",1),("AND",1),("%_",1),("%_literal",1),("a\"b",1),("Interior",1),("does-not-exist",0)] {
            let (filter,args)=name_filter(text);
            assert_eq!(filter.contains("MATCH"),text.chars().count()>=3);
            let count=conn.query_row(&format!("SELECT count(*) FROM file_catalog WHERE {filter}"),rusqlite::params_from_iter(args),|row|row.get::<_,i64>(0)).unwrap();
            assert_eq!(count,total,"{text}");
        }
    });
}

#[test]
fn remount_changes_only_the_volume_root_and_preserves_file_id_and_relative_path() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(Path::new(":memory:")).unwrap();
    let old = directory.path().join("old");
    let new = directory.path().join("new");
    let make = |root: &Path| Location {
        volume: Volume {
            identity: "TEST".into(),
            path: root.into(),
        },
        path: root.join("folder/file.txt"),
    };
    let before = db.with_conn(|conn| {
        let tx = conn.unchecked_transaction().unwrap();
        let id = upsert(
            &tx,
            &make(&old),
            Some(FileKind::File),
            Some("inode:birth"),
            Some(10),
            None,
            "2026-01-01T00:00:00Z",
            None,
        )
        .unwrap();
        tx.execute("UPDATE files SET favorited=1 WHERE id=?1", [&id])
            .unwrap();
        tx.commit().unwrap();
        id
    });
    let after = db.with_conn(|conn| {
        let tx = conn.unchecked_transaction().unwrap();
        let id = upsert(
            &tx,
            &make(&new),
            Some(FileKind::File),
            Some("inode:birth"),
            Some(10),
            None,
            "2026-01-01T00:00:00Z",
            None,
        )
        .unwrap();
        tx.commit().unwrap();
        id
    });
    assert_eq!(before, after);
    let row = db.file(&after).unwrap().unwrap();
    assert_eq!(row.path, "folder/file.txt");
    assert!(row.favorited);
    assert_eq!(db.volumes().unwrap()[0].path, new.to_str().unwrap());
}

#[test]
fn missing_files_keep_business_state_and_reappear_with_the_same_strong_identity() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("report.txt");
    std::fs::write(&path, b"data").unwrap();
    let db = Db::open(Path::new(":memory:")).unwrap();
    let first = db
        .record_file_paths(&[path.to_str().unwrap().into()], None)
        .unwrap()
        .remove(0);
    db.with_conn(|conn| conn.execute("UPDATE files SET favorited=1 WHERE id=?1", [&first.id]))
        .unwrap();
    let absolute = locate(path.to_str().unwrap())
        .unwrap()
        .path
        .to_str()
        .unwrap()
        .to_owned();
    db.mark_file_paths_missing(&[absolute]).unwrap();
    let missing = db.file(&first.id).unwrap().unwrap();
    assert!(missing.missing && missing.favorited);
    let returned = db
        .record_file_paths(&[path.to_str().unwrap().into()], None)
        .unwrap()
        .remove(0);
    assert_eq!(returned.id, first.id);
    assert!(!returned.missing && returned.favorited);
}

#[test]
fn interrupted_rename_finishes_in_sql_on_reopen_without_another_filesystem_move() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db_path = directory.path().join("plain.db");
    let path = directory.path().join("before.txt");
    let destination = directory.path().join("after.txt");
    std::fs::write(&path, b"data").unwrap();
    let id = {
        let db = Db::open(&db_path).unwrap();
        let row = db
            .record_file_paths(&[path.to_str().unwrap().into()], None)
            .unwrap()
            .remove(0);
        let source = locate(path.to_str().unwrap()).unwrap().path;
        let target = locate(destination.to_str().unwrap()).unwrap().path;
        db.prepare_file_path_change(
            &row.id,
            source.to_str().unwrap(),
            target.to_str().unwrap(),
            PathChange::Move,
        )
        .unwrap();
        std::fs::rename(&path, &destination).unwrap();
        row.id
    };
    let db = Db::open(&db_path).unwrap();
    assert!(db.file(&id).unwrap().unwrap().path.ends_with("after.txt"));
    assert_eq!(db.key_values().iter().count(), 0);
    assert_eq!(std::fs::read(destination).unwrap(), b"data");
}

#[test]
fn interrupted_rename_does_not_attach_user_state_to_a_replacement_at_the_destination() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db_path = directory.path().join("plain.db");
    let source = directory.path().join("source.txt");
    let target = directory.path().join("target.txt");
    let retained = directory.path().join("retained.txt");
    std::fs::write(&source, b"original").unwrap();
    let id = {
        let db = Db::open(&db_path).unwrap();
        let row = db
            .record_file_paths(&[source.to_str().unwrap().into()], None)
            .unwrap()
            .remove(0);
        let from = locate(source.to_str().unwrap()).unwrap().path;
        let to = locate(target.to_str().unwrap()).unwrap().path;
        db.prepare_file_path_change(
            &row.id,
            from.to_str().unwrap(),
            to.to_str().unwrap(),
            PathChange::Move,
        )
        .unwrap();
        std::fs::rename(&source, &target).unwrap();
        std::fs::rename(&target, &retained).unwrap();
        std::fs::write(&target, b"replacement").unwrap();
        row.id
    };
    assert!(Db::open(&db_path).is_err());
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    let path: String = connection
        .query_row("SELECT path FROM files WHERE id=?1", [&id], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(path.ends_with("source.txt"));
    assert_eq!(std::fs::read(target).unwrap(), b"replacement");
    assert_eq!(std::fs::read(retained).unwrap(), b"original");
}

#[test]
fn visibility_batch_uses_path_indexes_instead_of_scanning_unrelated_files() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let db = Db::open(Path::new(":memory:")).unwrap();
    db.with_conn(|c| {
        c.execute("INSERT INTO volumes(id,identity,path,is_available) VALUES('v','v','/media',1)", []).unwrap();
        c.execute_batch("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000) INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) SELECT 'f'||x,'v','images/'||x||'.jpg',x||'.jpg','IMAGE','2026-01-01' FROM n;").unwrap();
    });
    let scope = crate::features::files::index_scope::Scope {
        volumes: Vec::new(),
        directories: vec!["/media".into()],
        exclusions: vec!["/media/images/1.jpg".into()],
        defaults: crate::features::files::index_scope::Defaults {
            excluded_directories: vec![],
            excluded_names: vec![],
        },
    };
    let steps = Arc::new(AtomicUsize::new(0));
    let observed = steps.clone();
    db.with_conn(|c| {
        c.progress_handler(
            100,
            Some(move || {
                observed.fetch_add(100, Ordering::Relaxed);
                false
            }),
        )
    });
    let paths = (1..=32)
        .map(|n| format!("/media/images/{n}.jpg"))
        .collect::<Vec<_>>();
    db.update_index_visibility(&scope, Some(&paths)).unwrap();
    db.with_conn(|c| c.progress_handler(0, None::<fn() -> bool>));
    assert!(
        steps.load(Ordering::Relaxed) < 20000,
        "VM steps: {}",
        steps.load(Ordering::Relaxed)
    );
    assert_eq!(
        db.with_conn(
            |c| c.query_row("SELECT indexed FROM files WHERE id='f1'", [], |row| row
                .get::<_, bool>(0))
        )
        .unwrap(),
        false
    );
    assert_eq!(
        db.with_conn(
            |c| c.query_row("SELECT COUNT(*) FROM files WHERE indexed=0", [], |row| row
                .get::<_, i64>(
                0
            ))
        )
        .unwrap(),
        1
    );
}

#[test]
fn visibility_subtrees_preserve_missing_trash_boundaries_and_nested_volumes() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    db.with_conn(|c| c.execute_batch("INSERT INTO volumes(id,identity,path,is_available) VALUES('v','v','/media',1),('nested','nested','/media/a/mount',0);
        INSERT INTO files(id,volume_id,path,file_name,kind,updated_at,missing,favorited,trashed_at,original_path) VALUES
        ('a','v','a','a','DIRECTORY','2026',0,1,NULL,NULL),
        ('child','v','a/你好%_.jpg','你好%_.jpg','IMAGE','2026',0,1,NULL,NULL),
        ('missing','v','a/old.jpg','old.jpg','IMAGE','2026',1,1,NULL,NULL),
        ('trash','v','a/trash.jpg','trash.jpg','IMAGE','2026',0,1,'2026','outside.jpg'),
        ('neighbor','v','ab/untouched.jpg','untouched.jpg','IMAGE','2026',0,1,NULL,NULL),
        ('nested','nested','child.jpg','child.jpg','IMAGE','2026',0,1,NULL,NULL);")).unwrap();
    let scope = crate::features::files::index_scope::Scope {
        volumes: Vec::new(),
        directories: vec!["/media".into()],
        exclusions: vec!["/media/a".into()],
        defaults: crate::features::files::index_scope::Defaults {
            excluded_directories: vec![],
            excluded_names: vec![],
        },
    };
    db.update_index_visibility(
        &scope,
        Some(&[
            "/media/a".into(),
            "/media/a/你好%_.jpg".into(),
            "/unrelated".into(),
        ]),
    )
    .unwrap();
    let hidden = db
        .with_conn(|c| {
            c.prepare("SELECT id FROM files WHERE indexed=0 ORDER BY id")?
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap();
    assert_eq!(hidden, ["a", "child", "missing", "nested"]);
    assert_eq!(
        db.with_conn(
            |c| c.query_row("SELECT COUNT(*) FROM files WHERE favorited=1", [], |r| r
                .get::<_, i64>(0))
        )
        .unwrap(),
        6
    );
    db.with_conn(|c| c.execute("UPDATE files SET indexed=1", []))
        .unwrap();
    db.update_index_visibility(&scope, Some(&["/".into()]))
        .unwrap();
    assert_eq!(
        db.with_conn(
            |c| c.query_row("SELECT COUNT(*) FROM files WHERE indexed=0", [], |r| r
                .get::<_, i64>(0))
        )
        .unwrap(),
        4
    );
}

#[cfg(feature = "chat")]
#[test]
fn scan_paths_commit_one_transaction_and_roll_back_a_failed_batch() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let paths = (0..300)
        .map(|index| {
            let path = directory.path().join(format!("{index}.rs"));
            std::fs::write(&path, b"source").unwrap();
            (path.to_str().unwrap().to_owned(), FileKind::File)
        })
        .collect::<Vec<_>>();
    let db = Db::open(Path::new(":memory:")).unwrap();
    let commits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    db.with_conn(|conn| {
        let commits = commits.clone();
        conn.commit_hook(Some(move || {
            commits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            false
        }))?;
        Ok::<_, rusqlite::Error>(())
    })
    .unwrap();
    db.record_scan_paths(&paths, "first").unwrap();
    assert_eq!(commits.load(std::sync::atomic::Ordering::SeqCst), 1);
    db.with_conn(|conn| {
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM files WHERE last_seen_scan_id='first'", [], |row| row.get::<_,i64>(0))?, 300);
        conn.execute_batch("CREATE TRIGGER reject_scan_record BEFORE UPDATE ON files WHEN NEW.file_name='299.rs' BEGIN SELECT RAISE(ABORT,'rejected'); END;")
    }).unwrap();
    let before = commits.load(std::sync::atomic::Ordering::SeqCst);
    assert!(db.record_scan_paths(&paths, "failed").is_err());
    assert_eq!(commits.load(std::sync::atomic::Ordering::SeqCst), before);
    db.with_conn(|conn| {
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM files WHERE last_seen_scan_id='failed'",
                [],
                |row| row.get::<_, i64>(0)
            )?,
            0
        );
        Ok::<_, rusqlite::Error>(())
    })
    .unwrap();
}

#[test]
fn volume_index_defaults_and_configuration_survive_rebinding_and_disconnect() {
    use crate::infrastructure::db::IndexMode;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&directory.path().join("index.db")).unwrap();
    let id = db.register_volume(directory.path()).unwrap();
    let volume = db.volumes().unwrap().remove(0);
    assert_eq!(volume.index_mode, IndexMode::Unconfigured);
    assert!(volume.included_dirs.is_empty());
    assert!(volume.excluded_dirs.is_empty());
    db.set_volume_index(
        &id,
        IndexMode::Allowlist,
        &["Photos".into()],
        &["Cache".into()],
    )
    .unwrap();
    assert_eq!(db.register_volume(directory.path()).unwrap(), id);
    db.with_conn(|conn| conn.execute("UPDATE volumes SET is_available=0 WHERE id=?1", [&id]))
        .unwrap();
    let offline = db.volumes().unwrap().remove(0);
    assert!(!offline.is_available);
    assert_eq!(offline.index_mode, IndexMode::Allowlist);
    assert_eq!(offline.included_dirs, vec!["Photos"]);
    assert_eq!(offline.excluded_dirs, vec!["Cache"]);
    assert_eq!(db.register_volume(directory.path()).unwrap(), id);
    let online = db.volumes().unwrap().remove(0);
    assert!(online.is_available);
    assert_eq!(online.included_dirs, offline.included_dirs);
}
