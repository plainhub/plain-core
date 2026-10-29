use super::*;
use std::io::{Cursor, Write};

#[test]
fn filtering_precedes_paging_and_large_offsets_remain_bounded() {
    let bytes = b"MATCH-old\r\n\nnoise\nmatch-middle\nnoise\nMATCH-new";
    assert_eq!(
        read(
            &mut Cursor::new(bytes),
            bytes.len() as u64,
            Some("match"),
            1,
            1
        )
        .unwrap(),
        vec!["match-middle"]
    );
    assert!(
        read(
            &mut Cursor::new(bytes),
            bytes.len() as u64,
            None,
            usize::MAX,
            10
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        read(&mut Cursor::new(bytes), bytes.len() as u64, None, 0, 0)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn multibyte_lines_survive_reverse_block_boundaries() {
    let line = "αβγ".repeat(30000);
    let bytes = format!("old\n{line}\r\nlast").into_bytes();
    assert_eq!(
        read(&mut Cursor::new(&bytes), bytes.len() as u64, None, 0, 3).unwrap(),
        vec!["last".to_owned(), line, "old".to_owned()]
    );
}

#[test]
fn newest_page_stops_after_one_block() {
    struct Reader {
        file: Cursor<Vec<u8>>,
        reads: usize,
        bytes: usize,
    }
    impl Read for Reader {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.reads += 1;
            let count = self.file.read(bytes)?;
            self.bytes += count;
            Ok(count)
        }
    }
    impl Seek for Reader {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            self.file.seek(position)
        }
    }
    let mut bytes = vec![b'x'; 256 * 1024];
    bytes.extend_from_slice(b"\nlatest\n");
    let len = bytes.len() as u64;
    let mut reader = Reader {
        file: Cursor::new(bytes),
        reads: 0,
        bytes: 0,
    };
    assert_eq!(read(&mut reader, len, None, 0, 1).unwrap(), vec!["latest"]);
    assert_eq!(reader.reads, 1);
    assert_eq!(reader.bytes, 64 * 1024);
}

#[tokio::test]
async fn clear_preserves_an_append_handle_and_missing_file_stays_missing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("plain.log");
    let logs = Logs::new(directory.path());
    logs.set_path(path.clone()).unwrap();
    logs.clear().await.unwrap();
    assert!(!path.exists());
    let mut writer = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .unwrap();
    writer.write_all(b"old\n").unwrap();
    logs.clear().await.unwrap();
    writer.write_all(b"new\n").unwrap();
    assert_eq!(logs.read(String::new(), 0, 10).await.unwrap(), vec!["new"]);
    assert!(logs.set_path(PathBuf::from("relative.log")).is_err());
    assert_eq!(logs.path().unwrap(), path);
}

