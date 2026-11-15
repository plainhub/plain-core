use super::*;

#[test]
fn cli_accepts_supported_commands() {
    for name in ["install", "uninstall", "passwd", "update", "run"] {
        assert!(Cli::try_parse_from(["plainapp", name]).is_ok(), "{name}");
    }
}

#[test]
fn installation_and_uninstallation_options_are_preserved() {
    assert!(Cli::try_parse_from(["plainapp", "install", "--with-libreoffice"]).is_err());
    assert!(matches!(
        Cli::try_parse_from(["plainapp", "uninstall", "--keep-config", "--keep-data"])
            .unwrap()
            .command,
        Cmd::Uninstall {
            keep_config: true,
            keep_data: true
        }
    ));
}

#[test]
fn general_run_accepts_multiple_media_directories_and_global_paths() {
    let cli = Cli::try_parse_from([
        "plainapp",
        "run",
        "--data-dir",
        "data",
        "--cache-dir",
        "cache",
        "--media-dir",
        "photos",
        "--media-dir",
        "documents",
        "--web-root",
        "web",
    ])
    .unwrap();
    let paths = cli.paths().unwrap();
    assert!(paths.data_dir.is_absolute());
    assert!(paths.data_dir.ends_with("data"));
    assert!(paths.cache_dir.ends_with("cache"));
    assert!(paths.tls_cert.ends_with("data/tls.pem"));
    assert!(paths.tls_key.ends_with("data/tls.key"));
    match cli.command {
        Cmd::Run {
            media_directories,
            web_root,
        } => {
            assert_eq!(
                media_directories,
                [PathBuf::from("photos"), PathBuf::from("documents")]
            );
            assert_eq!(web_root, Some(PathBuf::from("web")));
        }
        _ => panic!("run command required"),
    }
}

#[test]
fn password_input_can_use_standard_input() {
    assert!(matches!(
        Cli::try_parse_from(["plainapp", "passwd", "--stdin"])
            .unwrap()
            .command,
        Cmd::Passwd { stdin: true }
    ));
}

#[test]
fn preferences_command_requires_key_and_value() {
    let cli = Cli::try_parse_from([
        "plainapp",
        "prefs",
        "developer_mode",
        "true",
        "--data-dir",
        "data",
    ])
    .unwrap();
    assert!(
        matches!(cli.command, Cmd::Prefs { ref key, ref value } if key == "developer_mode" && value == "true")
    );
    assert_eq!(cli.data_dir, Some(PathBuf::from("data")));
    assert_eq!(cli.cache_dir, None);
    assert!(
        Cli::try_parse_from([
            "plainapp",
            "prefs",
            "developer_mode",
            "true",
            "--cache-dir",
            "cache"
        ])
        .is_err()
    );
    for arguments in [
        vec!["prefs"],
        vec!["prefs", "developer_mode"],
        vec!["prefs", "developer_mode", "true", "extra"],
        vec!["dev", "on"],
    ] {
        assert!(Cli::try_parse_from(std::iter::once("plainapp").chain(arguments)).is_err());
    }
    let help = Cli::try_parse_from(["plainapp", "prefs", "--help"]).unwrap_err();
    assert_eq!(help.code, 0);
    assert!(help.message.contains("<KEY> <VALUE>"));
}

#[test]
fn commands_share_default_and_overridden_data_directories() {
    const FIXTURE: &str = "PLAINAPP_CLI_PATH_TEST_HOME";
    if let Some(home) = std::env::var_os(FIXTURE) {
        let home = PathBuf::from(home);
        for arguments in [
            vec!["plainapp", "run"],
            vec!["plainapp", "prefs", "allow_any_host", "true"],
            vec!["plainapp", "passwd", "--stdin"],
        ] {
            let cli = Cli::try_parse_from(arguments.clone()).unwrap();
            let paths = cli.paths().unwrap();
            let expected = if cfg!(target_os = "linux") {
                PathBuf::from("/var/lib/plainapp")
            } else {
                home.join(".plainapp/data")
            };
            let expected = std::env::var_os("PLAINAPP_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(expected);
            assert_eq!(paths.data_dir, expected);
            assert_eq!(paths.tls_cert, expected.join("tls.pem"));
            let explicit = home.join("explicit-data");
            let cli =
                Cli::try_parse_from(arguments.into_iter().map(std::ffi::OsString::from).chain([
                    std::ffi::OsString::from("--data-dir"),
                    explicit.clone().into_os_string(),
                ]))
                .unwrap();
            assert_eq!(cli.paths().unwrap().data_dir, explicit);
        }
        return;
    }
    let directory = plain_libs::temporary::tempdir().unwrap();
    let home = directory.path().join("generated-home");
    std::fs::create_dir(&home).unwrap();
    for configured in [None, Some(home.join("configured-data"))] {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "cli::tests::commands_share_default_and_overridden_data_directories",
                "--nocapture",
            ])
            .env(FIXTURE, &home)
            .env("HOME", &home)
            .env("LOCALAPPDATA", &home)
            .env("PLAINAPP_CACHE_DIR", directory.path().join("cache"))
            .env_remove("PLAINAPP_DATA_DIR")
            .env_remove("PLAINAPP_TLS_CERT")
            .env_remove("PLAINAPP_TLS_KEY");
        if let Some(configured) = configured {
            command.env("PLAINAPP_DATA_DIR", configured);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(!home.join(".plainapp/data").exists());
}
