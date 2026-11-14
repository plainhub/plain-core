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
fn developer_mode_commands_require_an_explicit_state() {
    for (state, enabled) in [("on", true), ("off", false)] {
        let cli = Cli::try_parse_from([
            "plainapp",
            "dev",
            state,
            "--data-dir",
            "data",
            "--cache-dir",
            "cache",
        ])
        .unwrap();
        assert!(matches!(cli.command, Cmd::Dev { enabled: value } if value == enabled));
        assert_eq!(cli.data_dir, Some(PathBuf::from("data")));
        assert_eq!(cli.cache_dir, Some(PathBuf::from("cache")));
    }
    for arguments in [vec!["dev"], vec!["dev", "true"], vec!["dev", "on", "off"]] {
        let error = Cli::try_parse_from(std::iter::once("plainapp").chain(arguments)).unwrap_err();
        assert_eq!(error.code, 2);
    }
    let error = Cli::try_parse_from(["plainapp", "dev"]).unwrap_err();
    assert!(error.message.contains("plainapp dev"));
    let help = Cli::try_parse_from(["plainapp", "dev", "--help"]).unwrap_err();
    assert_eq!(help.code, 0);
    assert!(help.message.contains("on"));
    assert!(help.message.contains("off"));
}