#[test]
fn missing_log_is_empty_but_read_failures_are_reported() {
    let directory = tempfile::tempdir().unwrap();
    assert!(
        read_lines_newest_first(&directory.path().join("missing.log"), None, 0, 10)
            .unwrap()
            .is_empty()
    );
    assert!(read_lines_newest_first(directory.path(), None, 0, 10).is_err());
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn native_main_uses_the_shared_default_path_without_host_requests() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
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
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
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

#[tokio::test]
async fn storage_management_counts_log_family_and_preserves_the_active_append_handle() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("plain.log");
    let backup = directory.path().join("plain.log.bak");
    let rotated = directory.path().join("plain.log.1");
    let unrelated = directory.path().join("unrelated.txt");
    std::fs::write(&path, b"live").unwrap();
    std::fs::write(&backup, b"backup").unwrap();
    std::fs::write(&rotated, b"old").unwrap();
    std::fs::write(&unrelated, b"preserved").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&unrelated, directory.path().join("plain.log.link")).unwrap();
    let rust_path = plain_rs::data_paths::DataPaths::new(directory.path()).rust_log_file();
    std::fs::create_dir_all(rust_path.parent().unwrap()).unwrap();
    std::fs::write(&rust_path, b"rust live").unwrap();
    let rust_backup = rust_path.with_extension("log.bak");
    let rust_rotated = rust_path.with_extension("log.1");
    std::fs::write(&rust_backup, b"rust backup").unwrap();
    std::fs::write(&rust_rotated, b"rust old").unwrap();
    let mut rust_writer = std::fs::OpenOptions::new()
        .append(true)
        .open(&rust_path)
        .unwrap();
    let logs = Logs::new(directory.path());
    logs.set_path(path.clone()).unwrap();
    assert_eq!(logs.size().await.unwrap(), 41);
    let mut writer = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    logs.clear().await.unwrap();
    assert!(path.is_file());
    assert!(!backup.exists());
    assert!(!rotated.exists());
    assert!(std::fs::read(&rust_path).unwrap().is_empty());
    assert!(!rust_backup.exists());
    assert!(!rust_rotated.exists());
    #[cfg(unix)]
    assert!(
        std::fs::symlink_metadata(directory.path().join("plain.log.link"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read(&unrelated).unwrap(), b"preserved");
    writer.write_all(b"after clear").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"after clear");
    rust_writer.write_all(b"rust after clear").unwrap();
    assert_eq!(std::fs::read(&rust_path).unwrap(), b"rust after clear");
    assert_eq!(logs.size().await.unwrap(), 27);
    logs.clear().await.unwrap();
    assert_eq!(logs.size().await.unwrap(), 0);
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn main_log_export_and_clear_cover_app_and_rust_log_families() {
    let directory = tempfile::tempdir().unwrap();
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
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
    let mut names = zip.file_names().map(|name| name.unwrap().into_owned()).collect::<Vec<_>>();
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

#[tokio::test]
async fn missing_log_family_reports_zero_and_exports_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let logs = Logs::new(directory.path());
    logs.set_path(directory.path().join("missing/latest.log"))
        .unwrap();
    assert_eq!(logs.size().await.unwrap(), 0);
    logs.clear().await.unwrap();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let (_stop, stopped) = tokio::sync::watch::channel(false);
        assert!(
            logs.export(
                directory.path(),
                &plain_rs::cache_paths::CachePaths::new(&directory.path().join("cache")),
                stopped
            )
            .await
            .unwrap()
            .is_none()
        );
        assert!(!directory.path().join("cache").exists());
    }
}

#[tokio::test]
async fn rust_logs_are_exported_without_app_logs_and_counted_once_for_a_shared_path() {
    let directory = tempfile::tempdir().unwrap();
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    paths.initialize_directories().unwrap();
    std::fs::write(paths.rust_log_file(), b"rust").unwrap();
    std::fs::write(paths.rust_log_file().with_extension("log.bak"), b"old").unwrap();
    let logs = Logs::new(directory.path());
    assert_eq!(logs.size().await.unwrap(), 7);
    let (_stop, stopped) = tokio::sync::watch::channel(false);
    let exported = logs
        .export(
            directory.path(),
            &plain_rs::cache_paths::CachePaths::new(&directory.path().join("cache")),
            stopped,
        )
        .await
        .unwrap()
        .unwrap();
    let zip = zip::ZipArchive::new(std::fs::File::open(exported).unwrap()).unwrap();
    let mut names = zip.file_names().map(|name| name.unwrap().into_owned()).collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, vec!["logs/rust.log", "logs/rust.log.bak"]);
    logs.set_path(paths.rust_log_file()).unwrap();
    assert_eq!(logs.size().await.unwrap(), 7);
    logs.clear().await.unwrap();
    assert_eq!(logs.size().await.unwrap(), 0);
    assert!(std::fs::read(paths.rust_log_file()).unwrap().is_empty());
    assert!(!paths.rust_log_file().with_extension("log.bak").exists());
}

#[tokio::test]
async fn clear_reports_log_directory_errors() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("logs"), b"not a directory").unwrap();
    let logs = Logs::new(directory.path());
    assert!(logs.clear().await.is_err());
}
