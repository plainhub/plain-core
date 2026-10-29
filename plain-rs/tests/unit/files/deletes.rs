use super::*;
#[test]
fn deleted_receipts_cascade_atomically_and_preserve_failed_and_foreign_records() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.with_conn(|c|c.execute_batch("INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('video','old',2,'first'),('foreign','old',6,'first'),('file','/root/deleted',22,'first'),('failed','/root/failed',22,'first'),('sibling','/root_sibling/file',22,'first'); INSERT INTO media_play_history(media_type,file_id,position_ms,played_at,play_count) VALUES(2,'old',4000000001,'first',0),(2,'keep',4000000002,'first',0); INSERT INTO platform_audio_media(file_id,path,title,artist,duration_ms,album_id) VALUES('test:/root/deleted','/root/deleted','gone','',1,''),('test:/root/failed','/root/failed','keep','',1,''),('test:/root_sibling/file','/root_sibling/file','keep','',1,''); INSERT INTO audio_queue_items(file_id,sort_order) VALUES('test:/root/deleted',0),('test:/root/failed',1),('test:/root_sibling/file',2);")).unwrap();
    db.with_conn(|c|c.execute_batch("INSERT INTO image_embeddings(file_id,path,embedding,created_at,updated_at) VALUES('private','/root/deleted',x'3f800000','first','first'),('pending','/root/failed',x'3f800000','first','first'),('sibling','/root_sibling/file',x'3f800000','first','first');")).unwrap();
    cleanup(
        &db,
        &[Item {
            media_type: 2,
            id: "old".into(),
            path: "/root/deleted".into(),
        }],
        &["/root/deleted".into()],
        &[],
    )
    .unwrap();
    db.with_conn(|c| {
        assert_eq!(
            c.query_row("SELECT count(*) FROM media_play_history", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM tag_relations", [], |r| r
                .get::<_, i64>(0))?,
            3
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM image_embeddings", [], |r| r
                .get::<_, i64>(0))?,
            2
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM audio_queue_items", [], |r| r
                .get::<_, i64>(0))?,
            2
        );
        Ok::<_, rusqlite::Error>(())
    })
    .unwrap();
    cleanup(&db, &[], &["/root".into()], &["/root".into()]).unwrap();
    assert_eq!(
        db.with_conn(
            |c| c.query_row("SELECT count(*) FROM image_embeddings", [], |r| r
                .get::<_, i64>(0))
        )
        .unwrap(),
        1
    );
    db.with_conn(|c| {
        assert_eq!(
            c.query_row("SELECT count(*) FROM audio_queue_items", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM tag_relations", [], |r| r
                .get::<_, i64>(0))?,
            2
        );
        Ok::<_, rusqlite::Error>(())
    })
    .unwrap();
}

fn catalog_file(data_type: DataType, path: &str) -> crate::db::MediaFileMetadata {
    crate::db::MediaFileMetadata {
        album: String::new(),
        album_artist: String::new(),

        data_type,
        path: path.into(),
        title: "Title".into(),
        artist: String::new(),
        bucket_id: "/root".into(),
        size_bytes: 10,
        duration_ms: 0,
        cover_image_id: None,
        width: 0,
        height: 0,
        rotation: 0,
        taken_at: None,
        created_at: "2026-10-13T00:00:00.000Z".into(),
        updated_at: "2026-10-13T00:00:00.000Z".into(),
    }
}

