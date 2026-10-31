use super::*;
use crate::cli::{Cli, Cmd};
use plain_server::native::management::NativeWebLoginRequest;

#[test]
fn login_commands_keep_the_explicit_instance_and_decision() {
    for (name, allowed) in [("allow", true), ("deny", false)] {
        let cli = Cli::try_parse_from([
            "plainapp",
            "--data-dir",
            "fixture",
            "login",
            name,
            "request",
        ])
        .unwrap();
        assert_eq!(cli.data_dir.unwrap(), std::path::PathBuf::from("fixture"));
        match cli.command {
            Cmd::Login {
                command: Command::Allow { request_id },
            } if allowed => assert_eq!(request_id, "request"),
            Cmd::Login {
                command: Command::Deny { request_id },
            } if !allowed => assert_eq!(request_id, "request"),
            _ => panic!("explicit login decision required"),
        }
    }
    assert!(
        matches!(Cli::try_parse_from(["plainapp", "login", "requests", "--after", "cursor"]).unwrap().command,
        Cmd::Login { command: Command::Requests { after: Some(cursor) } } if cursor == "cursor")
    );
    assert!(Cli::try_parse_from(["plainapp", "login", "allow"]).is_err());
}

#[test]
fn untrusted_request_fields_cannot_inject_terminal_commands() {
    let malicious = "browser\x1b[2J\r\npretend approval".to_string();
    let page = NativeWebLoginRequests {
        requests: vec![NativeWebLoginRequest {
            request_id: "request".into(),
            client_id: malicious.clone(),
            client_ip: malicious.clone(),
            browser_name: malicious.clone(),
            browser_version: malicious.clone(),
            os_name: malicious.clone(),
            os_version: malicious.clone(),
            verification_code: Some(malicious.clone()),
        }],
        next_after: Some(malicious),
    };
    let mut output = Vec::new();
    display_requests(&mut output, &page).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains('\x1b'));
    assert!(!output.contains('\r'));
    assert!(output.contains("\\u{1b}[2J\\r\\npretend approval"));
    assert!(output.contains("plainapp login deny <request-id>"));
}
