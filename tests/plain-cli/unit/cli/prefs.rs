use super::*;
use crate::cli::{Cli, Cmd};
use std::path::Path;

async fn invoke(data: &Path, cache: &Path, key: &str, value: &str) -> Result<()> {
    let cli = Cli::try_parse_from([
        "plainapp",
        "prefs",
        "--data-dir",
        data.to_str().unwrap(),
        "--cache-dir",
        cache.to_str().unwrap(),
        "--",
        key,
        value,
    ])
    .unwrap();
    let Cmd::Prefs { ref key, ref value } = cli.command else {
        panic!("prefs command required")
    };
    run(&cli.paths().unwrap(), key, value).await
}

#[tokio::test]
async fn preferences_command_persists_settings_and_preserves_other_values() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    let path = prefs::default_path(&data);
    invoke(&data, &cache, "device_name", "\"CLI fixture\"")
        .await
        .unwrap();
    for value in ["true", "true", "false", "false"] {
        for key in ["developer_mode", "allow_any_host"] {
            invoke(&data, &cache, key, value).await.unwrap();
            let stored = Prefs::load(&path).unwrap();
            assert_eq!(stored.get::<bool>(key).unwrap(), Some(value == "true"));
            assert_eq!(
                stored.get::<String>("device_name").unwrap().as_deref(),
                Some("CLI fixture")
            );
        }
    }
    invoke(&data, &cache, "http_port", "8088").await.unwrap();
    assert_eq!(
        Prefs::load(&path).unwrap().get::<u16>("http_port").unwrap(),
        Some(8088)
    );
}

#[tokio::test]
async fn preferences_command_rejects_invalid_or_protected_writes() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    invoke(&data, &cache, "developer_mode", "true")
        .await
        .unwrap();
    let path = prefs::default_path(&data);
    let before = std::fs::read(&path).unwrap();
    for (key, value) in [
        ("developer_mode", "\"true\""),
        ("allow_any_host", "1"),
        ("http_port", "-1"),
        ("unknown_key", "true"),
        ("client_id", "\"replacement\""),
        ("device_name", "not-json"),
    ] {
        assert!(
            invoke(&data, &cache, key, value).await.is_err(),
            "{key}: {value}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[tokio::test]
async fn preferences_command_preserves_corrupt_configuration() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    std::fs::create_dir_all(&data).unwrap();
    let path = prefs::default_path(&data);
    std::fs::write(&path, "invalid json").unwrap();
    assert!(
        invoke(&data, &cache, "developer_mode", "true")
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid json");
}
