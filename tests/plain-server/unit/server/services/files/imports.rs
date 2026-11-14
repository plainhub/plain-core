use plain_libs::serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn internal_import_keeps_its_staged_source_until_the_database_commit_before_cache_clear() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core =
        Arc::new(crate::server::ContentServer::open(&directory.path().join("data")).unwrap());
    let state = core.runtime_state().core;
    let (generation, mut requests) = state.host.connect();
    let importing = {
        let state = state.clone();
        tokio::spawn(async move {
            plain_rs::features::services::files::imports::import(
                &plain_rs::features::files::app_files::FileStore::new(
                    state.db.clone(),
                    state.directory.clone(),
                ),
                &state.resources,
                state.cache.root(),
                plain_rs::features::services::files::imports::Import {
                    source: "content://fixture/picked".into(),
                    name: "picked.txt".into(),
                    mime: "text/plain".into(),
                    delete_source: false,
                },
                state.stop.clone(),
                state.cache_changes.clone(),
            )
            .await
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
    let mut clearing = Box::pin(plain_rs::infrastructure::cache::clear_application(&state));
    assert!(futures_util::poll!(&mut clearing).is_pending());
    stream.write(b" clear".to_vec()).await.unwrap();
    stream.finish().await.unwrap();
    let row = importing.await.unwrap().unwrap();
    clearing.await.unwrap();
    assert_eq!(
        std::fs::read(state.directory.join(row.real_path)).unwrap(),
        b"before clear"
    );
    assert!(state.cache_changes.try_write().is_ok());
    core.stop_core().await;
}
