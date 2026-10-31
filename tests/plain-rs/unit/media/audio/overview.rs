use super::*;

#[test]
fn statistics_cover_the_full_catalog_and_resolve_live_history_before_fallback() {
    let mut summary = Accumulator::new(vec![
        History {
            id: "missing".into(),
            play_count: 100,
        },
        History {
            id: "1005".into(),
            play_count: 7,
        },
        History {
            id: "2".into(),
            play_count: 3,
        },
        History {
            id: "1001".into(),
            play_count: 4,
        },
    ])
    .unwrap();
    for i in 0..1100 {
        let artist = match i {
            4 => "",
            6 => "<unknown>",
            _ if i % 2 == 0 => "A",
            _ => "B",
        };
        summary
            .push(&i.to_string(), artist, &format!("/{i}"), i, i)
            .unwrap();
    }
    let result = summary.finish();
    assert_eq!(result.recent_items, [1005, 2, 1001]);
    assert_eq!(
        result.artists,
        vec![
            Artist {
                name: "B".into(),
                item_count: 550,
                play_count: 11,
                sample_path: "/1".into()
            },
            Artist {
                name: "A".into(),
                item_count: 548,
                play_count: 3,
                sample_path: "/0".into()
            },
        ]
    );
}

#[test]
fn deleted_history_falls_back_to_latest_added_with_stable_ties() {
    let mut summary = Accumulator::new(vec![History {
        id: "missing".into(),
        play_count: 3,
    }])
    .unwrap();
    for i in 0..12 {
        summary
            .push(&i.to_string(), " ", &format!("/{i}"), 50, i)
            .unwrap();
    }
    summary.push("new", " ", "/new", 60, 12).unwrap();
    let result = summary.finish();
    assert!(result.artists.is_empty());
    assert_eq!(result.recent_items, [12, 0, 1, 2, 3, 4, 5, 6]);
    assert!(
        Accumulator::<usize>::new(vec![])
            .unwrap()
            .finish()
            .recent_items
            .is_empty()
    );
}

#[test]
fn duplicate_rows_negative_counts_and_count_overflow_are_errors() {
    assert!(
        Accumulator::<()>::new(vec![History {
            id: "id".into(),
            play_count: -1
        }])
        .is_err()
    );
    let mut summary = Accumulator::new(vec![
        History {
            id: "one".into(),
            play_count: i64::MAX,
        },
        History {
            id: "two".into(),
            play_count: 1,
        },
    ])
    .unwrap();
    summary.push("one", "Artist", "/one", 0, ()).unwrap();
    assert!(summary.push("one", "Artist", "/one", 0, ()).is_err());
    assert!(summary.push("two", "Artist", "/two", 0, ()).is_err());
    assert!(summary.push("", "Artist", "/missing", 0, ()).is_err());
}

#[test]
fn streaming_does_not_retain_all_full_track_payloads() {
    use std::{cell::Cell, rc::Rc};
    struct Payload(Rc<Cell<usize>>);
    impl Clone for Payload {
        fn clone(&self) -> Self {
            self.0.set(self.0.get() + 1);
            Self(self.0.clone())
        }
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.set(self.0.get() - 1);
        }
    }
    let live = Rc::new(Cell::new(0));
    let mut summary = Accumulator::new(
        (0..1100)
            .map(|i| History {
                id: i.to_string(),
                play_count: 1,
            })
            .collect(),
    )
    .unwrap();
    for i in 0..1100 {
        live.set(live.get() + 1);
        summary
            .push(
                &i.to_string(),
                "Artist",
                &format!("/{i}"),
                i,
                Payload(live.clone()),
            )
            .unwrap();
        assert!(live.get() <= 16, "retained {} payloads", live.get());
    }
    let result = summary.finish();
    assert_eq!(live.get(), 8);
    drop(result);
    assert_eq!(live.get(), 0);
}

