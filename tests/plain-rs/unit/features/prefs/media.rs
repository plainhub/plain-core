use super::*;

#[test]
fn source_directories_round_trip_and_preserve_other_settings() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    assert!(source_directories(&prefs).unwrap().is_empty());
    prefs.set("developer_mode", true).unwrap();
    let sources = vec!["/media/a".into(), "/media/b".into(), "/media/a".into()];
    set_source_directories(&prefs, &sources).unwrap();
    let reloaded = Prefs::load(prefs.path()).unwrap();
    assert_eq!(source_directories(&reloaded).unwrap(), sources);
    assert_eq!(reloaded.get::<bool>("developer_mode").unwrap(), Some(true));
    set_source_directories(&prefs, &[]).unwrap();
    assert!(source_directories(&prefs).unwrap().is_empty());
}

#[test]
fn invalid_source_directory_value_is_an_error() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    prefs.set(MEDIA_SOURCE_DIRS, "invalid").unwrap();
    assert!(source_directories(&prefs).is_err());
}
