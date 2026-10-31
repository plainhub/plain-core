use super::*;
#[path = "../../library/fixtures.rs"]
mod fixtures;

fn add(db: &Db, playlist: &str, id: &str, album: &str, order: i64) {
    db.cache_platform_audio(
        &[crate::db::audio_catalog::AudioMedia {
            file_id: Some(id.into()),
            aliases: Vec::new(),
            track: crate::media::audio::queue::AudioTrack {
                file_id: None,
                path: format!("/{id}.mp3"),
                title: id.into(),
                artist: String::new(),
                album_id: album.into(),
                duration_ms: 1,
            },
        }],
        false,
    )
    .unwrap();
    db.with_conn(|conn| conn.execute(
        "INSERT INTO audio_playlist_items(id,playlist_id,file_id,sort_order,added_at) VALUES(?1,?2,?3,?4,'now')",
        rusqlite::params![format!("{playlist}-{id}"),playlist,id,order],
    )).unwrap();
}

#[test]
fn covers_rank_by_count_break_ties_by_playlist_order_and_bound_candidates() {
    let db = fixtures::test_db("covers");
    db.use_platform_audio_catalog().unwrap();
    add(&db, "a", "first", "early", 0);
    for i in 0..5 {
        add(&db, "a", &format!("popular-{i}"), "popular", i + 1);
    }
    add(&db, "a", "last", "late", 6);
    add(&db, "a", "unknown", "", 7);
    add(&db, "a", "blank", "  ", 8);
    let rows = playlist_covers(&db, &["a".into(), "missing".into()]).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0]
            .albums
            .iter()
            .map(|a| a.album_id.as_str())
            .collect::<Vec<_>>(),
        ["popular", "early", "late"]
    );
    assert_eq!(rows[0].albums[0].track_count, 5);
    assert_eq!(
        rows[0].albums[0].candidate_paths,
        ["/popular-0.mp3", "/popular-1.mp3", "/popular-2.mp3"]
    );
    assert!(rows[1].albums.is_empty());
}

#[test]
fn batches_are_isolated_and_cover_limits_do_not_truncate_album_counts() {
    let db = fixtures::test_db("cover-limits");
    db.use_platform_audio_catalog().unwrap();
    for i in 0..10 {
        add(&db, "a", &format!("a-{i}"), &format!("album-{i}"), i);
    }
    add(&db, "b", "b", "other", 0);
    let rows = playlist_covers(&db, &["b".into(), "a".into(), "b".into()]).unwrap();
    assert_eq!(rows[0], rows[2]);
    assert_eq!(rows[0].albums[0].album_id, "other");
    assert_eq!(rows[1].albums.len(), 8);
    assert_eq!(rows[1].albums.last().unwrap().album_id, "album-7");
    assert!(playlist_covers(&db, &[]).unwrap().is_empty());
}
