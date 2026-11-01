use super::*;

#[test]
fn install_updates_existing_unit_to_the_actual_binary() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("system/plainapp.service");
    write_service(&path, "/opt/other/plainapp").unwrap();
    write_service(&path, BINARY_PATH).unwrap();
    let unit = std::fs::read_to_string(&path).unwrap();
    assert!(unit.contains("ExecStart=/usr/local/bin/plainapp run\n"));
    assert!(!unit.contains("/opt/other/plainapp"));
    assert!(unit.contains("Restart=on-failure"));
}

#[test]
fn service_write_failure_is_returned() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("file");
    std::fs::write(&file, "bytes").unwrap();
    assert!(write_service(&file.join("unit"), BINARY_PATH).is_err());
    assert_eq!(std::fs::read_to_string(file).unwrap(), "bytes");
}

#[test]
fn restart_moves_the_control_command_to_a_collected_scope_before_restarting() {
    let directory = tempfile::tempdir().unwrap();
    let script = directory.path().join("systemd-run");
    let arguments = directory.path().join("arguments");
    std::fs::write(
        &script,
        "output=$1\nshift\nprintf '%s\\n' \"$@\" > \"$output\"\n[ \"$1\" = '--scope' ]\n",
    )
    .unwrap();
    let mut command = std::process::Command::new("/bin/sh");
    command.arg(script).arg(&arguments);
    restart_service(&mut command).unwrap();
    assert_eq!(
        std::fs::read_to_string(arguments).unwrap(),
        "--scope\n--quiet\n--collect\n--no-ask-password\n--\nsystemctl\n--no-block\nrestart\nplainapp.service\n"
    );
}

#[test]
fn restart_returns_a_rejected_job_as_an_error() {
    let mut command = std::process::Command::new("/bin/sh");
    command.args(["-c", "exit 17"]);
    assert_eq!(
        restart_service(&mut command).unwrap_err().to_string(),
        "service restart failed"
    );
}

#[test]
fn restart_returns_scheduler_spawn_failure() {
    let directory = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new(directory.path().join("missing-systemctl"));
    assert!(restart_service(&mut command).is_err());
}
