use crate::server::ContentServer;

#[tokio::test]
async fn desktop_v4_startup_migrates_published_desktop_chat_and_identity_once() {
    use plain_libs::rusqlite::{Connection, params};
    use plain_libs::serde_json::json;

    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    std::fs::create_dir(&data).unwrap();
    let old_database = data.join("local_chat.db");
    let legacy = Connection::open(&old_database).unwrap();
    legacy.execute_batch("CREATE TABLE chats (id TEXT PRIMARY KEY, from_id TEXT NOT NULL, to_id TEXT NOT NULL, channel_id TEXT NOT NULL, content TEXT NOT NULL, status TEXT NOT NULL, status_data TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
        CREATE TABLE chat_channels (id TEXT PRIMARY KEY, name TEXT NOT NULL, owner TEXT NOT NULL, members TEXT NOT NULL, key TEXT NOT NULL, version INTEGER NOT NULL, status TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
        CREATE TABLE peers (id TEXT PRIMARY KEY, name TEXT NOT NULL, ip TEXT NOT NULL, key TEXT NOT NULL, public_key TEXT NOT NULL, status TEXT NOT NULL, port INTEGER NOT NULL, device_type TEXT NOT NULL, token TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
        CREATE TABLE app_files (id TEXT PRIMARY KEY, size INTEGER NOT NULL, mime_type TEXT NOT NULL, real_path TEXT NOT NULL, ref_count INTEGER NOT NULL, weak_hash TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);").unwrap();
    let date = "2026-01-02T03:04:05Z";
    legacy
        .execute(
            "INSERT INTO chats VALUES (?1, 'me', 'local', '', ?2, 'SENT', '', ?3, ?3)",
            params![
                "00abcdef12345678",
                json!({"type":"text","value":{"text":"Offline chat from Desktop"}}).to_string(),
                date
            ],
        )
        .unwrap();
    drop(legacy);
    let raw_db = std::fs::read(&old_database).unwrap();
    let (pair, _) = plain_libs::crypto::ed25519_generate();
    let raw_prefs = json!({"client_id":"plain1","device_name":"Fixture","signature_key_pair":plain_libs::utils::base64::base64_encode(&pair)}).to_string().into_bytes();
    std::fs::write(data.join("prefs.json"), &raw_prefs).unwrap();

    for _ in 0..2 {
        let server = ContentServer::open_with_cache(&data, &cache).unwrap();
        let state = server.runtime_state();
        assert_eq!(
            state.native_client_id().unwrap(),
            "706c6169-0001-8000-8000-00005c3bb5dd"
        );
        assert_eq!(
            plain_rs::features::prefs::security::signing_keypair(&state.prefs).unwrap(),
            pair
        );
        let (count, payload): (i32, String) = state
            .db
            .with_conn(|conn| {
                conn.query_row("SELECT count(*), content FROM chats", [], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
            })
            .unwrap();
        assert_eq!(count, 1);
        assert!(payload.contains("Offline chat from Desktop"));
        assert_eq!(std::fs::read(&old_database).unwrap(), raw_db);
        assert_eq!(
            std::fs::read(data.join("prefs-v0.1.backup.json")).unwrap(),
            raw_prefs
        );
        server.shutdown().await;
        drop(state);
    }
}

#[tokio::test]
async fn desktop_v4_startup_keeps_legacy_inputs_when_chat_import_fails() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    std::fs::create_dir(&data).unwrap();
    let old_database = data.join("local_chat.db");
    let preferences = data.join("prefs.json");
    let raw_prefs = br#"{"client_id":"plain1","device_name":"Fixture"}"#;
    std::fs::write(&old_database, b"Corrupt legacy SQLite database").unwrap();
    std::fs::write(&preferences, raw_prefs).unwrap();

    assert!(ContentServer::open_with_cache(&data, &cache).is_err());
    assert_eq!(
        std::fs::read(&old_database).unwrap(),
        b"Corrupt legacy SQLite database"
    );
    assert_eq!(std::fs::read(&preferences).unwrap(), raw_prefs);
}

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
