use crate::features::{
    enums::DataType,
    files::{deletes, moves},
    tags,
};
use crate::infrastructure::db::{Db, notes_feeds::FeedEntryRow};
use std::time::Instant;

#[test]
#[ignore = "measures RSS and tag database operations"]
fn rss_and_tag_operations() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("fixture.db")).unwrap();
    let now = "2026-01-01T00:00:00.000Z";
    let entries = (0..4000)
        .map(|i| FeedEntryRow {
            id: format!("entry-{i:04}"),
            feed_id: format!("feed-{}", i % 20),
            title: format!("Article {i}"),
            url: format!("https://example.test/{i}"),
            author: String::new(),
            description: String::new(),
            content: String::new(),
            image: String::new(),
            source_entry_id: format!("source-{i}"),
            published_at: format!("2026-01-{:02}T00:00:00.000Z", i % 28 + 1),
            is_read: false,
            created_at: now.into(),
            updated_at: now.into(),
        })
        .collect::<Vec<_>>();
    let measure = |name, operation: &mut dyn FnMut()| {
        let start = Instant::now();
        operation();
        println!("{name}={}", start.elapsed().as_micros());
    };

    measure("rss_insert", &mut || {
        assert_eq!(db.feed_entries_insert(&entries).unwrap().len(), 4000)
    });
    measure("rss_repeat", &mut || {
        assert!(db.feed_entries_insert(&entries).unwrap().is_empty())
    });
    measure("rss_page", &mut || {
        for i in 0..100 {
            let page = db
                .feed_entries_list(&format!("feed_id:feed-{}", i % 20), 50, 0)
                .unwrap();
            assert_eq!(page.len(), 50);
            assert!(
                page.windows(2)
                    .all(|w| (&w[0].published_at, &w[0].id) > (&w[1].published_at, &w[1].id))
            );
        }
    });

    let tag = tags::create_tag(&db, DataType::File, "Files").unwrap();
    let keys = (0..2000)
        .map(|i| format!("/fixture/{}/{i}", i % 10))
        .collect::<Vec<_>>();
    measure("tag_insert", &mut || {
        tags::update_relations(
            &db,
            DataType::File,
            &keys,
            std::slice::from_ref(&tag.id),
            &[],
        )
        .unwrap()
    });
    measure("tag_lookup", &mut || {
        for chunk in keys.chunks(50) {
            let relations = tags::relations_for_keys_of_kind(&db, chunk, DataType::File).unwrap();
            assert!(relations.iter().map(|r| &r.target_key).eq(chunk.iter()));
        }
    });
    measure("tag_move", &mut || {
        moves::rebind(&db, &[], "/fixture/0", "/moved").unwrap();
    });
    assert_eq!(tags::relations_for_key(&db, "/moved/0").unwrap().len(), 1);
    measure("tag_delete", &mut || {
        deletes::cleanup(&db, &[], &[], &["/moved".into()]).unwrap();
    });
    assert_eq!(tags::tag_by_id(&db, &tag.id).unwrap().unwrap().count, 1800);
}
