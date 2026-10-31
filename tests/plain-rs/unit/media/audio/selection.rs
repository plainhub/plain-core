use super::*;
#[path = "../../library/fixtures.rs"]
mod fixtures;

#[test]
fn next_previous_and_repeat_one_share_selection_without_a_player_command() {
    let dir = tempfile::tempdir().unwrap();
    let db = fixtures::test_db("selection");
    let prefs = Prefs::load(&dir.path().join("prefs")).unwrap();
    fixtures::indexed_enqueue(
        &db,
        &[
            AudioTrack::from_path_stem("one"),
            AudioTrack::from_path_stem("two"),
        ],
        false,
    )
    .unwrap();
    let start = playback::snapshot(&db).unwrap();
    let first = advance(&db, &prefs, AudioSelectionAction::Next, &start, |_| {
        panic!("unexpected metadata")
    })
    .unwrap()
    .unwrap();
    assert_eq!(first.track.unwrap().path, "one");
    let previous = advance(
        &db,
        &prefs,
        AudioSelectionAction::Previous,
        &first.playback,
        |_| panic!("unexpected metadata"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(previous.track.unwrap().path, "two");
    queue::save_audio_mode(&prefs, "REPEAT_ONE").unwrap();
    let mut reads = 0;
    let repeated = advance(
        &db,
        &prefs,
        AudioSelectionAction::Completed,
        &previous.playback,
        |path| {
            reads += 1;
            Ok(AudioTrack::from_path_stem(path))
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(reads, 1);
    assert_eq!(repeated.track.unwrap().path, "two");
    assert_ne!(repeated.playback.revision, previous.playback.revision);
    let manual = advance(
        &db,
        &prefs,
        AudioSelectionAction::Next,
        &repeated.playback,
        |_| panic!("unexpected metadata"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(manual.track.unwrap().path, "one");
}

#[test]
fn stale_and_duplicate_completions_have_no_selection_or_metadata_effect() {
    let dir = tempfile::tempdir().unwrap();
    let db = fixtures::test_db("stale-selection");
    let prefs = Prefs::load(&dir.path().join("prefs")).unwrap();
    fixtures::indexed_enqueue(&db, &[AudioTrack::from_path_stem("one")], false).unwrap();
    let before = playback::snapshot(&db).unwrap();
    let selected = advance(&db, &prefs, AudioSelectionAction::Next, &before, |_| {
        panic!("metadata")
    })
    .unwrap()
    .unwrap();
    assert!(
        advance(
            &db,
            &prefs,
            AudioSelectionAction::Completed,
            &before,
            |_| panic!("metadata")
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(playback::snapshot(&db).unwrap(), selected.playback);
    let forged = Playback {
        path: "other".into(),
        ..selected.playback.clone()
    };
    assert!(
        advance(
            &db,
            &prefs,
            AudioSelectionAction::Completed,
            &forged,
            |_| panic!("metadata")
        )
        .unwrap()
        .is_none()
    );
    let invalid = Playback {
        revision: -1,
        ..selected.playback
    };
    assert!(
        advance(
            &db,
            &prefs,
            AudioSelectionAction::Next,
            &invalid,
            |_| panic!("metadata")
        )
        .is_err()
    );
}

#[test]
fn shuffle_excludes_the_current_track_and_empty_queues_do_not_fabricate_tracks() {
    let dir = tempfile::tempdir().unwrap();
    let db = fixtures::test_db("shuffle-selection");
    let prefs = Prefs::load(&dir.path().join("prefs")).unwrap();
    let initial = playback::snapshot(&db).unwrap();
    let empty = advance(&db, &prefs, AudioSelectionAction::Next, &initial, |_| {
        panic!("metadata")
    })
    .unwrap()
    .unwrap();
    assert!(empty.track.is_none());
    fixtures::indexed_enqueue(
        &db,
        &[
            AudioTrack::from_path_stem("one"),
            AudioTrack::from_path_stem("two"),
        ],
        false,
    )
    .unwrap();
    queue::save_audio_current(&db, "one").unwrap();
    queue::save_audio_mode(&prefs, "SHUFFLE").unwrap();
    for _ in 0..8 {
        let before = playback::snapshot(&db).unwrap();
        let selected = advance(&db, &prefs, AudioSelectionAction::Next, &before, |_| {
            panic!("metadata")
        })
        .unwrap()
        .unwrap();
        assert_ne!(selected.track.unwrap().path, before.path);
    }
}

#[test]
fn explicit_shuffle_selection_avoids_current_track_without_changing_the_saved_play_mode() {
    let dir = tempfile::tempdir().unwrap();
    let db = fixtures::test_db("explicit-shuffle");
    let prefs = Prefs::load(&dir.path().join("prefs")).unwrap();
    fixtures::indexed_enqueue(
        &db,
        &[
            AudioTrack::from_path_stem("one"),
            AudioTrack::from_path_stem("two"),
        ],
        false,
    )
    .unwrap();
    queue::save_audio_mode(&prefs, "REPEAT_ONE").unwrap();
    let start = playback::snapshot(&db).unwrap();
    let first = advance(&db, &prefs, AudioSelectionAction::Next, &start, |_| {
        panic!("unexpected metadata")
    })
    .unwrap()
    .unwrap();
    let shuffled = advance_with_shuffle(
        &db,
        &prefs,
        AudioSelectionAction::Next,
        &first.playback,
        true,
        |_| panic!("unexpected metadata"),
    )
    .unwrap()
    .unwrap();
    assert_ne!(shuffled.playback.path, first.playback.path);
    assert_eq!(queue::play_mode(&prefs).unwrap(), MediaPlayMode::RepeatOne);
}
