use super::*;

#[tokio::test]
async fn init_requires_a_client_id_and_returns_forbidden_when_access_is_disabled() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let remote = ConnectInfo("127.0.0.1:12345".parse().unwrap());
    assert_eq!(
        init(State(state.clone()), remote, HeaderMap::new(), Bytes::new())
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    state.prefs.set("desktop_access", false).unwrap();
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
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let id = state.native_client_id().unwrap();
    assert!(state.db.session_get(&id).unwrap().is_none());
    state.prefs.set("password", "existing-password").unwrap();
    let key = plain_rs::utils::base64::base64_decode(&core.native_url_token().unwrap());
    let mut headers = HeaderMap::new();
    headers.insert("c-id", id.parse().unwrap());
    let response = init(
        State(state.clone()),
        ConnectInfo("127.0.0.1:1234".parse().unwrap()),
        headers,
        Bytes::from(plain_rs::crypto::xchacha_encrypt_raw(&key, b"init").unwrap()),
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
    assert_eq!(
        state.prefs.get::<String>("password").unwrap().as_deref(),
        Some("existing-password")
    );
    assert!(state.db.session_get(&id).unwrap().is_none());
    core.stop_core().await;
}

#[tokio::test]
async fn unauthenticated_init_never_returns_or_rotates_passwords_and_reports_explicit_state() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state.prefs.set("password", "preserved-password").unwrap();
    plain_rs::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
    for mode in ["FIXED", "NONE"] {
        state.prefs.set("password_type", mode).unwrap();
        let before = std::fs::read(directory.path().join("prefs.json")).unwrap();
        let mut events = state.events.subscribe();
        for body in [
            vec![],
            plain_rs::crypto::xchacha_encrypt_raw(&[9; 32], b"stale-token-probe").unwrap(),
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
            assert!(!payload.to_string().contains("preserved-password"));
            assert!(
                !payload
                    .to_string()
                    .contains(&plain_rs::crypto::sha512_hex("preserved-password"))
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
