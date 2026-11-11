use super::*;

#[test]
fn explicit_modes_are_persisted_and_used_by_the_shared_queue() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("prefs");
    let prefs = crate::features::prefs::Prefs::load(&path).unwrap();
    for mode in [
        MediaPlayMode::RepeatOne,
        MediaPlayMode::Shuffle,
        MediaPlayMode::Repeat,
    ] {
        crate::features::prefs::settings::set_entry(
            &prefs,
            crate::features::prefs::pref_keys::AUDIO_PLAY_MODE,
            plain_libs::serde_json::json!(mode),
        )
        .unwrap();
        assert_eq!(
            crate::features::media::audio::queue::play_mode(&prefs).unwrap(),
            mode
        );
        let reopened = crate::features::prefs::Prefs::load(&path).unwrap();
        assert_eq!(
            crate::features::media::audio::queue::play_mode(&reopened).unwrap(),
            mode
        );
    }
}

#[test]
fn malformed_stored_modes_are_rejected_without_overwriting_them() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = crate::features::prefs::Prefs::load(&dir.path().join("prefs")).unwrap();
    prefs
        .set(crate::features::prefs::pref_keys::AUDIO_PLAY_MODE, 99)
        .unwrap();
    assert!(crate::features::media::audio::queue::play_mode(&prefs).is_err());
    assert_eq!(
        prefs
            .get::<i32>(crate::features::prefs::pref_keys::AUDIO_PLAY_MODE)
            .unwrap(),
        Some(99)
    );
}
