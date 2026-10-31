use std::{io::Read, path::PathBuf};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn native_main_uses_the_shared_default_path_without_host_requests() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let path = plain_rs::data_paths::DataPaths::new(directory.path()).log_file();
    assert_eq!(core.log_file().unwrap(), path);
    let contents = b"INFO previous\nERROR current\n";
    std::fs::write(&path, contents).unwrap();
    let native = core.native_graphql().unwrap();
    let (generation, mut requests) = core.runtime_state().host.connect();
    for _ in 0..3 {
        let response = native
            .execute(async_graphql::Request::new(
                "{ appLogPath appLogs(query:\"\",offset:0,limit:2) appLogFilesSize }",
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        assert_eq!(data["appLogPath"], path.to_string_lossy().as_ref());
        assert_eq!(
            data["appLogs"],
            serde_json::json!(["ERROR current", "INFO previous"])
        );
        assert_eq!(data["appLogFilesSize"], contents.len());
    }
    assert!(requests.try_recv().is_err());
    core.runtime_state().host.disconnect(generation);
    core.stop_core().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn native_main_uses_the_injected_logger_path_without_host_requests() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let path = directory.path().join("plain.log");
    std::fs::write(&path, b"INFO old\nERROR current\n").unwrap();
    core.set_log_file(&path).unwrap();
    let (_generation, mut requests) = core.runtime_state().host.connect();
    let native = core.native_graphql().unwrap();
    let response = native
        .execute(async_graphql::Request::new(
            "{ appLogPath appLogs(query: \"text:ERROR\", offset: 0, limit: 1) }",
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["appLogPath"], path.to_string_lossy().as_ref());
    assert_eq!(data["appLogs"], serde_json::json!(["ERROR current"]));
    let response = native
        .execute(async_graphql::Request::new("mutation { clearAppLogs }"))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(std::fs::read(&path).unwrap().is_empty());
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn main_log_export_and_clear_cover_app_and_rust_log_families() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let path = directory.path().join("plain.log");
    let backup = directory.path().join("plain.log.bak");
    let unrelated = directory.path().join("unrelated.txt");
    std::fs::write(&path, b"current bytes").unwrap();
    std::fs::write(&backup, b"rotated bytes").unwrap();
    std::fs::write(&unrelated, b"not a log").unwrap();
    let rust_path = plain_rs::data_paths::DataPaths::new(core.data_directory()).rust_log_file();
    std::fs::write(&rust_path, b"rust current").unwrap();
    std::fs::write(rust_path.with_extension("log.bak"), b"rust rotated").unwrap();
    core.set_log_file(&path).unwrap();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let existing = state.cache.root().join("logs.zip");
    std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
    std::fs::write(&existing, b"preserved archive").unwrap();
    let native = core.native_graphql().unwrap();
    let response = native
        .execute(async_graphql::Request::new("mutation { exportAppLogs }"))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let exported = PathBuf::from(data["exportAppLogs"].as_str().unwrap());
    assert_ne!(exported, existing);
    assert_eq!(exported.file_name().unwrap(), "logs (1).zip");
    assert_eq!(std::fs::read(existing).unwrap(), b"preserved archive");
    let mut zip = zip::ZipArchive::new(std::fs::File::open(exported).unwrap()).unwrap();
    let mut names = zip
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        vec![
            "logs/plain.log",
            "logs/plain.log.bak",
            "logs/rust.log",
            "logs/rust.log.bak"
        ]
    );
    for (name, expected) in [
        ("logs/plain.log", "current bytes"),
        ("logs/plain.log.bak", "rotated bytes"),
        ("logs/rust.log", "rust current"),
        ("logs/rust.log.bak", "rust rotated"),
    ] {
        let mut text = String::new();
        zip.by_name(name)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, expected);
    }
    let response = native
        .execute(async_graphql::Request::new("mutation { clearAppLogs }"))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(std::fs::read(&path).unwrap().is_empty());
    assert!(!backup.exists());
    let response = native
        .execute(async_graphql::Request::new("{ appLogFilesSize }"))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(std::fs::read(&rust_path).unwrap().is_empty());
    assert!(!rust_path.with_extension("log.bak").exists());
    assert_eq!(response.data.into_json().unwrap()["appLogFilesSize"], 0);
    let response = native
        .execute(async_graphql::Request::new("mutation { clearAppLogFiles }"))
        .await;
    assert!(!response.errors.is_empty());
    assert!(!backup.exists());
    assert_eq!(std::fs::read(unrelated).unwrap(), b"not a log");
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}
