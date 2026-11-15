use super::*;
use crate::cli::{Cli, Cmd};

#[test]
fn status_uses_the_existing_directory_selection_and_rejects_unused_options() {
    let cli = Cli::try_parse_from(["plainapp", "status", "--data-dir", "selected"]).unwrap();
    assert!(matches!(cli.command, Cmd::Status));
    assert!(cli.paths().unwrap().data_dir.ends_with("selected"));
    for arguments in [
        vec!["status", "extra"],
        vec!["status", "--cache-dir", "cache"],
    ] {
        assert!(Cli::try_parse_from(std::iter::once("plainapp").chain(arguments)).is_err());
    }
    let help = Cli::try_parse_from(["plainapp", "help", "status"]).unwrap_err();
    assert_eq!(help.code, 0);
    assert!(help.message.contains("plainapp status"));
}

#[test]
fn status_displays_bound_ports_and_listener_failures_separately_from_process_state() {
    let mut bytes = Vec::new();
    let mut status = Status {
        pid: 42,
        listeners: ListenerState::Running,
        http_port: 18080,
        https_port: 18443,
        error: String::new(),
    };
    display(&mut bytes, &status).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("Service: running\nPID: 42\nHTTP server: running"));
    assert!(text.contains("http: 18080 (http://127.0.0.1:18080)"));
    assert!(text.contains("https: 18443 (https://127.0.0.1:18443)"));
    for (state, label) in [
        (ListenerState::Restarting, "restarting"),
        (ListenerState::Stopped, "stopped"),
        (ListenerState::Failed, "failed"),
    ] {
        status.listeners = state;
        status.http_port = 0;
        status.https_port = 0;
        status.error = "fixture listener error".into();
        let mut bytes = Vec::new();
        display(&mut bytes, &status).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("Service: running"));
        assert!(text.contains(&format!("HTTP server: {label}")));
        assert!(text.contains("http: not listening"));
        assert!(text.contains("https: not listening"));
        assert!(text.contains("Error: fixture listener error"));
        assert!(!text.contains("://"));
    }
}

#[tokio::test]
async fn absent_status_is_read_only_and_authentication_errors_are_not_stopped_status() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let data = root.path().join("absent");
    let cli =
        Cli::try_parse_from(["plainapp", "status", "--data-dir", data.to_str().unwrap()]).unwrap();
    run(&cli.paths().unwrap()).await.unwrap();
    assert!(!data.exists());
    assert!(!management_socket::not_running(
        &std::io::Error::from(std::io::ErrorKind::PermissionDenied).into()
    ));
    assert!(!management_socket::not_running(
        &plain_libs::anyhow::anyhow!("Invalid local management reply")
    ));
}
