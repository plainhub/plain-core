use super::*;
use crate::server::ContentServer;
use plain_libs::serde_json::Value;
use plain_rs::features::files::shares::Service;
use plain_rs::features::prefs::Prefs;
use std::sync::Arc;
#[tokio::test]
async fn guest_executes_encrypted_contract_and_rejects_replay_and_revocation() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let folder = dir.path().join("Shared");
    std::fs::create_dir(&folder).unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs
        .set(
            "master_secret",
            plain_libs::utils::base64::base64_encode(&[11; 32]),
        )
        .unwrap();
    let server = ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[10; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let service = Service::new(state.db.clone(), state.prefs.clone());
    let share = service
        .create(plain_rs::features::files::shares::Input {
            name: "Test".into(),
            password: String::new(),
            data: (vec![folder.to_str().unwrap().into()])
                .into_iter()
                .map(|path: String| {
                    let path = std::path::PathBuf::from(path);
                    plain_rs::features::files::shares::SharePath {
                        virtual_path: format!("/{}", path.file_name().unwrap().to_str().unwrap()),
                        real_path: path.to_str().unwrap().into(),
                        is_dir: path.is_dir(),
                    }
                })
                .collect(),
            read_only: true,
            expires_at: None,
        })
        .unwrap();
    let key = plain_libs::utils::base64::base64_decode_checked(&service.token(&share.id).unwrap())
        .unwrap();
    let wrap = |query: &str| {
        plain_rs::infrastructure::transport::http_request::encrypt(&key, "/guest_graphql", query)
            .unwrap()
    };
    let body = wrap(
        r#"{"query":"query($path:String){ alias:sharedInfo(relativePath:$path){name readOnly expiresAt entries{name relativePath isDir size mimeType hasThumb}} }","variables":{"path":""}}"#,
    );
    let (status, encrypted) = execute_guest(&state, &share.id, &body).await;
    assert_eq!(status, 200);
    let result: Value = plain_libs::serde_json::from_slice(
        &plain_rs::infrastructure::transport::http_response::Context::new(&body)
            .decrypt(&key, &encrypted)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["data"]["alias"]["name"], "Test");
    assert_eq!(
        result["data"]["alias"]["entries"][0]["relativePath"],
        "/Shared"
    );
    assert_eq!(result["data"]["alias"]["entries"][0]["size"], 0);
    assert_eq!(execute_guest(&state, &share.id, &body).await.0, 400);
    assert_eq!(execute_guest(&state, &share.id, &[0; 40]).await.0, 401);
    assert_eq!(execute_guest(&state, "", &body).await.0, 401);
    let malformed = plain_rs::infrastructure::transport::http_request::encrypt_raw(
        &key,
        "POST",
        "/guest_graphql",
        b"invalid",
    )
    .unwrap();
    assert_eq!(execute_guest(&state, &share.id, &malformed).await.0, 400);
    let forbidden = wrap(r#"{"query":"{ shareRecords { id } }"}"#);
    let (status, encrypted) = execute_guest(&state, &share.id, &forbidden).await;
    assert_eq!(status, 200);
    let result: Value = plain_libs::serde_json::from_slice(
        &plain_rs::infrastructure::transport::http_response::Context::new(&forbidden)
            .decrypt(&key, &encrypted)
            .unwrap(),
    )
    .unwrap();
    assert!(result["errors"].is_array());
    state.db.share_delete(&share.id).unwrap();
    assert_eq!(execute_guest(&state, &share.id, &body).await.0, 403);
    server.shutdown().await;
}
