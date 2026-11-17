use super::*;
use plain_rs::features::access::AccessFeature;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

static MDNS_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn management_pairing_with_cli_requires_password_without_second_approval() {
    let _mdns = MDNS_TEST_LOCK.lock().await;
    use plain_rs::features::services::discovery::pairing_attempts::{self, Mode};
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
    state
        .nearby_devices
        .observe(plain_rs::features::discovery::nearby_devices::Device {
            id: remote_id.clone(),
            name: "Fixture".into(),
            ips: vec!["127.0.0.1".into()],
            port: remote.core.http_ports().await.unwrap().1,
            remote_access: Some(true),
            signature_public_key: plain_libs::utils::base64::base64_encode(
                &plain_rs::features::services::peers::peer_wire::signing_keypair(
                    &remote_state.prefs,
                )
                .unwrap()[32..],
            ),
            device_type: plain_rs::features::chat::enums::DeviceType::Computer,
            version: "1".into(),
            platform: "fixture".into(),
            last_seen_at: plain_rs::infrastructure::db::now_iso(),
            discovery_methods: vec![
                plain_rs::features::discovery::nearby_devices::DiscoveryMethod::Lan,
            ],
        })
        .unwrap();
    for (password, expected) in [
        ("wrong password", "PAIRING_FAILED"),
        ("fixture password", "PAIRING_COMPLETED"),
    ] {
        let mut events = state.events.subscribe();
        assert!(
            pairing_attempts::start(
                state,
                remote_id.clone(),
                Mode::Manage,
                Some(password.into()),
                None
            )
            .await
            .unwrap()
        );
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while let Ok(event) = events.recv().await {
                if matches!(event.event_type, "PAIRING_COMPLETED" | "PAIRING_FAILED") {
                    return event;
                }
            }
            panic!("pairing notifications closed")
        })
        .await
        .unwrap();
        assert_eq!(outcome.event_type, expected, "{}", outcome.payload);
        let session = remote_state.db.session_get(&local_id).unwrap();
        let peer = peers::get(&state.db, &remote_id).unwrap();
        let remote_peer = peers::get(&remote_state.db, &local_id).unwrap();
        if expected == "PAIRING_FAILED" {
            assert_eq!(
                plain_libs::serde_json::from_str::<plain_libs::serde_json::Value>(&outcome.payload)
                    .unwrap()["error"],
                "INVALID_PASSWORD"
            );
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
    let _mdns = MDNS_TEST_LOCK.lock().await;
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
        let password = plain_rs::features::prefs::security::password_credential(&prefs).unwrap();
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
            let body = plain_libs::serde_json::json!({
                "query": "{ app { capabilities access } fileIndexVolumes { id } fileIndexDefaults { excludedDirs } fileIndexScanProgress { state } audios(query:\"\",offset:0,limit:1,sortBy:TITLE_ASC) { id } }"
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
            let result: plain_libs::serde_json::Value =
                plain_libs::serde_json::from_str(body).unwrap();
            assert!(result["errors"].is_null(), "{result}");
            let data = &result["data"];
            assert_eq!(
                data["app"]["access"],
                plain_libs::serde_json::json!(defaults)
            );
            assert!(
                data["app"]["capabilities"]
                    .as_array()
                    .unwrap()
                    .contains(&plain_libs::serde_json::json!("MEDIA_SCAN"))
            );
            assert!(data["fileIndexVolumes"].is_array());
            assert!(data["fileIndexDefaults"]["excludedDirs"].is_array());
            assert!(data["fileIndexScanProgress"]["state"].is_string());
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
                plain_rs::features::prefs::security::password_credential(&prefs).unwrap(),
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
    let _mdns = MDNS_TEST_LOCK.lock().await;
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
    plain_rs::features::prefs::security::set_password_login(&prefs, "fixture password").unwrap();
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
        plain_libs::serde_json::json!(["IMAGES", "VIDEOS", "AUDIO", "DOCS", "FILES"])
    );
    let catalog = service.core.native_media_catalog().unwrap();
    assert_eq!(
        catalog.source_directories().unwrap(),
        vec![source.to_string_lossy().into_owned()]
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while service
            .core
            .runtime()
            .media_watch
            .lock()
            .unwrap()
            .is_empty()
        {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(catalog.file_scan_status().unwrap().is_none());
    catalog.start_volume_scan(None).unwrap().unwrap();
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
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
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
        management_socket::requests(&paths.data_dir, None)
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
    assert!(
        management_socket::requests(&paths.data_dir, None)
            .await
            .is_err()
    );
    let mut invalid = options();
    invalid.web_root = Some(directory.path().join("missing-web"));
    assert!(Service::start(&paths, invalid).await.is_err());
    assert!(
        management_socket::requests(&paths.data_dir, None)
            .await
            .is_err()
    );
    let reopened = ContentServer::open_with_cache(&paths.data_dir, &paths.cache_dir).unwrap();
    reopened.stop_core().await;
}

#[tokio::test]
async fn unconfigured_service_fails_before_listeners_or_passwordless_access() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let paths = AppPaths {
        data_dir: root.path().join("data"),
        cache_dir: root.path().join("cache"),
        tls_cert: root.path().join("tls.pem"),
        tls_key: root.path().join("tls.key"),
    };
    let error = match Service::start(&paths, Options::default()).await {
        Ok(service) => {
            service.shutdown().await;
            panic!("Unconfigured service started");
        }
        Err(error) => error,
    };
    assert!(error.to_string().contains("plainapp passwd"));
    assert!(!paths.tls_cert.exists());
    assert!(!paths.tls_key.exists());
    let prefs = plain_rs::features::prefs::Prefs::load(&plain_rs::features::prefs::default_path(
        &paths.data_dir,
    ))
    .unwrap();
    assert!(
        plain_rs::features::prefs::security::password_credential(&prefs)
            .unwrap()
            .is_none()
    );
    assert!(
        management_socket::requests(&paths.data_dir, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn web_http_server_card_reads_state_and_changes_the_serving_port() {
    let _mdns = MDNS_TEST_LOCK.lock().await;
    use plain_libs::protocols::http_client;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let paths = AppPaths {
        data_dir: directory.path().join("data"),
        cache_dir: directory.path().join("cache"),
        tls_cert: directory.path().join("tls.pem"),
        tls_key: directory.path().join("tls.key"),
    };
    std::fs::create_dir(&paths.data_dir).unwrap();
    let prefs = plain_rs::features::prefs::Prefs::load(&plain_rs::features::prefs::default_path(
        &paths.data_dir,
    ))
    .unwrap();
    prefs.set("http_port", 0).unwrap();
    prefs.set("https_port", 0).unwrap();
    plain_rs::features::prefs::security::set_password_login(&prefs, "owned web fixture").unwrap();
    let service = Service::start(&paths, Options::default()).await.unwrap();
    let prefs = service.core.shared_preferences().unwrap();
    let original_ports = service.core.http_ports().await.unwrap();
    let session = service
        .core
        .native_management()
        .unwrap()
        .create_session("HTTP card fixture".into())
        .unwrap();
    let client = http_client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    let port = i32::from(service.core.http_ports().await.unwrap().0);
    let request = |port, query: &'static str| {
        client
            .post(format!("http://127.0.0.1:{port}/graphql"))
            .header("C-ID", &session.client_id)
            .bearer_auth(&session.token)
            .header("Content-Type", "application/json")
            .body(plain_libs::serde_json::json!({"query":query}).to_string())
    };
    let response: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&request(port, "{ app { capabilities } httpServer { state { revision restarting httpPort httpsPort error } addresses onlineClients } }")
        .send().await.unwrap().text().await.unwrap()).unwrap();
    assert!(response["errors"].is_null(), "{response}");
    assert!(
        response["data"]["app"]["capabilities"]
            .as_array()
            .unwrap()
            .contains(&plain_libs::serde_json::json!(Capability::HttpServer))
    );
    assert_eq!(response["data"]["httpServer"]["state"]["httpPort"], port);
    assert!(response["data"]["httpServer"]["addresses"].is_array());
    assert!(response["data"]["httpServer"]["onlineClients"].is_array());
    let denied: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(
        &request(
            port,
            "mutation { setPref(key:\"remote_access\",value:false) }",
        )
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap(),
    )
    .unwrap();
    assert!(denied["errors"].is_array(), "{denied}");
    assert!(prefs.get_or("remote_access", true));
    let renamed: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(
        &request(
            port,
            "mutation { setPref(key:\"mdns_hostname\",value:\"fixture-http-card.local\") }",
        )
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap(),
    )
    .unwrap();
    assert!(renamed["errors"].is_null(), "{renamed}");
    assert_eq!(
        prefs.get_or("mdns_hostname", String::new()),
        "fixture-http-card.local"
    );
    for (operation, query) in [
        (
            "acquireMdnsDebugSession",
            r#"mutation { acquireMdnsDebugSession(token:"00000000-0000-4000-8000-000000000001") }"#,
        ),
        (
            "releaseMdnsDebugSession",
            r#"mutation { releaseMdnsDebugSession(token:"00000000-0000-4000-8000-000000000001") }"#,
        ),
    ] {
        let response: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(
            &request(port, query)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(response["errors"].is_null(), "{response}");
        assert_eq!(response["data"][operation], true);
    }
    for invalid in [
        "mutation { restartHttpServer(httpPort:-1) { revision } }",
        "mutation { restartHttpServer(httpsPort:65536) { revision } }",
    ] {
        let response: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(
            &request(port, invalid)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(response["errors"].is_array(), "{response}");
        assert_eq!(service.core.runtime().http_server.snapshot().revision.0, 0);
    }
    let response: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(
        &request(
            port,
            "mutation { restartHttpServer(httpPort:0) { revision restarting } }",
        )
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap(),
    )
    .unwrap();
    assert!(response["errors"].is_null(), "{response}");
    assert_eq!(response["data"]["restartHttpServer"]["restarting"], false);
    let state = service.core.runtime().http_server.snapshot();
    let completed = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        service
            .core
            .runtime()
            .http_server
            .wait_finished(state.revision),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(completed.error.is_empty(), "{}", completed.error);
    assert_eq!(completed.https_port, i32::from(original_ports.1));
    let response: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(
        &request(
            completed.http_port,
            "{httpServer{state{revision restarting httpPort httpsPort error}}}",
        )
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap(),
    )
    .unwrap();
    assert!(response["errors"].is_null(), "{response}");
    assert_eq!(
        response["data"]["httpServer"]["state"]["httpPort"],
        completed.http_port
    );
    let unauthenticated = client
        .post(format!("http://127.0.0.1:{}/graphql", completed.http_port))
        .header("Content-Type", "application/json")
        .body("{\"query\":\"{httpServer{remoteAccess}}\"}")
        .send()
        .await
        .unwrap();
    assert_eq!(unauthenticated.status().as_u16(), 401);
    let tls_response = client.post(format!("https://127.0.0.1:{}/graphql", completed.https_port))
        .header("C-ID", &session.client_id).bearer_auth(&session.token)
        .header("Content-Type", "application/json")
        .body(plain_libs::serde_json::json!({"query":"mutation { restartHttpServer(httpsPort:0) { restarting httpPort httpsPort error } }"}).to_string())
        .send().await.unwrap().text().await.unwrap();
    let tls_response: plain_libs::serde_json::Value =
        plain_libs::serde_json::from_str(&tls_response).unwrap();
    assert!(tls_response["errors"].is_null(), "{tls_response}");
    assert_eq!(
        tls_response["data"]["restartHttpServer"]["restarting"],
        false
    );
    assert_eq!(
        tls_response["data"]["restartHttpServer"]["httpPort"],
        completed.http_port
    );
    assert_eq!(tls_response["data"]["restartHttpServer"]["error"], "");
    let https_port = tls_response["data"]["restartHttpServer"]["httpsPort"]
        .as_i64()
        .unwrap();
    assert_eq!(
        client
            .get(format!("https://127.0.0.1:{https_port}/health"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "OK"
    );
    service.shutdown().await;
}