#[test]
fn deletion_uses_current_catalog_identities_and_preserves_partial_failures_and_foreign_types() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let created = db
        .record_media_files(&[
            catalog_file(DataType::Video, "/root/deleted.mp4"),
            catalog_file(DataType::Image, "/root/failed.jpg"),
            catalog_file(DataType::Image, "/root_sibling/keep.jpg"),
        ])
        .unwrap();
    let video = &created[0].id;
    db.with_conn(|c| -> rusqlite::Result<()> {
        c.execute("INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('media',?1,2,'first'),('foreign',?1,6,'first')",[video])?;
        c.execute("INSERT INTO media_play_history(media_type,file_id,position_ms,played_at,play_count) VALUES(2,?1,123,'first',0)",[video])?;
        Ok(())
    }).unwrap();
    cleanup(&db, &[], &["/root/deleted.mp4".into()], &[]).unwrap();
    assert!(
        db.media_file_by_path(DataType::Video, "/root/deleted.mp4")
            .unwrap()
            .is_none()
    );
    assert!(
        db.media_file_by_path(DataType::Image, "/root/failed.jpg")
            .unwrap()
            .is_some()
    );
    assert!(
        db.media_file_by_path(DataType::Image, "/root_sibling/keep.jpg")
            .unwrap()
            .is_some()
    );
    db.with_conn(|c| -> rusqlite::Result<()> {
        assert_eq!(
            c.query_row(
                "SELECT count(*) FROM tag_relations WHERE target_key=?1 AND type=2",
                [video],
                |r| r.get::<_, i64>(0)
            )?,
            0
        );
        assert_eq!(
            c.query_row(
                "SELECT count(*) FROM tag_relations WHERE target_key=?1 AND type=6",
                [video],
                |r| r.get::<_, i64>(0)
            )?,
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM media_play_history", [], |r| r
                .get::<_, i64>(0))?,
            0
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn confirmed_directory_removal_cleans_descendants_using_literal_boundaries() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.record_media_files(&[
        catalog_file(DataType::Image, "/root_%/child.jpg"),
        catalog_file(DataType::Image, "/root_%/nested/child.jpg"),
        catalog_file(DataType::Image, "/root_%_sibling/keep.jpg"),
        catalog_file(DataType::Image, "/rootAB/keep.jpg"),
    ])
    .unwrap();
    cleanup(&db, &[], &["/root_%".into()], &["/root_%".into()]).unwrap();
    assert_eq!(db.media_file_count(DataType::Image).unwrap(), 2);
    assert!(
        db.media_file_by_path(DataType::Image, "/root_%/child.jpg")
            .unwrap()
            .is_none()
    );
    assert!(
        db.media_file_by_path(DataType::Image, "/root_%_sibling/keep.jpg")
            .unwrap()
            .is_some()
    );
    assert!(
        db.media_file_by_path(DataType::Image, "/rootAB/keep.jpg")
            .unwrap()
            .is_some()
    );
}

#[test]
fn catalog_deletion_failure_rolls_back_all_associated_state_cleanup() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let created = db
        .record_media_files(&[catalog_file(DataType::Video, "/root/deleted.mp4")])
        .unwrap()
        .remove(0);
    db.with_conn(|c| -> rusqlite::Result<()> {
        c.execute("INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('media',?1,2,'first')",[&created.id])?;
        c.execute("INSERT INTO media_play_history(media_type,file_id,position_ms,played_at,play_count) VALUES(2,?1,123,'first',0)",[&created.id])?;
        c.execute_batch("CREATE TRIGGER reject_catalog_delete BEFORE DELETE ON files BEGIN SELECT RAISE(ABORT,'catalog delete rejected'); END;")?;
        Ok(())
    }).unwrap();
    assert!(cleanup(&db, &[], &[created.path.clone()], &[]).is_err());
    assert_eq!(db.media_file_count(DataType::Video).unwrap(), 1);
    db.with_conn(|c| -> rusqlite::Result<()> {
        assert_eq!(
            c.query_row("SELECT count(*) FROM tag_relations", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM media_play_history", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        Ok(())
    })
    .unwrap();
}

#[cfg(windows)]
#[test]
fn directory_prefix_uses_the_same_separator_as_native_canonical_paths() {
    assert_eq!(directory_prefix(r"C:\root"), r"C:\root\");
    assert_eq!(directory_prefix("C:/root"), "C:/root/");
}

#[test]
fn missing_paths_keep_user_state_until_confirmed_permanent_deletion() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.record_media_files(&[
        catalog_file(DataType::Video, "/root/gone.mp4"),
        catalog_file(DataType::Video, "/root/kept.mp4"),
    ])
    .unwrap();
    let gone = db
        .media_file_by_path(DataType::Video, "/root/gone.mp4")
        .unwrap()
        .unwrap();
    db.with_conn(|c| -> rusqlite::Result<()> {
        c.execute(
            "INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('media',?1,2,'first')",
            [&gone.id],
        )?;
        c.execute(
            "INSERT INTO media_play_history(media_type,file_id,position_ms,played_at,play_count) VALUES(2,?1,123,'first',0)",
            [&gone.id],
        )?;
        c.execute(
            "INSERT INTO image_embeddings(file_id,path,embedding,created_at,updated_at) VALUES(?1,?2,x'3f800000','first','first')",
            params![&gone.id, &gone.path],
        )?;
        Ok(())
    })
    .unwrap();

    assert_eq!(db.forget_media_file("/root/gone.mp4").unwrap(), 1);
    assert!(
        db.media_file_by_path(DataType::Video, "/root/gone.mp4")
            .unwrap()
            .is_none()
    );
    assert!(
        db.media_file_by_path(DataType::Video, "/root/kept.mp4")
            .unwrap()
            .is_some(),
        "forgetting one path must not touch a sibling"
    );
    db.with_conn(|c| -> rusqlite::Result<()> {
        let tags: i64 = c.query_row(
            "SELECT COUNT(*) FROM tag_relations WHERE target_key=?1",
            [&gone.id],
            |row| row.get(0),
        )?;
        let progress: i64 = c.query_row(
            "SELECT COUNT(*) FROM media_play_history WHERE file_id=?1",
            [&gone.id],
            |row| row.get(0),
        )?;
        let embeddings: i64 = c.query_row(
            "SELECT COUNT(*) FROM image_embeddings WHERE file_id=?1",
            [&gone.id],
            |row| row.get(0),
        )?;
        assert_eq!((tags, progress, embeddings), (1, 1, 1));
        Ok(())
    })
    .unwrap();

    assert_eq!(
        db.forget_media_file("/root/absent.mp4").unwrap(),
        0,
        "a path that was never catalogued is not an error"
    );
}

#[test]
fn retired_identity_preserves_replacement_and_file_tags_at_the_same_path() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let old = db
        .record_media_files(&[catalog_file(DataType::Video, "/root/replaced.mp4")])
        .unwrap()
        .remove(0);
    let new = db
        .record_media_files(&[catalog_file(DataType::Image, "/root/replaced.mp4")])
        .unwrap()
        .remove(0);
    db.with_conn(|connection| {
        connection.execute("INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('old',?1,2,'first'),('replacement',?2,3,'first'),('file','/root/replaced.mp4',22,'first'),('neighbor','/root/neighbor',22,'first')", params![old.id,new.id])?;
        connection.execute("INSERT INTO media_play_history(file_id,media_type,position_ms,played_at,play_count) VALUES(?1,2,123,'first',0)", [&old.id])?;
        Ok::<_,rusqlite::Error>(())
    }).unwrap();
    retire_identity(
        &db,
        &Item {
            media_type: 2,
            id: old.id,
            path: old.path,
        },
    )
    .unwrap();
    assert!(
        db.media_file_by_path(DataType::Video, "/root/replaced.mp4")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        db.media_file_by_path(DataType::Image, "/root/replaced.mp4")
            .unwrap()
            .unwrap()
            .id,
        new.id
    );
    db.with_conn(|connection| {
        assert_eq!(
            connection.query_row("SELECT count(*) FROM tag_relations", [], |row| row
                .get::<_, i64>(0))?,
            3
        );
        assert_eq!(
            connection.query_row("SELECT count(*) FROM media_play_history", [], |row| row
                .get::<_, i64>(0))?,
            0
        );
        Ok::<_, rusqlite::Error>(())
    })
    .unwrap();
}
