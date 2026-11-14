use super::*;
use crate::cli::{Cli, Cmd};

#[test]
fn developer_mode_commands_persist_both_states_and_preserve_other_settings() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    let path = prefs::default_path(&data);
    let prefs = prefs::Prefs::load(&path).unwrap();
    settings::patch(
        &prefs,
        settings::SettingsPatch {
            device_name: Some("CLI fixture".into()),
            ..Default::default()
        },
    )
    .unwrap();
    for (state, enabled) in [("on", true), ("on", true), ("off", false), ("off", false)] {
        let cli = Cli::try_parse_from([
            "plainapp",
            "dev",
            state,
            "--data-dir",
            data.to_str().unwrap(),
            "--cache-dir",
            cache.to_str().unwrap(),
        ])
        .unwrap();
        let Cmd::Dev { enabled: parsed } = cli.command else {
            panic!("dev command required")
        };
        run(&cli.paths().unwrap(), parsed).unwrap();
        let stored = prefs::Prefs::load(&path).unwrap();
        let settings = settings::snapshot(&stored).unwrap();
        assert_eq!(settings.developer_mode, enabled);
        assert_eq!(settings.device_name, "CLI fixture");
    }
}

#[test]
fn developer_mode_command_rejects_invalid_preferences_without_overwriting_them() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    std::fs::create_dir_all(&data).unwrap();
    let path = prefs::default_path(&data);
    std::fs::write(&path, "invalid json").unwrap();
    let cli = Cli::try_parse_from([
        "plainapp",
        "dev",
        "on",
        "--data-dir",
        data.to_str().unwrap(),
        "--cache-dir",
        cache.to_str().unwrap(),
    ])
    .unwrap();
    assert!(run(&cli.paths().unwrap(), true).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid json");
}
