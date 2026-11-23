use super::*;
#[path = "../../library/fixtures.rs"]
mod fixtures;

fn add(db: &Db, playlist: &str, id: &str, order: i64) {
    db.cache_platform_audio(
        &[crate::infrastructure::db::audio_catalog::AudioMedia {
            file_id: Some(id.into()),
            aliases: Vec::new(),
            track: crate::features::media::audio::queue::AudioTrack {
                file_id: None,
                path: format!("/{id}.mp3"),
                title: id.into(),
                artist: String::new(),
                duration_ms: 1,
            },
        }],
        false,
    )
    .unwrap();
    db.with_conn(|conn| conn.execute(
        "INSERT INTO audio_playlist_items(id,playlist_id,file_id,sort_order,added_at) VALUES(?1,?2,?3,?4,'now')",
        plain_libs::rusqlite::params![format!("{playlist}-{id}"),playlist,id,order],
    )).unwrap();
}

#[test]
fn candidates_follow_playlist_order_and_include_tracks_without_album_tags() {
    let db = fixtures::test_db("covers");
    db.use_platform_audio_catalog().unwrap();
    add(&db, "a", "last", 3);
    add(&db, "a", "first", 0);
    add(&db, "b", "other", 0);
    add(&db, "a", "middle", 1);
    let rows = playlist_cover_candidates(&db, &["b".into(), "a".into(), "missing".into()]).unwrap();
    assert_eq!(rows[0], ["/other.mp3"]);
    assert_eq!(rows[1], ["/first.mp3", "/middle.mp3", "/last.mp3"]);
    assert!(rows[2].is_empty());
    assert!(playlist_cover_candidates(&db, &[]).unwrap().is_empty());
}
