use super::*;
use crate::cli::{Cli, Cmd};
use plain_rs::features::prefs::{self, Prefs};
use std::path::Path;

async fn invoke(data: &Path, key: &str, value: &str) -> Result<()> {
    let cli = Cli::try_parse_from([
        "plainapp",
        "prefs",
        "--data-dir",
        data.to_str().unwrap(),
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

async fn start(
    data: &Path,
    cache: &Path,
) -> (
    plain_server::server::ContentServer,
    crate::management_socket::Service,
) {
    let core = plain_server::server::ContentServer::open_with_cache(data, cache).unwrap();
    let service = crate::management_socket::Service::start(&core)
        .await
        .unwrap()
        .unwrap();
    (core, service)
}

async fn stop(
    core: plain_server::server::ContentServer,
    service: crate::management_socket::Service,
) {
    core.stop_core().await;
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn preferences_command_persists_settings_and_preserves_other_values() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    let path = prefs::default_path(&data);
    let (core, service) = start(&data, &cache).await;
    core.runtime()
        .prefs
        .set("device_name", "CLI fixture")
        .unwrap();
    for value in ["true", "true", "false", "false"] {
        for key in ["developer_mode", "allow_any_host"] {
            invoke(&data, key, value).await.unwrap();
            let stored = Prefs::load(&path).unwrap();
            assert_eq!(stored.get::<bool>(key).unwrap(), Some(value == "true"));
            assert_eq!(
                stored.get::<String>("device_name").unwrap().as_deref(),
                Some("CLI fixture")
            );
        }
    }
    stop(core, service).await;
}

#[tokio::test]
async fn preferences_command_rejects_invalid_or_protected_writes() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    let (core, service) = start(&data, &cache).await;
    invoke(&data, "developer_mode", "true").await.unwrap();
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
        assert!(invoke(&data, key, value).await.is_err(), "{key}: {value}");
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    stop(core, service).await;
}

#[tokio::test]
async fn preferences_command_preserves_corrupt_configuration() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    std::fs::create_dir_all(&data).unwrap();
    let path = prefs::default_path(&data);
    std::fs::write(&path, "invalid json").unwrap();
    assert!(invoke(&data, "developer_mode", "true").await.is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid json");
    assert!(!cache.exists());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn preferences_command_updates_running_instance_and_publishes_changes() {
    const CHILD: &str = "PLAINAPP_ONLINE_PREFS_TEST_DATA";
    if let Some(data) = std::env::var_os(CHILD) {
        let data = std::path::PathBuf::from(data);
        invoke(&data, "developer_mode", "true").await.unwrap();
        return;
    }
    let root = plain_libs::temporary::tempdir().unwrap();
    let data = root.path().join("data");
    let cache = root.path().join("cache");
    let core = plain_server::server::ContentServer::open_with_cache(&data, &cache).unwrap();
    let listener = crate::management_socket::Service::start(&core)
        .await
        .unwrap()
        .unwrap();
    let mut events = core.runtime().events.subscribe();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "cli::prefs::tests::preferences_command_updates_running_instance_and_publishes_changes",
            "--nocapture",
        ])
        .env(CHILD, &data)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        core.runtime().prefs.get::<bool>("developer_mode").unwrap(),
        Some(true)
    );
    assert_eq!(events.try_recv().unwrap().event_type, "PREFS_UPDATED");
    core.stop_core().await;
    listener.shutdown().await.unwrap();
}

#[tokio::test]
async fn preferences_command_does_not_create_or_write_an_offline_instance() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let data = root.path().join("absent");
    let cache = root.path().join("cache");
    assert!(invoke(&data, "developer_mode", "true").await.is_err());
    assert!(!data.exists());
    assert!(!cache.exists());
}
