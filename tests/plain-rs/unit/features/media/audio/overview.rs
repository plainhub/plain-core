use super::*;

#[test]
fn artist_statistics_cover_the_full_catalog_and_ignore_missing_tracks() {
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
            .push(&i.to_string(), artist, &format!("/{i}"))
            .unwrap();
    }
    let result = summary.finish();
    assert_eq!(
        result,
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
fn duplicate_rows_negative_counts_and_count_overflow_are_errors() {
    assert!(
        Accumulator::new(vec![History {
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
    summary.push("one", "Artist", "/one").unwrap();
    assert!(summary.push("one", "Artist", "/one").is_err());
    assert!(summary.push("two", "Artist", "/two").is_err());
    assert!(summary.push("", "Artist", "/missing").is_err());
}

fn insert_native_audio(
    connection: &plain_libs::rusqlite::Connection,
    id: &str,
    artist: &str,
    created: &str,
) -> plain_libs::rusqlite::Result<usize> {
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
    connection.execute("INSERT INTO files(id,volume_id,path,file_name,kind,title,artist,created_at,updated_at,size_bytes,duration_ms) VALUES(?1,1,?2,?2,'AUDIO',?2,?3,?4,?4,1,1000)",plain_libs::rusqlite::params![number,id,artist,created])
}

fn native_database(path: &std::path::Path) -> Db {
    let db = Db::open(path).unwrap();
    db.with_conn(|connection| -> plain_libs::rusqlite::Result<()> {
        insert_native_audio(connection, "one", "Old Artist", "2026-01-01T00:00:00Z")?;
        insert_native_audio(connection, "two", "Old Artist", "2026-01-02T00:00:00Z")?;
        connection.execute("INSERT INTO media_play_history(media_type,file_id,position_ms,play_count,played_at) VALUES(1,'1',0,5,'2026-01-02T00:00:00Z')", [])?;
        Ok(())
    }).unwrap();
    db
}

#[test]
fn native_history_and_catalog_share_one_snapshot_during_external_writes() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("db");
    let db = native_database(&path);
    let writer = plain_libs::rusqlite::Connection::open(&path).unwrap();
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
    assert_eq!(before.len(), 1);
    assert_eq!(before[0].name, "Old Artist");
    assert_eq!(before[0].item_count, 2);
    assert_eq!(before[0].play_count, 5);
    let after = native(&db).unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].name, "New Artist");
    assert_eq!(after[0].item_count, 2);
    assert_eq!(after[0].play_count, 99);
}

#[test]
fn native_scan_covers_more_than_a_page_and_excludes_trash_and_other_media() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = native_database(&directory.path().join("db"));
    db.with_conn(|connection| -> plain_libs::rusqlite::Result<()> {
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
            .iter()
            .map(|artist| artist.item_count)
            .sum::<i32>(),
        1102
    );
    assert!(
        !summary
            .iter()
            .any(|artist| artist.name == "Excluded")
    );
}
