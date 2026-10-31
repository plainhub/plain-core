use super::*;

#[test]
fn file_paths_are_relative_and_preserve_user_state() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("你好.txt");
    std::fs::write(&path, b"hello").unwrap();
    let db = Db::open(Path::new(":memory:")).unwrap();
    let first = db
        .record_file_paths(&[path.to_str().unwrap().into()], None)
        .unwrap()
        .remove(0);
    assert!(!Path::new(&first.path).is_absolute());
    db.with_conn(|conn| conn.execute("UPDATE files SET favorited=1 WHERE id=?1", [first.id]))
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
        conn.execute("INSERT INTO volumes(identity,path,is_available) VALUES('TEST','/media',1)",[]).unwrap();
        conn.execute("INSERT INTO files(volume_id,path,file_name,kind,updated_at) VALUES(1,'你好世界.txt','你好世界.txt','FILE','2026-01-01T00:00:00Z')",[]).unwrap();
        let count=|text:&str|conn.query_row("SELECT count(*) FROM file_names_fts WHERE file_names_fts MATCH ?1",[text],|row|row.get::<_,i64>(0)).unwrap();
        assert_eq!(count("你好世"),1);
        { let tx=conn.unchecked_transaction().unwrap(); tx.execute("UPDATE files SET file_name='replacement.txt' WHERE id=1",[]).unwrap(); }
        assert_eq!(count("你好世"),1);
        conn.execute("UPDATE files SET file_name='replacement.txt' WHERE id=1",[]).unwrap();
        assert_eq!(count("你好世"),0);
        assert_eq!(count("replacement"),1);
        conn.execute("DELETE FROM files WHERE id=1",[]).unwrap();
        assert_eq!(count("replacement"),0);
    });
}

#[test]
fn literal_name_search_handles_short_chinese_symbols_and_fts_syntax() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    db.with_conn(|conn| {
        conn.execute("INSERT INTO volumes(identity,path,is_available) VALUES('TEST','/catalog',1)",[]).unwrap();
        for name in ["你好世界.txt","road-AND-test.txt","100%_literal.txt","a\"b-file.txt","prefix.Interior.suffix"] {
            conn.execute("INSERT INTO files(volume_id,path,file_name,kind,updated_at) VALUES(1,?1,?1,'FILE','2026-01-01T00:00:00Z')",[name]).unwrap();
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
    let directory = tempfile::tempdir().unwrap();
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
        )
        .unwrap();
        tx.execute("UPDATE files SET favorited=1 WHERE id=?1", [id])
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
        )
        .unwrap();
        tx.commit().unwrap();
        id
    });
    assert_eq!(before, after);
    let row = db.file(after).unwrap().unwrap();
    assert_eq!(row.path, "folder/file.txt");
    assert!(row.favorited);
    assert_eq!(db.volumes().unwrap()[0].path, new.to_str().unwrap());
}

#[test]
fn missing_files_keep_business_state_and_reappear_with_the_same_strong_identity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.txt");
    std::fs::write(&path, b"data").unwrap();
    let db = Db::open(Path::new(":memory:")).unwrap();
    let first = db
        .record_file_paths(&[path.to_str().unwrap().into()], None)
        .unwrap()
        .remove(0);
    db.with_conn(|conn| conn.execute("UPDATE files SET favorited=1 WHERE id=?1", [first.id]))
        .unwrap();
    let absolute = locate(path.to_str().unwrap())
        .unwrap()
        .path
        .to_str()
        .unwrap()
        .to_owned();
    db.mark_file_paths_missing(&[absolute]).unwrap();
    let missing = db.file(first.id).unwrap().unwrap();
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
    let directory = tempfile::tempdir().unwrap();
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
            row.id,
            source.to_str().unwrap(),
            target.to_str().unwrap(),
            PathChange::Move,
        )
        .unwrap();
        std::fs::rename(&path, &destination).unwrap();
        row.id
    };
    let db = Db::open(&db_path).unwrap();
    assert!(db.file(id).unwrap().unwrap().path.ends_with("after.txt"));
    assert_eq!(db.key_values().iter().count(), 0);
    assert_eq!(std::fs::read(destination).unwrap(), b"data");
}

#[test]
fn interrupted_rename_does_not_attach_user_state_to_a_replacement_at_the_destination() {
    let directory = tempfile::tempdir().unwrap();
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
            row.id,
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
        .query_row("SELECT path FROM files WHERE id=?1", [id], |row| row.get(0))
        .unwrap();
    assert!(path.ends_with("source.txt"));
    assert_eq!(std::fs::read(target).unwrap(), b"replacement");
    assert_eq!(std::fs::read(retained).unwrap(), b"original");
}
