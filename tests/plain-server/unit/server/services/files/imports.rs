use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn main_import_keeps_its_staged_source_until_the_database_commit_before_cache_clear() {
    let directory = tempfile::tempdir().unwrap();
    let core =
        Arc::new(crate::server::ContentServer::open(&directory.path().join("data")).unwrap());
    let state = core.runtime_state().core;
    let (generation, mut requests) = state.host.connect();
    let importing = {
        let core = core.clone();
        tokio::spawn(async move {
            core.native_graphql().unwrap().execute(async_graphql::Request::new(
            "mutation {importAppFile(source:\"content://fixture/picked\",fileName:\"picked.txt\",mimeType:\"text/plain\",deleteSource:false){id}}",
        )).await
        })
    };
    let request = loop {
        let request = requests.recv().await.unwrap();
        if request["method"] == "systemCapabilities" {
            state
                .host
                .reply(generation, json!({"id":request["id"],"result":[]}))
                .unwrap();
        } else {
            break request;
        }
    };
    assert_eq!(request["method"], "fileResourceStream");
    let stream = state
        .resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    state
        .host
        .reply(generation, json!({"id":request["id"],"result":true}))
        .unwrap();
    stream.write(b"before".to_vec()).await.unwrap();
    assert!(state.cache_changes.try_write().is_err());
    let mut clearing = Box::pin(plain_rs::services::cache::clear_application(&state));
    assert!(futures_util::poll!(&mut clearing).is_pending());
    stream.write(b" clear".to_vec()).await.unwrap();
    stream.finish().await.unwrap();
    let response = importing.await.unwrap();
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let row = state
        .db
        .app_file_get(data["importAppFile"]["id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    clearing.await.unwrap();
    assert_eq!(
        std::fs::read(state.directory.join(row.real_path)).unwrap(),
        b"before clear"
    );
    assert!(state.cache_changes.try_write().is_ok());
    core.stop_core().await;
}
