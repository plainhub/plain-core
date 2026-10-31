use super::*;

#[test]
fn explicit_modes_are_persisted_and_used_by_the_shared_queue() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("prefs");
    let prefs = crate::prefs::Prefs::load(&path).unwrap();
    for mode in [
        MediaPlayMode::RepeatOne,
        MediaPlayMode::Shuffle,
        MediaPlayMode::Repeat,
    ] {
        crate::prefs::settings::set_entry(
            &prefs,
            crate::prefs::pref_keys::AUDIO_PLAY_MODE,
            serde_json::json!(mode),
        )
        .unwrap();
        assert_eq!(crate::media::audio::queue::play_mode(&prefs).unwrap(), mode);
        let reopened = crate::prefs::Prefs::load(&path).unwrap();
        assert_eq!(
            crate::media::audio::queue::play_mode(&reopened).unwrap(),
            mode
        );
    }
}

#[test]
fn malformed_stored_modes_are_rejected_without_overwriting_them() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = crate::prefs::Prefs::load(&dir.path().join("prefs")).unwrap();
    prefs
        .set(crate::prefs::pref_keys::AUDIO_PLAY_MODE, 99)
        .unwrap();
    assert!(crate::media::audio::queue::play_mode(&prefs).is_err());
    assert_eq!(
        prefs
            .get::<i32>(crate::prefs::pref_keys::AUDIO_PLAY_MODE)
            .unwrap(),
        Some(99)
    );
}
