use super::*;

#[test]
fn all_platforms_derive_one_layout_from_the_data_root() {
    let directory = std::env::temp_dir().join("plain-data-layout");
    let paths = DataPaths::new(&directory);
    for (actual, relative) in [
        (paths.database(), "plain.db"),
        (paths.prefs(), "prefs.json"),
        (paths.note_images(), "note-images"),
        (paths.audio(), "audio"),
        (paths.logs(), "logs"),
        (paths.log_file(), "logs/latest.log"),
        (paths.rust_log_file(), "logs/rust.log"),
        (paths.crash_report(), "crash_report.txt"),
        (paths.crash_log(), "crash_log.txt"),
        (paths.pomodoro_sound(), "audio/pomodoro_sound.mp3"),
        (paths.tls_identity(), "tls-identity.json"),
    ] {
        assert_eq!(actual, directory.join(relative));
        assert_eq!(
            actual.strip_prefix(&directory).unwrap(),
            Path::new(relative)
        );
    }
}

#[cfg(unix)]
#[test]
fn fixed_directories_cannot_redirect_application_io() {
    for child in ["note-images", "audio", "logs"] {
        let directory = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let paths = DataPaths::new(directory.path());
        let link = directory.path().join(child);
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(external.path(), link).unwrap();
        assert!(paths.initialize_directories().is_err(), "{child}");
        assert_eq!(std::fs::read_dir(external.path()).unwrap().count(), 0);
    }
}
