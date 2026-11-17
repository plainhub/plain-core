use super::*;

#[tokio::test]
async fn password_command_writes_shared_system_credentials() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    set_password(
        directory.path(),
        &directory.path().join("cache"),
        "example-password",
    )
    .await
    .unwrap();
    let prefs = plain_rs::features::prefs::Prefs::load(&plain_rs::features::prefs::default_path(
        directory.path(),
    ))
    .unwrap();
    assert!(
        plain_rs::features::prefs::security::opaque_public_key(&prefs)
            .unwrap()
            .is_some()
    );
    let stored =
        std::fs::read_to_string(plain_rs::features::prefs::default_path(directory.path())).unwrap();
    assert!(!stored.contains("example-password"));
    assert_eq!(
        plain_rs::features::prefs::security::password_type(&prefs).unwrap(),
        plain_rs::features::prefs::security::PasswordType::Fixed
    );
    assert!(!prefs.get_or("login_require_approval", true));
}

fn authenticate(prefs: &plain_rs::features::prefs::Prefs, password: &str) -> bool {
    let Ok(Some(credential)) = plain_rs::features::prefs::security::password_credential(prefs)
    else {
        return false;
    };
    let identity = plain_libs::utils::base64::base64_decode(
        &prefs.get::<String>("signature_key_pair").unwrap().unwrap(),
    );
    let setup = plain_libs::utils::base64::base64_decode(&credential.setup);
    let record = plain_libs::utils::base64::base64_decode(&credential.record);
    let Ok(server_key) = plain_libs::auth::server::public_key(&setup) else {
        return false;
    };
    let client =
        plain_libs::auth::client::OpaqueLogin::start(password.as_bytes().to_vec()).unwrap();
    let context = plain_libs::auth::context("prefs-test", &identity[32..]);
    let Ok((server, response)) = plain_libs::auth::server::start(
        &setup,
        &record,
        &client.request(),
        &context,
        &identity[32..],
    ) else {
        return false;
    };
    let Ok(result) = client.finish(&response, "prefs-test", &identity[32..], &server_key) else {
        return false;
    };
    let Ok(session_key) = server.finish(&result.ke3) else {
        return false;
    };
    result.api_key.as_slice() == plain_libs::auth::api_key(&session_key, &context)
}

#[tokio::test]
async fn running_password_updates_use_exact_values_and_preserve_identity_and_other_instances() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let data = root.path().join("data");
    let cache = root.path().join("cache");
    let other = plain_server::server::ContentServer::open(&root.path().join("other")).unwrap();
    let core = plain_server::server::ContentServer::open_with_cache(&data, &cache).unwrap();
    let listener = crate::management_socket::Service::start(&core)
        .await
        .unwrap()
        .unwrap();
    let identity = core
        .runtime()
        .prefs
        .get::<String>("signature_key_pair")
        .unwrap();
    let mut events = core.runtime().events.subscribe();
    let exact = "  exact π password!  ";
    set_password(&data, &cache, exact).await.unwrap();
    assert!(authenticate(&core.runtime().prefs, exact));
    assert!(!authenticate(&core.runtime().prefs, exact.trim()));
    assert_eq!(events.try_recv().unwrap().event_type, "PREFS_UPDATED");
    let cli = crate::cli::Cli::try_parse_from([
        "plainapp",
        "prefs",
        "password",
        "\"replacement fixture\"",
        "--data-dir",
        data.to_str().unwrap(),
    ])
    .unwrap();
    crate::cli::prefs::run(&cli.paths().unwrap(), "password", "\"replacement fixture\"")
        .await
        .unwrap();
    assert!(authenticate(&core.runtime().prefs, "replacement fixture"));
    assert!(!authenticate(&core.runtime().prefs, exact));
    let before = std::fs::read(plain_rs::features::prefs::default_path(&data)).unwrap();
    for invalid in ["", "  ", &"x".repeat(1025)] {
        let error = set_password(&data, &cache, invalid)
            .await
            .unwrap_err()
            .to_string();
        assert!(!error.contains(&"x".repeat(1025)));
        assert_eq!(
            std::fs::read(plain_rs::features::prefs::default_path(&data)).unwrap(),
            before
        );
    }
    assert_eq!(
        core.runtime()
            .prefs
            .get::<String>("signature_key_pair")
            .unwrap(),
        identity
    );
    assert!(
        plain_rs::features::prefs::security::password_credential(&other.runtime().prefs)
            .unwrap()
            .is_none()
    );
    let stored = String::from_utf8(before).unwrap();
    assert!(!stored.contains(exact));
    assert!(!stored.contains("replacement fixture"));
    core.stop_core().await;
    listener.shutdown().await.unwrap();
    other.stop_core().await;
}

#[tokio::test]
async fn unsafe_socket_and_unsupported_replies_never_fall_back_to_configuration_writes() {
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let root = plain_libs::temporary::tempdir().unwrap();
    let data = root.path().join("data");
    let cache = root.path().join("cache");
    let core = plain_server::server::ContentServer::open_with_cache(&data, &cache).unwrap();
    let path = plain_rs::features::prefs::default_path(&data);
    let before = std::fs::read(&path).unwrap();
    let directory = plain_libs::platform::local_socket::SocketDirectory::open(&data).unwrap();
    let socket_path = directory.socket_path();
    let socket = tokio::net::UnixListener::bind(&socket_path).unwrap();
    let file = directory.protect_socket().unwrap();
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o666)).unwrap();
    let error = set_password(&data, &cache, "not applied")
        .await
        .unwrap_err();
    assert!(!crate::management_socket::not_running(&error));
    assert!(
        crate::cli::prefs::run(
            &crate::cli::Cli::try_parse_from([
                "plainapp",
                "prefs",
                "developer_mode",
                "true",
                "--data-dir",
                data.to_str().unwrap()
            ])
            .unwrap()
            .paths()
            .unwrap(),
            "developer_mode",
            "true"
        )
        .await
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let server = tokio::spawn(async move {
        let (mut connection, _) = socket.accept().await.unwrap();
        let length = connection.read_u32().await.unwrap();
        let mut payload = vec![0; length as usize];
        connection.read_exact(&mut payload).await.unwrap();
        let reply = br#"{"version":9,"result":{"type":"UPDATED"}}"#;
        connection.write_u32(reply.len() as u32).await.unwrap();
        connection.write_all(reply).await.unwrap();
    });
    let error = set_password(&data, &cache, "not applied")
        .await
        .unwrap_err();
    assert!(error.to_string().contains("version"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    server.await.unwrap();
    drop(file);
    drop(directory);
    core.stop_core().await;
}
