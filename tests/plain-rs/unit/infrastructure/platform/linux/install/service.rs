use super::*;

#[test]
fn install_updates_existing_unit_to_the_actual_binary() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("system/plainapp.service");
    write_service(&path, "/opt/other/plainapp").unwrap();
    write_service(&path, BINARY_PATH).unwrap();
    let unit = std::fs::read_to_string(&path).unwrap();
    assert!(
        unit.contains("[Service]\nType=simple\nUser=root\nExecStart=/usr/local/bin/plainapp run\n")
    );
    assert!(!unit.contains("/opt/other/plainapp"));
    assert!(unit.contains("Restart=on-failure"));
}

#[test]
fn service_write_failure_is_returned() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let file = directory.path().join("file");
    std::fs::write(&file, "bytes").unwrap();
    assert!(write_service(&file.join("unit"), BINARY_PATH).is_err());
    assert_eq!(std::fs::read_to_string(file).unwrap(), "bytes");
}

#[test]
fn uninstall_can_keep_configuration_and_tls_without_keeping_application_data() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path();
    let paths = DataPaths::new(data);
    let certificate = data.join("tls/custom.pem");
    let private_key = data.join("tls/custom.key");
    fs::create_dir(data.join("tls")).unwrap();
    for path in [
        paths.lock(),
        paths.prefs(),
        paths.tls_identity(),
        paths.database(),
        certificate.clone(),
        private_key.clone(),
        data.join("tls/unused"),
    ] {
        fs::write(path, "fixture").unwrap();
    }
    let keep = [
        &paths.lock(),
        &paths.prefs(),
        &paths.tls_identity(),
        &certificate,
        &private_key,
    ];
    remove_data_except(data, &keep.map(|path| path.as_path())).unwrap();
    assert!(keep.into_iter().all(|path| path.is_file()));
    assert!(!paths.database().exists());
    assert!(!data.join("tls/unused").exists());
}

#[test]
fn restart_moves_the_control_command_to_a_collected_scope_before_restarting() {
    let directory = plain_libs::temporary::tempdir().unwrap();
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
    let directory = plain_libs::temporary::tempdir().unwrap();
    let mut command = std::process::Command::new(directory.path().join("missing-systemctl"));
    assert!(restart_service(&mut command).is_err());
}
