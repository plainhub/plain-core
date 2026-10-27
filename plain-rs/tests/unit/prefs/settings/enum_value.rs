use crate::prefs::{Prefs, pref_keys as keys, settings};
use serde_json::json;

#[test]
fn published_integer_preferences_decode_to_current_names_without_rewriting_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("prefs.json");
    let prefs = Prefs::load(&path).unwrap();
    let themes: &[&str] = &["USE_DEVICE_THEME", "ON", "OFF"];
    let modes: &[&str] = &["REPEAT", "REPEAT_ONE", "SHUFFLE"];
    let sorts: &[&str] = &[
        "DATE_ASC",
        "DATE_DESC",
        "SIZE_ASC",
        "SIZE_DESC",
        "NAME_ASC",
        "NAME_DESC",
        "TAKEN_AT_DESC",
    ];
    for (key, names) in [
        (keys::DARK_THEME, themes),
        (keys::AUDIO_PLAY_MODE, modes),
        (keys::AUDIO_SORT_BY, sorts),
        (keys::VIDEO_SORT_BY, sorts),
        (keys::IMAGE_SORT_BY, sorts),
        (keys::DOC_SORT_BY, sorts),
        (keys::FILE_SORT_BY, sorts),
        (keys::PACKAGE_SORT_BY, sorts),
    ] {
        for (number, name) in names.iter().enumerate() {
            prefs.set(key, number).unwrap();
            let before = std::fs::read(&path).unwrap();
            let reopened = Prefs::load(&path).unwrap();
            assert_eq!(
                settings::get_entry(&reopened, key).unwrap(),
                Some(json!(name))
            );
            assert_eq!(settings::values(&reopened).unwrap()[key], json!(name));
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert!(
                settings::set_entry(&reopened, key, json!(name))
                    .unwrap()
                    .unwrap()
            );
            let named = Prefs::load(&path).unwrap();
            assert_eq!(settings::get_entry(&named, key).unwrap(), Some(json!(name)));
            assert_eq!(named.get::<String>(key).unwrap().as_deref(), Some(*name));
        }
    }
}

#[test]
fn invalid_enum_preferences_are_rejected_and_preserved() {
    for (key, limit) in [
        (keys::DARK_THEME, 3),
        (keys::AUDIO_PLAY_MODE, 3),
        (keys::FILE_SORT_BY, 7),
    ] {
        for value in [
            json!(-1),
            json!(limit),
            json!(99),
            json!(2.0),
            json!("2"),
            json!("UNKNOWN"),
            json!(null),
            json!(true),
            json!({"OFF": null}),
            json!([]),
        ] {
            let prefs = Prefs::in_memory();
            prefs.set(key, &value).unwrap();
            assert!(settings::get_entry(&prefs, key).is_err(), "{key}: {value}");
            assert!(settings::values(&prefs).is_err(), "{key}: {value}");
            assert_eq!(prefs.get::<serde_json::Value>(key).unwrap(), Some(value));
        }
    }
}
