use super::*;

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn app_and_remote_access_share_listeners_without_background_service() {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    prefs.set("background_service_enabled", false).unwrap();
    prefs.set("desktop_access", false).unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[19; 32]);
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let server = ContentServer::start_http(
        directory.path(),
        &token,
        prefs.clone(),
        crate::platform::startup::config(),
        0,
        0,
        cert.cert.pem().into_bytes(),
        cert.key_pair.serialize_pem().into_bytes(),
    )
    .await
    .unwrap();
    assert!(
        server.task.is_none(),
        "unified mode must not create a separate loopback listener"
    );
    let (http, https) = server.http_ports().await.unwrap();
    assert_eq!(http, server.port);
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    for url in [
        format!("http://127.0.0.1:{http}"),
        format!("https://127.0.0.1:{https}"),
    ] {
        let response = client
            .get(format!("{url}/health"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            client
                .post(format!("{url}/graphql"))
                .bearer_auth(&token)
                .header("content-type", "application/json")
                .body(serde_json::json!({"query":"{ __typename }"}).to_string())
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        assert_eq!(
            client
                .post(format!("{url}/graphql"))
                .header("content-type", "application/json")
                .body(serde_json::json!({"query":"{ __typename }"}).to_string())
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
        assert_eq!(
            client
                .get(format!("{url}/host"))
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    let router =
        crate::http::router::build_router(server.state.clone(), server.stop.subscribe());
    use tower::ServiceExt;
    for peer in [None, Some("192.0.2.42:1234"), Some("127.0.0.1:1234")] {
        let mut request = axum::http::Request::builder()
            .method("GET")
            .uri("/health")
            .header("authorization", format!("Bearer {token}"))
            .header("x-forwarded-for", "127.0.0.1")
            .body(axum::body::Body::empty())
            .unwrap();
        if let Some(peer) = peer {
            request.extensions_mut().insert(axum::extract::ConnectInfo(
                peer.parse::<std::net::SocketAddr>().unwrap(),
            ));
        }
        let expected = if peer == Some("127.0.0.1:1234") {
            200
        } else {
            403
        };
        assert_eq!(
            router.clone().oneshot(request).await.unwrap().status(),
            expected
        );
    }
    prefs.set("desktop_access", true).unwrap();
    assert_eq!(
        client
            .post(format!("http://127.0.0.1:{http}/graphql"))
            .header("content-type", "application/json")
            .body(serde_json::json!({"query":"{ __typename }"}).to_string())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn configured_public_uses_core_settings_and_keeps_listeners_on_tls_preflight_error() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let response = core
        .native_graphql()
        .unwrap()
        .execute(async_graphql::Request::new(
            "mutation{http:setPref(key:\"http_port\",value:0) https:setPref(key:\"https_port\",value:0)}",
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        core.runtime_state()
            .prefs
            .get::<i32>("http_port")
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        core.runtime_state()
            .prefs
            .get::<i32>("https_port")
            .unwrap(),
        Some(0)
    );
    let ports = core.start_configured_public().await.unwrap();
    assert_ne!(ports.0, 0);
    assert_ne!(ports.1, 0);
    assert_eq!(core.http_ports().await, Some(ports));
    assert!(core.task.is_none());
    core.state
        .prefs
        .set("rotate_url_token_on_restart", true)
        .unwrap();
    let preferences = core.state.prefs.entries();
    {
        let state = core.runtime_state();
        let _shutdown = state.shutdown.lock().await;
        state.restart_public_locked().await.unwrap();
    }
    assert_eq!(core.state.prefs.entries(), preferences);
    let ports = core.http_ports().await.unwrap();
    std::fs::write(directory.path().join("tls-identity.json"), "{}").unwrap();
    {
        let state = core.runtime_state();
        let _shutdown = state.shutdown.lock().await;
        assert!(state.restart_public_locked().await.is_err());
    }
    assert_eq!(core.http_ports().await, Some(ports));
    core.stop_core().await;
    assert_eq!(core.http_ports().await, None);
    assert!(core.native_url_token().is_err());
    assert!(core.native_graphql().is_err());
    assert!(core.start_configured_public().await.is_err());
    {
        let state = core.runtime_state();
        let _shutdown = state.shutdown.lock().await;
        assert!(state.restart_public_locked().await.is_err());
    }
}

#[tokio::test]
async fn core_startup_persists_complete_identity_and_only_rotates_url_token_on_recreation() {
    use plain_rs::prefs::pref_keys::*;
    let directory = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let core = ContentServer::open_with_cache(directory.path(), cache.path()).unwrap();
    let prefs = core.state.prefs.clone();
    let first = prefs.entries();
    for key in [
        CLIENT_ID,
        SIGNATURE_KEY_PAIR,
        MASTER_SECRET,
        PASSWORD,
        ADB_TOKEN,
        MDNS_HOSTNAME,
        URL_TOKEN,
    ] {
        assert!(
            !prefs.get::<String>(key).unwrap().unwrap().is_empty(),
            "{key}"
        );
    }
    assert_eq!(prefs.get::<String>(PASSWORD_TYPE).unwrap().as_deref(), Some("NONE"));
    assert_eq!(prefs.get::<bool>(LOGIN_REQUIRE_APPROVAL).unwrap(), Some(true));
    let persisted = plain_rs::prefs::Prefs::load(prefs.path()).unwrap();
    assert_eq!(persisted.entries(), first);
    core.stop_core().await;
    drop(core);
    let core = ContentServer::open_with_cache(directory.path(), cache.path()).unwrap();
    assert_eq!(core.state.prefs.entries(), first);
    core.state
        .prefs
        .set(ROTATE_URL_TOKEN_ON_RESTART, true)
        .unwrap();
    core.state.prefs.set(PASSWORD, "custom-password").unwrap();
    core.state.prefs.set(LOGIN_REQUIRE_APPROVAL, false).unwrap();
    let before: std::collections::BTreeMap<_, _> = core.state.prefs.entries().into_iter().collect();
    core.stop_core().await;
    drop(core);
    let core = ContentServer::open_with_cache(directory.path(), cache.path()).unwrap();
    assert_ne!(
        core.state.prefs.get::<String>(URL_TOKEN).unwrap(),
        persisted.get::<String>(URL_TOKEN).unwrap()
    );
    let mut after: std::collections::BTreeMap<_, _> =
        core.state.prefs.entries().into_iter().collect();
    after.insert(URL_TOKEN.into(), before[URL_TOKEN].clone());
    assert_eq!(after, before);
    core.stop_core().await;
}

#[tokio::test]
async fn core_startup_propagates_system_initialization_failure_without_publishing_partial_secrets()
{
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("prefs.json");
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&path).unwrap());
    prefs.set("dev_token", "existing").unwrap();
    let before = prefs.entries();
    let blocker = path.with_extension("json.tmp");
    std::fs::create_dir(&blocker).unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[21; 32]);
    assert!(ContentServer::start_core(directory.path(), &token, prefs.clone()).is_err());
    assert_eq!(prefs.entries(), before);
    assert_eq!(
        plain_rs::prefs::Prefs::load(&path).unwrap().entries(),
        before
    );
    std::fs::remove_dir(blocker).unwrap();
    let core = ContentServer::start_core(directory.path(), &token, prefs).unwrap();
    assert!(
        core.state
            .prefs
            .get::<String>("master_secret")
            .unwrap()
            .is_some()
    );
    core.stop_core().await;
}

#[cfg(feature = "core")]
#[tokio::test]
async fn shell_preferences_share_the_core_owner() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let shell = core.shared_preferences().unwrap();
    assert!(Arc::ptr_eq(&shell, &core.state.prefs));
    shell.set("device_name", "Shared device").unwrap();
    assert_eq!(
        core.preferences()
            .unwrap()
            .get::<String>("device_name")
            .unwrap(),
        Some("Shared device".into())
    );
    core.stop_core().await;
    assert!(core.shared_preferences().is_err());
}

#[tokio::test]
async fn every_transport_build_serves_the_main_guest_and_peer_contracts_without_private_routes() {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    prefs.set("desktop_access", true).unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[3; 32]);
    let server = ContentServer::start(directory.path(), &token, prefs).unwrap();
    let client = reqwest::Client::new();
    let document = serde_json::json!({"query":"query Main { __typename audioQueueItems(query: \"\", offset: 0, limit: 10) { mediaId } }","operationName":"Main"});
    let native = server
        .graphql_executor()
        .execute(serde_json::from_value(document.clone()).unwrap())
        .await;
    assert!(native.errors.is_empty(), "{:?}", native.errors);
    let http = client
        .post(format!("http://127.0.0.1:{}/graphql", server.port))
        .bearer_auth(&token)
        .header("content-type", "application/json")
        .body(document.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(http.status(), 200);
    let body: serde_json::Value = serde_json::from_slice(&http.bytes().await.unwrap()).unwrap();
    assert_eq!(body, serde_json::to_value(native).unwrap());
    for path in ["/guest_graphql", "/peer_graphql"] {
        let response = client
            .post(format!("http://127.0.0.1:{}{path}", server.port))
            .body(document.to_string())
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            401,
            "{path} must reach its own authentication gate"
        );
        assert_eq!(
            response.headers()["content-type"],
            "application/octet-stream"
        );
    }
    for path in [
        "/host",
        "/host/graphql",
        "/hostGraphql",
        "/host_graphql",
        "/private/graphql",
        "/private_graphql",
        "/content_graphql",
        "/system/tls",
        "/system/provider-plan",
        "/providers/media-buckets",
        "/images/models",
        "/images/search",
        "/chat/mdns",
        "/chat/prewarm",
        "/chat/store",
        "/chat/download",
        "/chat/transport",
        "/shares/batch",
        "/shares/client",
        "/shares/client/file",
        "/shares/client/zip",
    ] {
        let response = client
            .post(format!("http://127.0.0.1:{}{path}", server.port))
            .bearer_auth(&token)
            .body(document.to_string())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404, "obsolete endpoint {path}");
    }
    server.stop_core().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn startup_device_name_initialization_is_shared_and_preserves_user_changes_without_host_calls()
 {
    for (initial, expected) in [
        (None, "Startup Name"),
        (Some(" \t"), "Startup Name"),
        (Some("Chosen Name"), "Chosen Name"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let prefs = Arc::new(
            plain_rs::prefs::Prefs::load(
                &plain_rs::data_paths::DataPaths::new(directory.path()).prefs(),
            )
            .unwrap(),
        );
        if let Some(initial) = initial {
            prefs.set("device_name", initial).unwrap();
        }
        let config = crate::host::startup::StartupConfig {
            name: "Startup Name".into(),
            ..crate::platform::startup::config()
        };
        let token = plain_rs::utils::base64::base64_encode(&[7; 32]);
        let (state, stop, _stopped) = ContentServer::build_state_with_config(
            directory.path(),
            &token,
            prefs.clone(),
            config,
        )
        .unwrap();
        assert_eq!(
            prefs.get::<String>("device_name").unwrap().as_deref(),
            Some(expected)
        );
        let (_generation, mut requests) = state.host.connect();
        for name in [expected, "Updated Name"] {
            prefs.set("device_name", name).unwrap();
            let response = crate::graphql::executor::execute_graphql(
                &state.public,
                async_graphql::Request::new("{app{deviceName}}"),
                crate::graphql::executor::RequestContext::local(state.clone()),
            )
            .await;
            assert!(response.errors.is_empty(), "{:?}", response.errors);
            assert_eq!(
                response.data.into_json().unwrap()["app"]["deviceName"],
                name
            );
        }
        assert!(requests.try_recv().is_err());
        stop.send(true).unwrap();
    }
}

#[tokio::test]
async fn embedded_web_assets_survive_cache_clear_without_extraction() {
    use crate::http::web_bundle::{SharedWebBundle, WebBundle};
    struct Bundle;
    impl WebBundle for Bundle {
        fn read(&self, path: &str) -> Option<&'static [u8]> {
            match path {
                "index.html" => Some(b"<html><head></head><body>embedded</body></html>"),
                "assets/app.js" => Some(b"console.log('embedded')"),
                _ => None,
            }
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    let server = ContentServer::open_with_cache(&data, &cache).unwrap();
    let state = server.runtime_state();
    let compiled = state.web_bundle.read().unwrap().clone().unwrap();
    use crate::http::web_bundle::EmbeddedBundle;
    assert_eq!(
        compiled.read("index.html"),
        EmbeddedBundle.read("index.html")
    );
    let bundle: SharedWebBundle = Arc::new(Bundle);
    server.set_web_bundle(bundle.clone());
    for iteration in 0..2 {
        if iteration == 1 {
            let cleared = server
                .native_graphql()
                .unwrap()
                .execute(async_graphql::Request::new("mutation { clearAppCache }"))
                .await;
            assert!(cleared.errors.is_empty(), "{:?}", cleared.errors);
            assert_eq!(cleared.data.into_json().unwrap()["clearAppCache"], true);
        }
        for (path, expected) in [
            ("/", "embedded"),
            ("/settings", "embedded"),
            ("/assets/app.js", "console.log('embedded')"),
        ] {
            let response = crate::http::static_files::fallback(
                axum::extract::State(server.runtime_state()),
                axum::http::Request::builder()
                    .uri(path)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await;
            assert_eq!(response.status(), axum::http::StatusCode::OK);
            let body = axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap();
            let text = std::str::from_utf8(&body).unwrap();
            assert!(text.contains(expected));
            if path != "/assets/app.js" {
                assert!(text.contains("window.__SERVER_TIME__="));
            }
        }
        assert!(state.web_root.read().unwrap().is_none());
        assert!(Arc::ptr_eq(
            state.web_bundle.read().unwrap().as_ref().unwrap(),
            &bundle
        ));
        assert!(!cache.join("web_assets").exists());
        assert!(!data.join("web-assets").exists());
    }
    server.stop_core().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn app_certificate_fingerprint_describes_the_active_listener() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let api = core.native_graphql().unwrap();
    let read = || async {
        let response = api.execute(async_graphql::Request::new("{app{tlsCertificateSha256}}")).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        response.data.into_json().unwrap()["app"]["tlsCertificateSha256"].clone()
    };
    assert!(read().await.is_null());
    core.state.prefs.set("http_port", 0).unwrap();
    core.state.prefs.set("https_port", 0).unwrap();
    core.start_configured_public().await.unwrap();
    let (_, _, cert, _) = core.state.http_configuration().await.unwrap();
    let expected = crate::services::tls_identity::certificate_sha256(&cert).unwrap();
    assert_eq!(read().await, expected);
    std::fs::write(directory.path().join("tls-identity.json"), "{}").unwrap();
    assert_eq!(read().await, expected);
    core.stop_public().await;
    assert!(read().await.is_null());
    core.stop_core().await;
}
