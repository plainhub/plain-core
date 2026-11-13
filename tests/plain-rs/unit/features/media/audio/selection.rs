use super::*;
#[path = "../../library/fixtures.rs"]
mod fixtures;

#[test]
fn native_queue_selection_preserves_manual_skip_and_repeat_one() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = fixtures::test_db("selection");
    let prefs = Prefs::load(&dir.path().join("prefs")).unwrap();
    let choose = |action| {
        select(&db, &prefs, action, |path| Ok(AudioTrack::from_path_stem(path)))
            .unwrap()
            .map(|track| track.path)
    };
    assert_eq!(choose(AudioSelectionAction::Next), None);
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
    queue::save_audio_mode(&prefs, "REPEAT_ONE").unwrap();
    assert_eq!(choose(AudioSelectionAction::Completed).as_deref(), Some("one"));
    assert_eq!(choose(AudioSelectionAction::Next).as_deref(), Some("two"));
    queue::save_audio_current(&db, "one").unwrap();
    assert_eq!(choose(AudioSelectionAction::Previous).as_deref(), Some("two"));
    queue::save_audio_current(&db, "one").unwrap();
    queue::save_audio_mode(&prefs, "SHUFFLE").unwrap();
    assert_eq!(choose(AudioSelectionAction::Next).as_deref(), Some("two"));
}
