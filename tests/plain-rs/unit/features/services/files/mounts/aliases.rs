use super::*;

#[test]
fn aliases_persist_in_the_volume_row_and_a_fresh_database_has_no_alias() {
    let fixture = plain_libs::temporary::tempdir().unwrap();
    let source = fixture.path().join("generated-volume");
    std::fs::create_dir(&source).unwrap();
    let facts = plain_libs::serde_json::json!([{"id":"fixture-mount","path":source,"mountPoint":source,"alias":"Host alias"}]);
    let path = fixture.path().join("plain.db");
    let db = Db::open(&path).unwrap();
    assert_eq!(apply_aliases(&db, facts.clone()).unwrap()[0]["alias"], "");
    assert!(db.volumes().unwrap().is_empty());
    set_alias(&db, &facts, "fixture-mount", " Archive ").unwrap();
    assert_eq!(
        apply_aliases(&db, facts.clone()).unwrap()[0]["alias"],
        "Archive"
    );
    let row = db.volumes().unwrap().remove(0);
    assert_eq!(row.alias, "Archive");
    drop(db);
    let db = Db::open(&path).unwrap();
    assert_eq!(
        apply_aliases(&db, facts.clone()).unwrap()[0]["alias"],
        "Archive"
    );
    assert_eq!(db.volumes().unwrap()[0].id, row.id);
    set_alias(&db, &facts, "fixture-mount", "  ").unwrap();
    assert_eq!(apply_aliases(&db, facts.clone()).unwrap()[0]["alias"], "");
    assert!(set_alias(&db, &facts, "missing", "Rejected").is_err());
    let fresh = Db::open(&fixture.path().join("fresh.db")).unwrap();
    assert_eq!(apply_aliases(&fresh, facts).unwrap()[0]["alias"], "");
}

#[test]
fn mount_ids_stay_stable_and_unmounted_partitions_do_not_acquire_aliases() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    let facts = plain_libs::serde_json::json!([{"id":"part:fixture","path":"/dev/generated","mountPoint":null,"alias":"Host alias"}]);
    let result = apply_aliases(&db, facts.clone()).unwrap();
    assert_eq!(result[0]["id"], "part:fixture");
    assert_eq!(result[0]["alias"], "");
    assert!(db.volumes().unwrap().is_empty());
    assert!(set_alias(&db, &facts, "part:fixture", "Rejected").is_err());
    assert_eq!(
        mount_id(&plain_libs::serde_json::json!({"path":"/fixture"})),
        "path:/fixture"
    );
    assert_eq!(
        mount_id(&plain_libs::serde_json::json!({"id":"fsuuid:fixture","path":"/fixture"})),
        "fsuuid:fixture"
    );
}
