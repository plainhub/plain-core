use crate::server::ContentServer;
use crate::test_support::graphql_http as call;
use std::sync::Arc;

#[tokio::test]
async fn file_import_dedupe_release_and_restart_use_core_storage() {
    let dir = tempfile::tempdir().unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[14; 32]);
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs
        .set(
            "api_permissions",
            serde_json::json!(["WRITE_EXTERNAL_STORAGE"]),
        )
        .unwrap();
    let path = dir.path().join("plain.db");
    let server = ContentServer::start(path.parent().unwrap(), &token, prefs.clone()).unwrap();
    let source = dir.path().join("upload");
    std::fs::write(&source, b"shared attachment").unwrap();
    let query = format!(
        r#"mutation {{importAppFile(source:{},fileName:"original.%_",mimeType:"text/plain",deleteSource:false) {{id realPath createdAt size}}}}"#,
        serde_json::to_string(source.to_str().unwrap()).unwrap()
    );
    let first = call(server.port, &token, &query).await;
    assert!(first.get("errors").is_none(), "{first}");
    let file = &first["data"]["importAppFile"];
    let id = file["id"].as_str().unwrap();
    let relative = file["realPath"].as_str().unwrap();
    assert!(std::path::Path::new(relative).is_absolute());
    assert!(std::path::Path::new(relative).starts_with(dir.path().join("files")));
    let stored = server.runtime_state().db.app_file_get(id).unwrap().unwrap();
    assert!(
        !std::path::Path::new(&stored.real_path).is_absolute(),
        "stored attachment paths must survive a data-root change"
    );
    assert_eq!(file["size"], 17);
    let second = call(server.port, &token, &query).await;
    assert_eq!(
        second["data"]["importAppFile"]["id"], id,
        "an identical import must reuse the stored row"
    );
    let page = call(
        server.port,
        &token,
        r#"{appFiles(offset:0,limit:20,query:"%_") {id} appFileCount(query:"%_")}"#,
    )
    .await;
    assert_eq!(page["data"]["appFileCount"], 1);
    assert_eq!(page["data"]["appFiles"][0]["id"], id);
    let url_token = plain_rs::prefs::ensure_url_token(&prefs);
    let encoded = plain_rs::files::resource_link::seal(
        &plain_rs::files::resource_link::Scope::Global,
        &plain_rs::utils::base64::base64_decode(&url_token),
        &plain_rs::files::resource_link::Resource {
            r#type: plain_rs::files::resource_link::ResourceType::AppFile,
            id: id.to_owned(),
        },
    )
    .unwrap();
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/fs", server.port);
    let response = client
        .get(&url)
        .query(&[("id", &encoded)])
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
        .query(&[("id", &encoded)])
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
    assert!(source.exists());
    let moved = call(
        server.port,
        &token,
        &query.replace("deleteSource:false", "deleteSource:true"),
    )
    .await;
    assert!(moved.get("errors").is_none(), "{moved}");
    assert!(!source.exists());
    assert!(dir.path().join(relative).exists());
    server.shutdown().await;
}

#[tokio::test]
async fn remote_import_requires_storage_authorization_before_reading_or_deleting_a_source() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    state
        .prefs
        .set("api_permissions", serde_json::json!([]))
        .unwrap();
    let (_generation, mut requests) = state.host.connect();
    let source = directory.path().join("preserved-source.txt");
    std::fs::write(&source, b"owned source bytes").unwrap();
    for delete_source in [false, true] {
        for path in [
            source.to_string_lossy().into_owned(),
            "content://fixture/denied".into(),
        ] {
            let response = state.public.execute(
                async_graphql::Request::new(
                    "mutation($path:String!,$delete:Boolean!){importAppFile(source:$path,fileName:\"owned.txt\",mimeType:\"text/plain\",deleteSource:$delete){id}}",
                ).variables(async_graphql::Variables::from_json(serde_json::json!({"path":path,"delete":delete_source})))
                    .data(crate::graphql::executor::RequestOrigin::Remote)
                    .data(state.clone()),
            ).await;
            assert_eq!(response.errors.len(), 1);
            assert_eq!(response.errors[0].message, "no_permission");
            assert_eq!(std::fs::read(&source).unwrap(), b"owned source bytes");
            assert_eq!(state.db.app_file_count("").unwrap(), 0);
            assert!(requests.try_recv().is_err());
        }
    }
    state
        .prefs
        .set(
            "api_permissions",
            serde_json::json!(["WRITE_EXTERNAL_STORAGE"]),
        )
        .unwrap();
    let response = state.public.execute(
        async_graphql::Request::new("mutation($path:String!){importAppFile(source:$path,fileName:\"owned.txt\",mimeType:\"text/plain\",deleteSource:false){id}}")
            .variables(async_graphql::Variables::from_json(serde_json::json!({"path":source.to_string_lossy()})))
            .data(crate::graphql::executor::RequestOrigin::Remote)
            .data(state.clone()),
    ).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(state.db.app_file_count("").unwrap(), 1);
    assert_eq!(std::fs::read(&source).unwrap(), b"owned source bytes");
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn main_schema_import_accepts_platform_bytes_without_app_managed_paths() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let schema = core.native_graphql().unwrap();
    let (generation, mut requests) = state.host.connect();
    let execution = tokio::spawn(async move {
        schema.execute(async_graphql::Request::new(
            r#"mutation { importAppFile(source:"content://fixture/picked",fileName:"picked.txt",mimeType:"text/plain",deleteSource:false) { id realPath size fileName } }"#,
        )).await
    });
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "fileResourceStream");
    let stream = state
        .resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    state
        .host
        .reply(
            generation,
            serde_json::json!({"id":request["id"],"result":true}),
        )
        .unwrap();
    stream.write(b"picked data".to_vec()).await.unwrap();
    stream.finish().await.unwrap();
    let response = execution.await.unwrap();
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let file = &data["importAppFile"];
    assert_eq!(file["size"], 11);
    assert_eq!(file["fileName"], "picked.txt");
    let path = std::path::Path::new(file["realPath"].as_str().unwrap());
    assert!(path.starts_with(directory.path().join("files")));
    assert_eq!(std::fs::read(path).unwrap(), b"picked data");
    assert!(!directory.path().join(".rust-import").exists());
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}
