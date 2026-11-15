use crate::server::ContentServer;

#[tokio::test]
async fn every_runtime_preserves_identity_and_keeps_fresh_native_credentials_out_of_sessions()
 {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let first = ContentServer::open(directory.path()).unwrap();
    let state = first.runtime_state();
    for table in [
        "chats",
        "chat_channels",
        "peers",
        "nearby_devices",
        "app_files",
        "tags",
        "audio_queue_items",
        "notes",
        "feeds",
    ] {
        assert!(
            !state.db.table_columns(table).is_empty(),
            "missing table {table}"
        );
    }
    let identity = plain_rs::features::prefs::ensure_identity(&state.prefs);
    let client_id = first.native_graphql().unwrap().client_id().to_owned();
    let token = first.native_url_token().unwrap();
    let pair = plain_rs::features::prefs::security::signing_keypair(&state.prefs).unwrap();
    assert_eq!(identity.client_id, client_id);
    assert_eq!(
        plain_libs::utils::base64::base64_decode(&identity.ed25519_keypair),
        pair
    );
    assert_eq!(token, state.token.as_ref());
    assert!(state.db.session_get(&client_id).unwrap().is_none());
    assert!(state.db.session_list().unwrap().is_empty());
    let response = first.native_graphql().unwrap().execute("{ sessions { clientId } }".into()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["sessions"], plain_libs::serde_json::json!([]));
    assert!(state.prefs.get::<String>("url_token").unwrap().is_none());
    assert!(state.prefs.get::<String>("urlToken").unwrap().is_none());
    assert_eq!(plain_libs::utils::base64::base64_decode(&token).len(), 32);
    first.shutdown().await;
    drop(state);
    let second = ContentServer::open(directory.path()).unwrap();
    assert_eq!(second.native_graphql().unwrap().client_id(), client_id);
    assert_ne!(second.native_url_token().unwrap(), token);
    assert!(second.runtime_state().db.session_list().unwrap().is_empty());
    assert_eq!(
        plain_rs::features::prefs::security::signing_keypair(second.preferences().unwrap())
            .unwrap(),
        pair
    );
    second.stop_core().await;
}

#[tokio::test]
async fn startup_supplied_token_authenticates_without_a_persisted_session() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = std::sync::Arc::new(plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let token = plain_libs::crypto::gen_token();
    let core = ContentServer::start_core(directory.path(), &token, prefs).unwrap();
    let state = core.runtime_state();
    let client_id = state.native_client_id().unwrap();
    assert!(state.db.session_list().unwrap().is_empty());
    assert_eq!(core.native_url_token().unwrap(), token);
    assert_eq!(plain_rs::features::services::sessions::local_key(&state, &client_id).unwrap(), plain_libs::utils::base64::base64_decode(&token));
    let mut headers = plain_libs::axum::http::HeaderMap::new();
    headers.insert("c-id", client_id.parse().unwrap());
    headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
    let response = crate::http::graphql::call(
        plain_libs::axum::extract::State(state.clone()), headers,
        plain_libs::axum::body::Bytes::from_static(br#"{"query":"{ sessions { clientId } app { urlToken } }"}"#),
    ).await;
    assert_eq!(response.status(), plain_libs::axum::http::StatusCode::OK);
    let bytes = plain_libs::axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: plain_libs::serde_json::Value = plain_libs::serde_json::from_slice(&bytes).unwrap();
    assert!(result.get("errors").is_none());
    assert_eq!(result["data"]["sessions"], plain_libs::serde_json::json!([]));
    assert_eq!(result["data"]["app"]["urlToken"], token);
    core.stop_core().await;
    assert!(plain_rs::features::services::sessions::local_key(&state, &client_id).is_none());
}
