//! Unit tests for `src/library/db/tag.rs` (row IO layer).

#[path = "../../../features/library/fixtures.rs"]
mod fixtures;
use fixtures::*;

fn tag(id: &str, kind: DataType, name: &str) -> crate::infrastructure::db::TagRow {
    crate::infrastructure::db::TagRow {
        id: id.into(),
        name: name.into(),
        kind,
        count: 0,
    }
}

#[test]
fn count_is_computed_live_on_every_read() {
    let db = test_db("tag_count_live");
    crate::infrastructure::db::tag::insert_tag(&db, &tag("t1", DataType::Audio, "music")).unwrap();
    let before = crate::infrastructure::db::tag::tag_by_id(&db, "t1")
        .unwrap()
        .unwrap();
    assert_eq!(before.count, 0);
    crate::infrastructure::db::tag::insert_relations(
        &db,
        &[("t1".into(), "k1".into()), ("t1".into(), "k2".into())],
    )
    .unwrap();
    let after = crate::infrastructure::db::tag::tag_by_id(&db, "t1")
        .unwrap()
        .unwrap();
    assert_eq!(after.count, 2);
    // There is no stored count column to drift — the subquery is the value.
    let raw: i64 = db.with_conn(|c| {
        c.query_row(
            "SELECT COUNT(*) FROM tag_relations WHERE tag_id='t1'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    });
    assert_eq!(raw, 2);
}

#[test]
fn remove_relations_cross_product() {
    let db = test_db("tag_cross");
    crate::infrastructure::db::tag::insert_tag(&db, &tag("t1", DataType::Audio, "a")).unwrap();
    crate::infrastructure::db::tag::insert_tag(&db, &tag("t2", DataType::Audio, "b")).unwrap();
    crate::infrastructure::db::tag::insert_relations(
        &db,
        &[
            ("t1".into(), "k1".into()),
            ("t1".into(), "k2".into()),
            ("t2".into(), "k1".into()),
        ],
    )
    .unwrap();
    // Remove only (t1 × k1): (t1,k2) and (t2,k1) survive.
    crate::infrastructure::db::tag::remove_relations(&db, &["k1".to_string()], &["t1".to_string()])
        .unwrap();
    let t1_keys = crate::infrastructure::db::tag::keys_for_tag(&db, "t1").unwrap();
    assert_eq!(t1_keys, vec!["k2".to_string()]);
    let t2_keys = crate::infrastructure::db::tag::keys_for_tag(&db, "t2").unwrap();
    assert_eq!(t2_keys, vec!["k1".to_string()]);
}

use crate::features::enums::DataType;

#[test]
fn tag_type_round_trips_all_enum_names_and_rejects_old_codes() {
    let db = test_db("tag_type_text");
    for kind in [
        DataType::Default,
        DataType::Audio,
        DataType::Video,
        DataType::Image,
        DataType::Sms,
        DataType::Contact,
        DataType::Note,
        DataType::FeedEntry,
        DataType::Call,
        DataType::Package,
        DataType::File,
        DataType::AppFile,
        DataType::Doc,
    ] {
        let row = crate::features::tags::create_tag(&db, kind, "example").unwrap();
        assert_eq!(
            crate::features::tags::tag_by_id(&db, &row.id).unwrap(),
            Some(row.clone())
        );
        let raw: String = db
            .with_conn(|conn| {
                conn.query_row("SELECT type FROM tags WHERE id=?", [&row.id], |row| {
                    row.get(0)
                })
            })
            .unwrap();
        assert_eq!(plain_libs::serde_json::to_value(kind).unwrap(), raw);
    }

    db.with_conn(|conn| {
        for invalid in ["1", "audio", "UNKNOWN"] {
            assert!(
                conn.execute(
                    "INSERT INTO tags VALUES('invalid','example',?,'now','now')",
                    [invalid],
                )
                .is_err()
            );
        }
    });
}

#[test]
fn typed_bulk_lookup_preserves_input_order_and_repeated_keys() {
    let db = test_db("tag_order");
    for (id, kind) in [("z", DataType::File), ("a", DataType::File), ("note", DataType::Note)] {
        crate::infrastructure::db::tag::insert_tag(&db, &tag(id, kind, id)).unwrap();
        crate::features::tags::add_relations(&db, &[(id.into(), "key2".into())]).unwrap();
    }
    crate::features::tags::add_relations(&db, &[("z".into(), "key1".into())]).unwrap();
    let keys = ["key2", "absent", "key1", "key2"].map(String::from);
    let rows = crate::features::tags::relations_for_keys_of_kind(&db, &keys, DataType::File).unwrap();
    let actual = rows.iter().map(|r|(r.target_key.as_str(),r.tag_id.as_str())).collect::<Vec<_>>();
    assert_eq!(actual, [("key2","a"),("key2","z"),("key1","z"),("key2","a"),("key2","z")]);
}
