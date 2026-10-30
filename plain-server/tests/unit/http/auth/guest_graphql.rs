use super::*;
use crate::assembly::server::ContentServer;
use plain_rs::prefs::Prefs;
use std::sync::Arc;
#[tokio::test]
async fn guest_executes_encrypted_contract_and_rejects_replay_and_revocation() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("Shared");
    std::fs::create_dir(&folder).unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs
        .set(
            "master_secret",
            plain_rs::utils::base64::base64_encode(&[11; 32]),
        )
        .unwrap();
    let server = ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[10; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let service = Service::new(state.db.clone(), state.prefs.clone());
    let share = service
        .create(
            "Test".into(),
            vec![folder.to_str().unwrap().into()],
            true,
            None,
        )
        .unwrap();
    let key =
        plain_rs::utils::base64::base64_decode_checked(&service.token(&share.id).unwrap()).unwrap();
    let wrap = |nonce: &str, query: &str| {
        plain_rs::crypto::xchacha_encrypt_raw(
            &key,
            format!("{}|{nonce}|{query}", chrono::Utc::now().timestamp_millis()).as_bytes(),
        )
        .unwrap()
    };
    let body = wrap(
        "a",
        r#"{"query":"query($path:String){ alias:sharedInfo(virtualPath:$path){name readOnly expiresAt entries{name virtualPath isDir size mimeType hasThumb}} }","variables":{"path":""}}"#,
    );
    let (status, encrypted) = execute(&state, &share.id, &body).await;
    assert_eq!(status, 200);
    let result: Value = serde_json::from_slice(
        &plain_rs::transport::http_response::Context::new("/guest_graphql", &body)
            .decrypt(&key, &encrypted)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["data"]["alias"]["name"], "Test");
    assert_eq!(
        result["data"]["alias"]["entries"][0]["virtualPath"],
        "Shared/"
    );
    assert_eq!(result["data"]["alias"]["entries"][0]["size"], 0);
    assert_eq!(execute(&state, &share.id, &body).await.0, 400);
    assert_eq!(execute(&state, &share.id, &[0; 40]).await.0, 401);
    assert_eq!(execute(&state, "", &body).await.0, 401);
    let malformed = wrap("b", "invalid");
    assert_eq!(execute(&state, &share.id, &malformed).await.0, 400);
    let forbidden = wrap("c", r#"{"query":"{ shareRecords { id } }"}"#);
    let (status, encrypted) = execute(&state, &share.id, &forbidden).await;
    assert_eq!(status, 200);
    let result: Value = serde_json::from_slice(
        &plain_rs::transport::http_response::Context::new("/guest_graphql", &forbidden)
            .decrypt(&key, &encrypted)
            .unwrap(),
    )
    .unwrap();
    assert!(result["errors"].is_array());
    state.db.share_delete(&share.id).unwrap();
    assert_eq!(execute(&state, &share.id, &body).await.0, 403);
    server.shutdown().await;
}

#[tokio::test]
async fn shared_file_rows_read_metadata_directly_without_host_dispatch() {
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().join("Shared");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("note.txt"), b"shared content").unwrap();
    std::fs::write(folder.join("cover.png"), b"image bytes").unwrap();
    let server = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = server.runtime_state();
    let shares = Service::new(state.db.clone(), state.prefs.clone());
    let share = shares
        .create(
            "Files".into(),
            vec![folder.to_string_lossy().into_owned()],
            true,
            None,
        )
        .unwrap();
    let (_generation, mut requests) = state.host.connect();
    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(Guest {
            state,
            id: share.id,
        })
        .finish();
    let result = schema
        .execute(
            "{ sharedInfo(virtualPath: \"Shared/\") { entries { name size mimeType hasThumb } } }",
        )
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    let rows = data["sharedInfo"]["entries"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let note = rows.iter().find(|row| row["name"] == "note.txt").unwrap();
    assert_eq!(note["size"], 14);
    assert_eq!(note["mimeType"], "text/plain");
    assert_eq!(note["hasThumb"], false);
    let cover = rows.iter().find(|row| row["name"] == "cover.png").unwrap();
    assert_eq!(cover["size"], 11);
    assert_eq!(cover["mimeType"], "image/png");
    assert_eq!(cover["hasThumb"], true);
    assert!(requests.try_recv().is_err());
    server.stop_core().await;
}

#[test]
fn guest_schema_matches_committed_sdl() {
    let actual = super::schema_builder().finish().sdl();
    if std::env::var_os("UPDATE_GRAPHQL_SCHEMAS").is_some() {
        std::fs::write(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../schema/guest-api.graphqls"),
            &actual,
        )
        .unwrap();
    } else {
        assert_eq!(
            actual,
            include_str!("../../../../../schema/guest-api.graphqls")
        );
    }
}
