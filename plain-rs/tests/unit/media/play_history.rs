use super::*;
use crate::utils::search_dsl::parse;

fn database() -> Db {
    Db::open(std::path::Path::new(":memory:")).unwrap()
}

#[test]
fn saving_progress_does_not_count_starts_or_change_the_local_player() {
    let db = database();
    let before = crate::media::audio::playback::snapshot(&db).unwrap();
    save(&db, DataType::Audio, "audio", 123).unwrap();
    assert_eq!(matching(&db, &[]).unwrap()[0].play_count, 0);
    record(&db, DataType::Audio, "audio", 234).unwrap();
    save(&db, DataType::Audio, "audio", 456).unwrap();
    save(&db, DataType::Audio, "audio", 456).unwrap();
    let rows = matching(&db, &[]).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].play_count, rows[0].position_ms), (1, 456));
    record(&db, DataType::Audio, "audio", 0).unwrap();
    assert_eq!(matching(&db, &[]).unwrap()[0].play_count, 2);
    assert_eq!(
        crate::media::audio::playback::snapshot(&db).unwrap(),
        before
    );
}

#[test]
fn video_progress_uses_shared_history_and_respects_type_and_time_filters() {
    let db = database();
    save(&db, DataType::Video, "42", 5_000).unwrap();
    record(&db, DataType::Audio, "audio", 10).unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE media_play_history SET played_at='2026-01-01T00:00:00.000Z'",
            [],
        )
    })
    .unwrap();
    let rows = matching(&db, &parse("type:VIDEO ids:42")).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].position_ms, rows[0].play_count), (5_000, 0));
    assert_eq!(
        matching(
            &db,
            &parse("type:VIDEO played_at:>=2026-01-01T00:00:00.000Z")
        )
        .unwrap()
        .len(),
        1
    );
    assert!(
        matching(
            &db,
            &parse("type:VIDEO played_at:>=2027-01-01T00:00:00.000Z")
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        matching(&db, &parse("type:VIDEO ids:audio"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn matching_combines_ids_counts_and_positions_without_reordering_history() {
    let db = database();
    for (kind, id, position) in [
        (DataType::Audio, "first", 100),
        (DataType::Video, "video", 200),
        (DataType::Audio, "last", 300),
    ] {
        record(&db, kind, id, position).unwrap();
    }
    record(&db, DataType::Audio, "last", 300).unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE media_play_history SET played_at='2026-01-01T00:00:00.000Z'",
            [],
        )
    })
    .unwrap();
    let rows = matching(
        &db,
        &parse("ids:first,last position_ms:>=100 play_count:>=1"),
    )
    .unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row.file_id.as_str())
            .collect::<Vec<_>>(),
        ["last", "first"]
    );
    let rows = matching(&db, &parse("type:AUDIO play_count:>1")).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].file_id, "last");
    assert_eq!(
        matching(&db, &parse("NOT type:AUDIO")).unwrap()[0].file_id,
        "video"
    );
}

#[test]
fn invalid_inputs_and_type_collisions_preserve_existing_records() {
    let db = database();
    record(&db, DataType::Audio, "audio", 123).unwrap();
    let before = matching(&db, &[]).unwrap();
    for (kind, id, position) in [
        (DataType::Image, "image", 0),
        (DataType::Audio, "", 0),
        (DataType::Video, "  ", 0),
        (DataType::Audio, "negative", -1),
        (DataType::Video, "audio", 999),
    ] {
        assert!(record(&db, kind, id, position).is_err());
        assert!(save(&db, kind, id, position).is_err());
        assert_eq!(matching(&db, &[]).unwrap(), before);
    }
    for query in [
        "type:IMAGE",
        "unknown:value",
        "play_count:not-a-number",
        "ids:>audio",
    ] {
        assert!(matching(&db, &parse(query)).is_err(), "{query}");
        assert_eq!(matching(&db, &[]).unwrap(), before);
    }
}

#[test]
fn text_filters_match_current_metadata_and_respect_negation() {
    let values = ["Road Song", "Ferry", "/music/road.mp3"];
    assert!(matches_text(&parse("fErRy"), &values));
    assert!(matches_text(&parse("road ferry"), &values));
    assert!(!matches_text(&parse("road absent"), &values));
    assert!(matches_text(&parse("NOT text:=absent"), &values));
    assert!(!matches_text(&parse("NOT text:=ferry"), &values));
}

#[test]
fn video_progress_preserves_exact_milliseconds_after_restart_and_uses_inclusive_time_boundaries() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("db");
    let db = Db::open(&path).unwrap();
    assert!(save(&db, DataType::Video, " ", 12).is_err());
    assert!(save(&db, DataType::Video, "video", -1).is_err());
    save(&db, DataType::Video, "video", 3_000_000_123).unwrap();
    let played_at = matching(&db, &[]).unwrap()[0].played_at.clone();
    drop(db);
    let db = Db::open(&path).unwrap();
    assert_eq!(matching(&db, &[]).unwrap()[0].position_ms, 3_000_000_123);
    let boundary = chrono::DateTime::parse_from_rfc3339(&played_at).unwrap();
    let fields = |time: String| {
        vec![crate::utils::search_dsl::FilterField {
            name: "played_at".into(),
            op: ">=".into(),
            value: time,
        }]
    };
    assert_eq!(matching(&db, &fields(played_at)).unwrap().len(), 1);
    let later = (boundary + chrono::Duration::milliseconds(1)).to_rfc3339();
    assert!(matching(&db, &fields(later)).unwrap().is_empty());
    save(&db, DataType::Video, "video", 0).unwrap();
    assert_eq!(matching(&db, &[]).unwrap()[0].position_ms, 0);
}

#[test]
fn a_failed_video_progress_save_preserves_the_previous_position() {
    let db = database();
    save(&db, DataType::Video, "video", 123).unwrap();
    let before = matching(&db, &[]).unwrap();
    db.with_conn(|conn|conn.execute_batch("CREATE TRIGGER fail_progress BEFORE INSERT ON media_play_history BEGIN SELECT RAISE(ABORT, 'fail'); END")).unwrap();
    assert!(save(&db, DataType::Video, "video", 456).is_err());
    assert_eq!(matching(&db, &[]).unwrap(), before);
}
