use super::*;
use std::sync::atomic::Ordering;

fn fixture() -> (plain_libs::temporary::TempDir, Arc<TagsLoader>) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    (dir, new(db))
}

fn key(kind: DataType, id: &str) -> TagKey {
    TagKey {
        kind: kind.kind(),
        id: id.to_owned(),
    }
}

#[tokio::test(start_paused = true)]
async fn batches_duplicate_keys_and_separates_kinds() {
    let (_dir, loader) = fixture();
    for (kind, name) in [(DataType::Note, "note"), (DataType::FeedEntry, "entry")] {
        let tag = plain_rs::features::tags::create_tag(&loader.db, kind.kind(), name).unwrap();
        plain_rs::features::tags::add_relations(&loader.db, &[(tag.id, "shared".into())]).unwrap();
    }
    let (note, duplicate, entry, missing) = tokio::join!(
        loader.load_one(key(DataType::Note, "shared")),
        loader.load_one(key(DataType::Note, "shared")),
        loader.load_one(key(DataType::FeedEntry, "shared")),
        loader.load_one(key(DataType::Note, "missing")),
    );
    assert_eq!(note.unwrap()[0].name, "note");
    assert_eq!(duplicate.unwrap()[0].name, "note");
    assert_eq!(entry.unwrap()[0].name, "entry");
    assert!(missing.unwrap().is_empty());
    assert_eq!(loader.loads.load(Ordering::Relaxed), 1);
    assert!(loader.pending.lock().unwrap().is_empty());
}

#[tokio::test(start_paused = true)]
async fn canceling_the_first_waiter_does_not_cancel_the_batch() {
    let (_dir, loader) = fixture();
    let mut canceled = Box::pin(loader.load_one(key(DataType::Note, "canceled")));
    assert!(futures_util::poll!(&mut canceled).is_pending());
    drop(canceled);
    assert!(
        loader
            .load_one(key(DataType::Note, "live"))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(loader.loads.load(Ordering::Relaxed), 1);
    assert!(loader.pending.lock().unwrap().is_empty());
    assert!(
        loader
            .load_one(key(DataType::Note, "next"))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(loader.loads.load(Ordering::Relaxed), 2);
}

#[tokio::test(start_paused = true)]
async fn database_failure_reaches_every_waiter_and_later_batches() {
    let (_dir, loader) = fixture();
    loader
        .db
        .with_conn(|conn| conn.execute_batch("DROP TABLE tag_relations; DROP TABLE tags;"))
        .unwrap();
    let (first, second) = tokio::join!(
        loader.load_one(key(DataType::Note, "first")),
        loader.load_one(key(DataType::Note, "second")),
    );
    assert!(first.unwrap_err().message.contains("no such table"));
    assert!(second.unwrap_err().message.contains("no such table"));
    assert_eq!(loader.loads.load(Ordering::Relaxed), 1);
    assert!(loader.pending.lock().unwrap().is_empty());
    assert!(loader.load_one(key(DataType::Note, "later")).await.is_err());
    assert_eq!(loader.loads.load(Ordering::Relaxed), 2);
}