fn insert_native_audio(
    connection: &rusqlite::Connection,
    id: &str,
    artist: &str,
    created: &str,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT OR IGNORE INTO volumes(id,identity,path,is_available) VALUES(1,'TEST','/',1)",
        [],
    )?;
    let number = match id {
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "trash" => 4,
        "video" => 5,
        _ => 100 + id.strip_prefix("track-").unwrap().parse::<i64>().unwrap(),
    };
    connection.execute("INSERT INTO files(id,volume_id,path,file_name,kind,title,artist,created_at,updated_at,size_bytes,duration_ms) VALUES(?1,1,?2,?2,'AUDIO',?2,?3,?4,?4,1,1000)",rusqlite::params![number,id,artist,created])
}

fn native_database(path: &std::path::Path) -> Db {
    let db = Db::open(path).unwrap();
    db.with_conn(|connection| -> rusqlite::Result<()> {
        insert_native_audio(connection, "one", "Old Artist", "2026-01-01T00:00:00Z")?;
        insert_native_audio(connection, "two", "Old Artist", "2026-01-02T00:00:00Z")?;
        connection.execute("INSERT INTO media_play_history(media_type,file_id,position_ms,play_count,played_at) VALUES(1,'1',0,5,'2026-01-02T00:00:00Z')", [])?;
        Ok(())
    }).unwrap();
    db
}

#[test]
fn native_history_and_catalog_share_one_snapshot_during_external_writes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("db");
    let db = native_database(&path);
    let writer = rusqlite::Connection::open(&path).unwrap();
    AFTER_NATIVE_HISTORY.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            writer
                .execute_batch(
                    "BEGIN;
             UPDATE media_play_history SET play_count=99 WHERE file_id='1';
             UPDATE files SET artist='New Artist' WHERE id=1;
             DELETE FROM files WHERE id=2;",
                )
                .unwrap();
            insert_native_audio(&writer, "three", "New Artist", "2026-01-03T00:00:00Z").unwrap();
            writer.execute_batch("COMMIT").unwrap();
        }))
    });
    let before = native(&db).unwrap();
    assert_eq!(before.artists.len(), 1);
    assert_eq!(before.artists[0].name, "Old Artist");
    assert_eq!(before.artists[0].item_count, 2);
    assert_eq!(before.artists[0].play_count, 5);
    assert_eq!(before.recent_items[0].artist, "Old Artist");
    let after = native(&db).unwrap();
    assert_eq!(after.artists.len(), 1);
    assert_eq!(after.artists[0].name, "New Artist");
    assert_eq!(after.artists[0].item_count, 2);
    assert_eq!(after.artists[0].play_count, 99);
}

#[test]
fn native_scan_covers_more_than_a_page_and_excludes_trash_and_other_media() {
    let directory = tempfile::tempdir().unwrap();
    let db = native_database(&directory.path().join("db"));
    db.with_conn(|connection| -> rusqlite::Result<()> {
        let transaction = connection.unchecked_transaction()?;
        for index in 0..1100 {
            insert_native_audio(
                &transaction,
                &format!("track-{index}"),
                "Artist",
                "2026-01-03T00:00:00Z",
            )?;
        }
        insert_native_audio(&transaction, "trash", "Excluded", "2026-01-03T00:00:00Z")?;
        insert_native_audio(&transaction, "video", "Excluded", "2026-01-03T00:00:00Z")?;
        transaction.execute(
            "UPDATE files SET trashed_at='2026-01-01T00:00:00Z',original_path=path WHERE id=4",
            [],
        )?;
        transaction.execute("UPDATE files SET kind='VIDEO' WHERE id=5", [])?;
        transaction.commit()
    })
    .unwrap();
    let summary = native(&db).unwrap();
    assert_eq!(
        summary
            .artists
            .iter()
            .map(|artist| artist.item_count)
            .sum::<i32>(),
        1102
    );
    assert!(
        !summary
            .artists
            .iter()
            .any(|artist| artist.name == "Excluded")
    );
    assert_eq!(summary.recent_items.len(), 1);
    assert_eq!(summary.recent_items[0].id, "1");
    db.with_conn(|connection| connection.execute("DELETE FROM media_play_history", []))
        .unwrap();
    let fallback = native(&db).unwrap();
    assert_eq!(fallback.recent_items.len(), 8);
    assert!(
        fallback
            .recent_items
            .iter()
            .all(|row| row.path.starts_with("/track-"))
    );
}
