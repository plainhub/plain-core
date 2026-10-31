use crate::server::ContentServer;
use serde_json::json;

fn request(report: &str) -> async_graphql::Request {
    async_graphql::Request::new("mutation($report:String!){createCrashReport(report:$report)}")
        .variables(async_graphql::Variables::from_json(
            json!({"report":report}),
        ))
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn native_report_uses_owned_cache_fixed_metadata_and_only_the_latest_200_log_lines() {
    let directory = tempfile::tempdir().unwrap();
    let external_cache = tempfile::tempdir().unwrap();
    let core = ContentServer::open_with_cache(directory.path(), external_cache.path()).unwrap();
    let state = core.runtime_state();
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    std::fs::write(paths.crash_report(), "pending fatal report").unwrap();
    std::fs::write(paths.crash_log(), "permanent crash history").unwrap();
    let log = directory.path().join("active.log");
    let lines = (0..205)
        .map(|index| format!("log-line-{index:03}\n"))
        .collect::<String>();
    std::fs::write(&log, &lines).unwrap();
    state.logs.set_path(log).unwrap();
    let (_generation, mut requests) = state.host.connect();
    let management = core.native_management().unwrap();
    for report in ["first report", "replacement report"] {
        let path = management
            .create_crash_report(report.to_owned())
            .await
            .unwrap();
        assert_eq!(
            std::path::Path::new(&path),
            core.cache_paths().crash_report_export()
        );
        let bytes = std::fs::read_to_string(core.cache_paths().crash_report_export()).unwrap();
        for field in [
            "App version: ",
            "OS: ",
            "Device: ",
            "Platform: ",
            "Build type: github",
        ] {
            assert!(bytes.contains(field), "{field}");
        }
        assert!(bytes.contains(&format!(
            "--- Crash Report ---\n{report}\n\n--- App Logs ---\nlog-line-005\n"
        )));
        assert!(bytes.ends_with("log-line-204"));
        assert!(!bytes.contains("log-line-004"));
        assert_eq!(
            std::fs::read_dir(core.cache_paths().root())
                .unwrap()
                .filter(
                    |entry| entry.as_ref().unwrap().path() != core.cache_paths().shared_previews()
                )
                .count(),
            1
        );
    }
    assert_eq!(
        std::fs::read_to_string(paths.crash_report()).unwrap(),
        "pending fatal report"
    );
    assert_eq!(
        std::fs::read_to_string(paths.crash_log()).unwrap(),
        "permanent crash history"
    );
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn empty_logs_do_not_add_a_section_and_remote_requests_cannot_create_a_report() {
    let directory = tempfile::tempdir().unwrap();
    let external_cache = tempfile::tempdir().unwrap();
    let core = ContentServer::open_with_cache(directory.path(), external_cache.path()).unwrap();
    let state = core.runtime_state();
    state
        .logs
        .set_path(directory.path().join("missing.log"))
        .unwrap();
    let _paths = plain_rs::data_paths::DataPaths::new(directory.path());
    let remote = crate::graphql::executor::execute_graphql(
        &state.public,
        request("remote report"),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!remote.errors.is_empty());
    assert!(!core.cache_paths().crash_report_export().exists());
    core.native_management()
        .unwrap()
        .create_crash_report("Unicode αβγ".into())
        .await
        .unwrap();
    let body = std::fs::read_to_string(core.cache_paths().crash_report_export()).unwrap();
    assert!(body.ends_with("Unicode αβγ"));
    assert!(!body.contains("--- App Logs ---"));
    core.stop_core().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn publication_failure_is_reported_and_cleans_its_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let external_cache = tempfile::tempdir().unwrap();
    let core = ContentServer::open_with_cache(directory.path(), external_cache.path()).unwrap();
    let state = core.runtime_state();
    state
        .logs
        .set_path(directory.path().join("missing.log"))
        .unwrap();
    let _paths = plain_rs::data_paths::DataPaths::new(directory.path());
    std::fs::create_dir_all(core.cache_paths().crash_report_export()).unwrap();
    assert!(
        core.native_management()
            .unwrap()
            .create_crash_report("cannot publish".into())
            .await
            .is_err()
    );
    assert!(core.cache_paths().crash_report_export().is_dir());
    assert_eq!(
        std::fs::read_dir(core.cache_paths().root())
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().path() != core.cache_paths().shared_previews())
            .count(),
        1
    );
    core.stop_core().await;
}
