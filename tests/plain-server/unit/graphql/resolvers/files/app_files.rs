use crate::server::ContentServer;
use crate::test_support::graphql_http as call;
use plain_libs::protocols::http_client;
use std::sync::Arc;

#[tokio::test]
async fn attachment_upload_dedupe_release_and_restart_use_core_storage() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[14; 32]);
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs
        .set("api_permissions", plain_libs::serde_json::json!(["FILES"]))
        .unwrap();
    let path = dir.path().join("plain.db");
    let server = ContentServer::start(path.parent().unwrap(), &token, prefs.clone()).unwrap();
    let file = upload_attachment(&server, &token, b"shared attachment").await;
    let id = file["id"].as_str().unwrap();
    let relative = file["realPath"].as_str().unwrap();
    assert!(std::path::Path::new(relative).is_absolute());
    assert!(std::path::Path::new(relative).starts_with(dir.path().join("files")));
    let stored = server.runtime_state().db.app_file_get(id).unwrap().unwrap();
    assert!(!std::path::Path::new(&stored.real_path).is_absolute());
    assert_eq!(file["size"], 17);
    let second = upload_attachment(&server, &token, b"shared attachment").await;
    assert_eq!(second["id"], id);
    let page = call(server.port, &token, &format!(
        r#"{{appFiles(offset:0,limit:20,query:"text:{id}") {{id}} appFileCount(query:"text:{id}")}}"#,
    )).await;
    assert_eq!(page["data"]["appFileCount"], 1);
    assert_eq!(page["data"]["appFiles"][0]["id"], id);
    let encoded = plain_rs::features::files::resource_link::seal(
        &plain_rs::features::files::resource_link::Scope::Session(
            server.runtime_state().native_client_id().unwrap(),
        ),
        &plain_rs::features::sessions::token_key(
            &server.runtime_state().native_url_token().unwrap(),
        )
        .unwrap(),
        &plain_rs::features::files::resource_link::Resource {
            r#type: plain_rs::features::files::resource_link::ResourceType::AppFile,
            id: id.to_owned(),
        },
    )
    .unwrap();
    let client = http_client::builder().build().unwrap();
    let url = format!("http://127.0.0.1:{}/fs", server.port);
    let response = client
        .get(&url)
        .query(&[("id", encoded.as_str()), ("action", "download")])
        .bearer_auth(&token)
        .header("range", "bytes=2-7")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 206);
    assert_eq!(response.headers()["content-range"], "bytes 2-7/17");
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"ared a");
    let response = client
        .get(&url)
        .query(&[("id", encoded.as_str()), ("action", "download")])
        .bearer_auth(&token)
        .header("range", "bytes=100-")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 416);
    server.shutdown().await;
    let server = ContentServer::start(path.parent().unwrap(), &token, prefs).unwrap();
    let release = format!(r#"mutation {{releaseAppFile(fileId:"{id}")}}"#);
    assert_eq!(
        call(server.port, &token, &release).await["data"]["releaseAppFile"],
        true
    );
    assert!(dir.path().join(relative).exists());
    assert_eq!(
        call(server.port, &token, &release).await["data"]["releaseAppFile"],
        true
    );
    assert!(!dir.path().join(relative).exists());
    assert_eq!(
        call(server.port, &token, &release).await["data"]["releaseAppFile"],
        false
    );
    server.shutdown().await;
}

async fn upload_attachment(
    server: &ContentServer,
    token: &str,
    bytes: &[u8],
) -> plain_libs::serde_json::Value {
    let id = plain_libs::utils::uuid::Uuid::new_v4().into_string();
    server
        .native_uploads()
        .unwrap()
        .write_chunk(&id, 0, bytes)
        .await
        .unwrap();
    let response = call(server.port, token, &format!(
        r#"mutation {{mergeAppFileChunks(fileId:"{id}",totalChunks:1,fileName:"original.txt",totalSize:{}) {{status error}}}}"#, bytes.len(),
    )).await;
    assert!(response.get("errors").is_none(), "{response}");
    for _ in 0..200 {
        let response = call(
            server.port,
            token,
            &format!(r#"{{mergeStatus(fileId:"{id}") {{status value error}}}}"#),
        )
        .await;
        assert!(response.get("errors").is_none(), "{response}");
        let task = &response["data"]["mergeStatus"];
        assert_ne!(task["status"], "FAILED", "{task}");
        if task["status"] == "DONE" {
            let suffix = task["value"].as_str().unwrap();
            let file_id = suffix.split('.').next().unwrap();
            let response = call(server.port, token, &format!(r#"{{appFiles(query:"text:{file_id}",offset:0,limit:1) {{id realPath size fileName createdAt}}}}"#)).await;
            return response["data"]["appFiles"][0].clone();
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("Attachment upload did not complete");
}

#[tokio::test]
async fn import_root_is_rejected_for_both_origins_without_reading_or_deleting_source() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let source = directory.path().join("preserved-source.txt");
    std::fs::write(&source, b"owned source bytes").unwrap();
    for origin in [
        crate::graphql::executor::RequestOrigin::Local,
        crate::graphql::executor::RequestOrigin::Remote,
    ] {
        for delete_source in [false, true] {
            let response = state.public.execute(
                plain_graphql::Request::new("mutation($path:String!,$delete:Boolean!){importAppFile(source:$path,fileName:\"owned.txt\",mimeType:\"text/plain\",deleteSource:$delete){id}}")
                    .variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({"path":source.to_string_lossy(),"delete":delete_source})))
                    .data(origin).data(state.clone()),
            ).await;
            assert!(
                response
                    .errors
                    .iter()
                    .any(|error| error.message.contains("Unknown field")
                        && error.message.contains("importAppFile")),
                "{:?}",
                response.errors
            );
            assert_eq!(std::fs::read(&source).unwrap(), b"owned source bytes");
            assert_eq!(state.db.app_file_count("").unwrap(), 0);
            assert!(requests.try_recv().is_err());
        }
    }
    core.stop_core().await;
}

#[tokio::test]
async fn remote_attachment_merge_preserves_chunks_when_file_access_is_denied() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state
        .prefs
        .set("api_permissions", plain_libs::serde_json::json!([]))
        .unwrap();
    core.native_uploads()
        .unwrap()
        .write_chunk("denied-attachment", 0, b"preserved")
        .await
        .unwrap();
    let before = state.upload_chunks.list("denied-attachment").await.unwrap();
    let response = state.public.execute(
        plain_graphql::Request::new(r#"mutation {mergeAppFileChunks(fileId:"denied-attachment",totalChunks:1,fileName:"picked.txt",totalSize:9){status}}"#)
            .data(crate::graphql::executor::RequestOrigin::Remote).data(state.clone()),
    ).await;
    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "no_permission");
    assert_eq!(state.db.app_file_count("").unwrap(), 0);
    assert_eq!(
        state.upload_chunks.list("denied-attachment").await.unwrap(),
        before
    );
    core.stop_core().await;
}
