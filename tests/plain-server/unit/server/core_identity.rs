use crate::server::ContentServer;

#[tokio::test]
async fn every_runtime_uses_the_current_schema_and_the_same_persisted_identity_keys_and_a_fresh_native_session()
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
    let session = state.db.session_get(&client_id).unwrap().unwrap();
    assert_eq!(session.url_token, token);
    assert_eq!(session.token, state.token.as_ref());
    assert_ne!(session.url_token, session.token);
    assert!(state.prefs.get::<String>("url_token").unwrap().is_none());
    assert!(state.prefs.get::<String>("urlToken").unwrap().is_none());
    assert_eq!(plain_libs::utils::base64::base64_decode(&token).len(), 32);
    first.shutdown().await;
    drop(state);
    let second = ContentServer::open(directory.path()).unwrap();
    assert_eq!(second.native_graphql().unwrap().client_id(), client_id);
    assert_ne!(second.native_url_token().unwrap(), token);
    assert_eq!(
        plain_rs::features::prefs::security::signing_keypair(second.preferences().unwrap())
            .unwrap(),
        pair
    );
    second.stop_core().await;
}
