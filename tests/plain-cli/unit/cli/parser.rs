use super::*;

#[test]
fn previous_parser_command_option_and_help_results_are_preserved() {
    let cases: &[(&[&str], i32, Option<&str>, Option<&str>)] = include!("reference_cases.rs");
    assert_eq!(cases.len(), 564);
    for (arguments, code, parsed, help) in cases {
        let result =
            Cli::try_parse_from(std::iter::once("plainapp").chain(arguments.iter().copied()));
        match result {
            Ok(cli) => {
                assert_eq!(*code, 0, "{arguments:?}");
                assert_eq!(
                    Some(format!("{cli:?}")).as_deref(),
                    *parsed,
                    "{arguments:?}"
                );
            }
            Err(error) => {
                assert_eq!(error.code, *code, "{arguments:?}: {}", error.message);
                assert!(parsed.is_none(), "{arguments:?}");
                if let Some(expected) = help {
                    assert_eq!(error.message, *expected, "{arguments:?}");
                }
            }
        }
    }
}

#[test]
fn option_values_and_positionals_have_distinct_boundaries() {
    for arguments in [
        vec!["run", "--data-dir="],
        vec!["run", "--media-dir="],
        vec!["run", "--web-root="],
        vec!["run", "--automount=true"],
        vec!["run", "--automount", "--automount"],
        vec!["--", "run"],
        vec!["login", "allow", "one", "two"],
        vec!["login", "allow", "-request"],
        vec!["login", "requests", "--after", "--help"],
    ] {
        assert_eq!(
            Cli::try_parse_from(std::iter::once("plainapp").chain(arguments))
                .unwrap_err()
                .code,
            2
        );
    }
    let cli = Cli::try_parse_from(["plainapp", "run", "--data-dir", "-"]).unwrap();
    assert_eq!(cli.data_dir.unwrap(), std::path::PathBuf::from("-"));
    assert!(
        matches!(Cli::try_parse_from(["plainapp", "login", "allow", "--", "-request"]).unwrap().command,
        Cmd::Login { command: login::Command::Allow { request_id } } if request_id == "-request")
    );
    assert!(
        matches!(Cli::try_parse_from(["plainapp", "login", "allow", ""]).unwrap().command,
        Cmd::Login { command: login::Command::Allow { request_id } } if request_id.is_empty())
    );
}

#[cfg(unix)]
#[test]
fn native_paths_preserve_non_utf8_bytes_but_text_arguments_reject_them() {
    use std::os::unix::ffi::OsStringExt;
    let path = OsString::from_vec(b"folder-\xff".to_vec());
    let cli = Cli::try_parse_from([
        OsString::from("plainapp"),
        "run".into(),
        "--data-dir".into(),
        path.clone(),
    ])
    .unwrap();
    assert_eq!(cli.data_dir.unwrap().as_os_str(), path);
    let attached = OsString::from_vec(b"--media-dir=folder-\xff".to_vec());
    assert!(
        matches!(Cli::try_parse_from([OsString::from("plainapp"), "run".into(), attached]).unwrap().command,
        Cmd::Run { media_directories, .. } if media_directories[0].as_os_str() == path)
    );
    for prefix in [vec!["login", "allow"], vec!["login", "requests", "--after"]] {
        let args = std::iter::once(OsString::from("plainapp"))
            .chain(prefix.into_iter().map(OsString::from))
            .chain([path.clone()]);
        assert_eq!(Cli::try_parse_from(args).unwrap_err().code, 2);
    }
}

#[cfg(windows)]
#[test]
fn native_paths_preserve_unpaired_utf16_surrogates() {
    use std::os::windows::ffi::OsStringExt;
    let path = OsString::from_wide(&[b'x' as u16, 0xd800]);
    let cli = Cli::try_parse_from([
        OsString::from("plainapp"),
        "run".into(),
        "--data-dir".into(),
        path.clone(),
    ])
    .unwrap();
    assert_eq!(cli.data_dir.unwrap().as_os_str(), path);
    let mut attached = OsString::from("--media-dir=");
    attached.push(&path);
    assert!(matches!(Cli::try_parse_from([
        OsString::from("plainapp"), "run".into(), attached,
    ]).unwrap().command, Cmd::Run { media_directories, .. } if media_directories[0].as_os_str() == path));
}
