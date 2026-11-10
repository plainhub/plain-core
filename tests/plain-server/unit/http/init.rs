use super::*;

#[tokio::test]
async fn fixed_password_without_a_secret_never_advertises_remote_setup() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state.prefs.set("password_type", "FIXED").unwrap();
    state.prefs.remove("password").unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("c-id", "first-browser".parse().unwrap());
    let response = init(
        State(state),
        ConnectInfo("127.0.0.1:1234".parse().unwrap()),
        headers,
        Bytes::new(),
    )
    .await;
    let payload: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 2048)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(payload["passwordRequired"], true);
    assert!(payload.get("needsSetup").is_none());
    assert!(payload.get("setupHttpsPort").is_none());
    core.stop_core().await;
}

#[tokio::test]
async fn init_requires_a_client_id_and_returns_forbidden_when_access_is_disabled() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let remote = ConnectInfo("127.0.0.1:12345".parse().unwrap());
    assert_eq!(
        init(State(state.clone()), remote, HeaderMap::new(), Bytes::new())
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    state.prefs.set("remote_access", false).unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("c-id", "web-client".parse().unwrap());
    assert_eq!(
        init(State(state), remote, headers, Bytes::new())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    core.stop_core().await;
}

#[tokio::test]
async fn native_init_uses_the_owned_url_key_without_creating_a_web_session_or_replacing_the_password()
 {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let id = state.native_client_id().unwrap();
    assert!(state.db.session_get(&id).unwrap().is_none());
    plain_rs::features::prefs::security::set_entry(
        &state.prefs,
        "password",
        serde_json::json!("existing-password"),
    )
    .unwrap();
    let key = plain_libs::utils::base64::base64_decode(&core.native_url_token().unwrap());
    let mut headers = HeaderMap::new();
    headers.insert("c-id", id.parse().unwrap());
    let response = init(
        State(state.clone()),
        ConnectInfo("127.0.0.1:1234".parse().unwrap()),
        headers,
        Bytes::from(
            plain_rs::infrastructure::transport::http_request::encrypt(&key, "/init", "init")
                .unwrap(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 2048)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(payload.get("password").is_none());
    assert_eq!(payload["authenticated"], true);
    assert_eq!(payload["passwordRequired"], false);
    assert!(!payload["signaturePublicKey"].as_str().unwrap().is_empty());
    assert!(payload.get("needsSetup").is_none());
    assert!(
        plain_rs::features::prefs::security::password_credential(&state.prefs)
            .unwrap()
            .is_some()
    );
    assert!(payload.get("opaquePublicKey").is_none());
    assert!(state.db.session_get(&id).unwrap().is_none());
    core.stop_core().await;
}

#[tokio::test]
async fn unauthenticated_init_never_returns_or_rotates_passwords_and_reports_explicit_state() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    plain_rs::features::prefs::security::set_entry(
        &state.prefs,
        "password",
        serde_json::json!("preserved-password"),
    )
    .unwrap();
    plain_rs::features::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
    for mode in ["FIXED", "NONE"] {
        state.prefs.set("password_type", mode).unwrap();
        let before = std::fs::read(directory.path().join("prefs.json")).unwrap();
        let mut events = state.events.subscribe();
        for body in [
            vec![],
            plain_rs::infrastructure::transport::http_request::encrypt(&[9; 32], "/init", "init")
                .unwrap(),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert("c-id", "browser".parse().unwrap());
            let response = init(
                State(state.clone()),
                ConnectInfo("127.0.0.1:1234".parse().unwrap()),
                headers,
                Bytes::from(body),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let payload: serde_json::Value = serde_json::from_slice(
                &axum::body::to_bytes(response.into_body(), 2048)
                    .await
                    .unwrap(),
            )
            .unwrap();
            assert!(payload.get("password").is_none());
            assert_eq!(payload["authenticated"], false);
            assert_eq!(payload["passwordRequired"], mode == "FIXED");
            if mode == "FIXED" {
                assert_eq!(
                    payload["opaquePublicKey"],
                    plain_rs::features::prefs::security::opaque_public_key(&state.prefs)
                        .unwrap()
                        .unwrap()
                );
                assert!(payload.get("setup").is_none());
                assert!(payload.get("record").is_none());
            } else {
                assert!(payload.get("opaquePublicKey").is_none());
            }
            assert!(!payload.to_string().contains("preserved-password"));
            assert!(
                !payload
                    .to_string()
                    .contains(&plain_libs::crypto::sha512_hex("preserved-password"))
            );
        }
        assert_eq!(
            std::fs::read(directory.path().join("prefs.json")).unwrap(),
            before
        );
        assert!(events.try_recv().is_err());
        assert!(state.db.session_get("browser").unwrap().is_none());
    }
    core.stop_core().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn removed_password_setup_route_cannot_initialize_or_replace_a_password() {
    use plain_libs::protocols::http_client;
    use plain_rs::features::prefs::security;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    security::configure_password_login(&state.prefs).unwrap();
    let (certificate, key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let client = http_client::builder()
        .add_root_certificate(http_client::Certificate::from_pem(&certificate).unwrap())
        .build()
        .unwrap();
    let (http, https) = core.start_public(0, 0, certificate, key).await.unwrap();
    for configured in [false, true] {
        if configured {
            security::set_password_login(&state.prefs, "installed-password").unwrap();
        }
        let before = state.prefs.entries();
        for (scheme, port) in [("http", http), ("https", https)] {
            let response = client.post(format!("{scheme}://localhost:{port}/auth/setup"))
                .header("Content-Type", "application/json")
                .body(serde_json::to_vec(&json!({"passwordHash": plain_libs::crypto::sha512_hex("unwanted-password")})).unwrap())
                .send().await.unwrap();
            assert_eq!(response.status().as_u16(), 404);
            assert_eq!(state.prefs.entries(), before);
        }
    }
    core.stop_core().await;
}
