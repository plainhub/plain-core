use super::*;

fn db() -> Db {
    Db::open(std::path::Path::new(":memory:")).unwrap()
}

#[test]
fn edits_use_only_typed_keys_and_preserve_existing_relation_dates() {
    let db = db();
    let audio = crate::features::tags::create_tag(&db, DataType::Audio, "Audio").unwrap();
    let image = crate::features::tags::create_tag(&db, DataType::Image, "Image").unwrap();
    let key = "/a, 'b' 文件".to_string();
    update_relations(&db, DataType::Audio, &[key.clone()], &[audio.id.clone()], &[]).unwrap();
    update_relations(&db, DataType::Image, &[key.clone()], &[image.id.clone()], &[]).unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE tag_relations SET created_at='2026-01-01T00:00:00Z' WHERE tag_id=?1",
            [&audio.id],
        )
    })
    .unwrap();
    update_relations(
        &db,
        DataType::Audio,
        &[key.clone(), key.clone()],
        &[audio.id.clone(), audio.id.clone()],
        &[],
    )
    .unwrap();
    db.with_conn(|conn| {
        let rows = conn
            .prepare("SELECT created_at FROM tag_relations WHERE tag_id=?1")
            .unwrap()
            .query_map([&audio.id], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(rows, ["2026-01-01T00:00:00Z"]);
    });
    update_relations(&db, DataType::Audio, &[key.clone()], &[], &[audio.id.clone()]).unwrap();
    assert!(
        crate::features::tags::keys_for_tag(&db, &audio.id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        crate::features::tags::keys_for_tag(&db, &image.id).unwrap(),
        [key]
    );
}

#[test]
fn invalid_batch_has_no_partial_additions_or_removals() {
    let db = db();
    let first = crate::features::tags::create_tag(&db, DataType::Audio, "First").unwrap();
    let second = crate::features::tags::create_tag(&db, DataType::Audio, "Second").unwrap();
    let other = crate::features::tags::create_tag(&db, DataType::Image, "Other").unwrap();
    update_relations(&db, DataType::Audio, &["kept".into()], &[first.id.clone()], &[]).unwrap();
    for ids in [
        vec![second.id.clone(), other.id],
        vec![second.id.clone(), "missing".into()],
    ] {
        assert!(
            update_relations(
                &db,
                DataType::Audio,
                &["kept".into(), "new".into()],
                &ids,
                &[first.id.clone()]
            )
            .is_err()
        );
        assert_eq!(
            crate::features::tags::keys_for_tag(&db, &first.id).unwrap(),
            ["kept"]
        );
        assert!(
            crate::features::tags::keys_for_tag(&db, &second.id)
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        update_relations(
            &db,
            DataType::Audio,
            &["valid".into(), " ".into()],
            &[second.id.clone()],
            &[]
        )
        .is_err()
    );
    assert!(
        crate::features::tags::keys_for_tag(&db, &second.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn database_failure_rolls_back_the_whole_batch() {
    let db = db();
    let tag = crate::features::tags::create_tag(&db, DataType::Audio, "Tag").unwrap();
    db.with_conn(|conn| conn.execute_batch("CREATE TRIGGER reject_relation BEFORE INSERT ON tag_relations WHEN NEW.target_key='fail' BEGIN SELECT RAISE(ABORT,'fixture failure'); END")).unwrap();
    assert!(
        update_relations(
            &db,
            DataType::Audio,
            &["first".into(), "fail".into()],
            &[tag.id.clone()],
            &[]
        )
        .is_err()
    );
    assert!(
        crate::features::tags::keys_for_tag(&db, &tag.id)
            .unwrap()
            .is_empty()
    );
}
