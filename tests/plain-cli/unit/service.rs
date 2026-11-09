use super::*;
use plain_rs::features::access::AccessFeature;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn management_pairing_with_cli_requires_password_without_second_approval() {
    use plain_rs::features::services::discovery::pairing_attempts::{self, Mode, PairingPhase};
    use plain_rs::infrastructure::db::chat_store::peers;

    let root = plain_libs::temporary::tempdir().unwrap();
    let start = |name: &str| {
        let dir = root.path().join(name);
        async move {
            let paths = AppPaths {
                data_dir: dir.join("data"),
                cache_dir: dir.join("cache"),
                tls_cert: dir.join("tls.pem"),
                tls_key: dir.join("tls.key"),
            };
            std::fs::create_dir_all(&paths.data_dir).unwrap();
            let prefs = plain_rs::features::prefs::Prefs::load(
                &plain_rs::features::prefs::default_path(&paths.data_dir),
            )
            .unwrap();
            prefs.set("http_port", 0).unwrap();
            prefs.set("https_port", 0).unwrap();
            plain_rs::features::prefs::security::set_password_login(&prefs, "fixture password")
                .unwrap();
            Service::start(&paths, Options::default()).await.unwrap()
        }
    };
    let local = start("local").await;
    let remote = start("remote").await;
    let state = local.core.runtime();
    let remote_state = remote.core.runtime();
    let local_id = state.native_client_id().unwrap();
    let remote_id = remote_state.native_client_id().unwrap();
    let port = remote.core.http_ports().await.unwrap().0;

    for (password, expected) in [
        ("wrong password", PairingPhase::Failed),
        ("fixture password", PairingPhase::Succeeded),
    ] {
        let attempt = pairing_attempts::start(
            state,
            Some(remote_id.clone()),
            Some(format!("http://127.0.0.1:{port}")),
            Some(Mode::Manage),
            Some(password.into()),
            None,
            None,
        )
        .await
        .unwrap();
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let outcome = state
                    .pairing_runtime
                    .attempts
                    .get(&attempt.generation)
                    .unwrap();
                if matches!(
                    outcome.phase,
                    PairingPhase::Failed
                        | PairingPhase::Succeeded
                        | PairingPhase::WaitingConfirmation
                ) {
                    break outcome;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(outcome.phase, expected, "{:?}", outcome.error);
        let session = remote_state.db.session_get(&local_id).unwrap();
        let peer = peers::get(&state.db, &remote_id).unwrap();
        let remote_peer = peers::get(&remote_state.db, &local_id).unwrap();
        if expected == PairingPhase::Failed {
            assert_eq!(outcome.error.as_deref(), Some("invalid_password"));
            assert!(session.is_none());
            assert!(peer.is_none() && remote_peer.is_none());
            continue;
        }
        let peer = peer.unwrap();
        let remote_peer = remote_peer.unwrap();
        assert!(peer.is_paired() && remote_peer.is_paired());
        assert_eq!(peer.key, remote_peer.key);
        assert_eq!(peer.token, session.unwrap().token);
    }
    local.shutdown().await;
    remote.shutdown().await;
}

#[tokio::test]
async fn service_grants_remote_file_features_and_preserves_other_settings_across_restart() {
    let defaults = AccessFeature::FILE_FEATURES.as_slice();
    for (configured, expected_access) in [
        (vec![], defaults.to_vec()),
        (
            vec![AccessFeature::Sms],
            [[AccessFeature::Sms].as_slice(), defaults].concat(),
        ),
        (
            vec![AccessFeature::Files],
            [[AccessFeature::Files].as_slice(), &defaults[..4]].concat(),
        ),
    ] {
        let directory = plain_libs::temporary::tempdir().unwrap();
        let paths = AppPaths {
            data_dir: directory.path().join("data"),
            cache_dir: directory.path().join("cache"),
            tls_cert: directory.path().join("tls.pem"),
            tls_key: directory.path().join("tls.key"),
        };
        std::fs::create_dir(&paths.data_dir).unwrap();
        let prefs = plain_rs::features::prefs::Prefs::load(
            &plain_rs::features::prefs::default_path(&paths.data_dir),
        )
        .unwrap();
        prefs.set("http_port", 0).unwrap();
        prefs.set("https_port", 0).unwrap();
        prefs.set("api_permissions", &configured).unwrap();
        plain_rs::features::prefs::security::set_password_login(&prefs, "owned fixture password")
            .unwrap();
        let password = plain_rs::features::prefs::security::password_hash(&prefs).unwrap();
        let mut identity = None;
        for _ in 0..2 {
            let service = Service::start(
                &paths,
                Options {
                    file_index_defaults: Some(plain_rs::features::files::index_scope::Defaults {
                        excluded_directories: Vec::new(),
                        excluded_names: Vec::new(),
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
            let session = service
                .core
                .native_management()
                .unwrap()
                .create_session("Index card fixture".into())
                .unwrap();
            let port = service.core.http_ports().await.unwrap().0;
            let mut connection = tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .unwrap();
            let body = serde_json::json!({
                "query": "{ app { capabilities access } fileIndexVolumes { id } fileIndexDefaults { excludedDirs } scanProgress { state } audios(query:\"\",offset:0,limit:1,sortBy:NAME_ASC) { id } }"
            })
            .to_string();
            let request = format!(
                "POST /graphql HTTP/1.1\r\nHost: localhost\r\nC-ID: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                session.client_id,
                session.token,
                body.len(),
            );
            connection.write_all(request.as_bytes()).await.unwrap();
            let mut response = String::new();
            tokio::time::timeout(
                std::time::Duration::from_secs(3),
                connection.read_to_string(&mut response),
            )
            .await
            .unwrap()
            .unwrap();
            assert!(response.starts_with("HTTP/1.1 200"));
            let (_, body) = response.split_once("\r\n\r\n").unwrap();
            let result: serde_json::Value = serde_json::from_str(body).unwrap();
            assert!(result["errors"].is_null(), "{result}");
            let data = &result["data"];
            assert_eq!(data["app"]["access"], serde_json::json!(defaults));
            assert!(
                data["app"]["capabilities"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("MEDIA_SCAN"))
            );
            assert!(data["fileIndexVolumes"].is_array());
            assert!(data["fileIndexDefaults"]["excludedDirs"].is_array());
            assert!(data["scanProgress"]["state"].is_string());
            assert!(data["audios"].is_array());
            let prefs = service.core.shared_preferences().unwrap();
            assert_eq!(
                prefs
                    .get::<Vec<AccessFeature>>("api_permissions")
                    .unwrap()
                    .as_ref(),
                Some(&expected_access)
            );
            assert_eq!(
                plain_rs::features::prefs::security::password_hash(&prefs).unwrap(),
                password
            );
            let current = prefs.get::<String>("client_id").unwrap();
            if let Some(expected) = &identity {
                assert_eq!(&current, expected);
            }
            identity = Some(current);
            service.shutdown().await;
        }
    }
}

#[tokio::test]
async fn service_starts_real_listeners_and_releases_them_and_the_data_lock_on_shutdown_or_failure()
{
    let directory = plain_libs::temporary::Builder::new()
        .prefix("cli-service-")
        .tempdir_in(plain_rs::infrastructure::platform::system::consts::home_directory().unwrap())
        .unwrap();
    let paths = AppPaths {
        data_dir: directory.path().join("data"),
        cache_dir: directory.path().join("cache"),
        tls_cert: directory.path().join("tls.pem"),
        tls_key: directory.path().join("tls.key"),
    };
    std::fs::create_dir(&paths.data_dir).unwrap();
    let source = directory.path().join("sources");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("owned.txt"), b"owned command-line source").unwrap();
    let prefs = plain_rs::features::prefs::Prefs::load(&plain_rs::features::prefs::default_path(
        &paths.data_dir,
    ))
    .unwrap();
    prefs.set("http_port", 0).unwrap();
    prefs.set("https_port", 0).unwrap();
    let options = || Options {
        media_directories: vec![source.clone()],
        file_index_defaults: Some(plain_rs::features::files::index_scope::Defaults {
            excluded_directories: Vec::new(),
            excluded_names: Vec::new(),
        }),
        ..Default::default()
    };
    let service = Service::start(&paths, options()).await.unwrap();
    assert!(
        plain_rs::features::services::device::access::is_enabled(
            &service.core.shared_preferences().unwrap(),
            &[AccessFeature::Files]
        ),
        "CLI must initialize Web storage access without a mobile settings step"
    );
    let response = service
        .core
        .native_graphql()
        .unwrap()
        .execute("{app{access}}".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["app"]["access"],
        serde_json::json!(["IMAGES", "VIDEOS", "AUDIO", "DOCS", "FILES"])
    );
    let catalog = service.core.native_media_catalog().unwrap();
    assert_eq!(
        catalog.source_directories().unwrap(),
        vec![source.to_string_lossy().into_owned()]
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while service.core.runtime().media_watch.lock().unwrap().is_none() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(catalog.file_scan_status().unwrap().is_none());
    catalog.start_source_scan("/").unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while catalog
            .count(plain_rs::features::enums::DataType::Doc)
            .unwrap()
            != 1
        {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        std::fs::read(source.join("owned.txt")).unwrap(),
        b"owned command-line source"
    );
    let (http, https) = service.core.http_ports().await.unwrap();
    assert_ne!(http, 0);
    assert_ne!(https, 0);
    let mut connection = tokio::net::TcpStream::connect(("127.0.0.1", http))
        .await
        .unwrap();
    connection
        .write_all(
            b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        connection.read_to_end(&mut response),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        response.starts_with(b"HTTP/1.1 200"),
        "{}",
        String::from_utf8_lossy(&response)
    );
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    assert!(
        login::requests(&paths.data_dir, None)
            .await
            .unwrap()
            .requests
            .is_empty()
    );
    tokio::time::timeout(std::time::Duration::from_secs(10), service.shutdown())
        .await
        .unwrap();
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", http))
            .await
            .is_err()
    );
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", https))
            .await
            .is_err()
    );
    assert!(login::requests(&paths.data_dir, None).await.is_err());
    let mut invalid = options();
    invalid.web_root = Some(directory.path().join("missing-web"));
    assert!(Service::start(&paths, invalid).await.is_err());
    assert!(login::requests(&paths.data_dir, None).await.is_err());
    let reopened = ContentServer::open_with_cache(&paths.data_dir, &paths.cache_dir).unwrap();
    reopened.stop_core().await;
}
